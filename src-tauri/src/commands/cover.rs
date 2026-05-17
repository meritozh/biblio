use base64::{engine::general_purpose::STANDARD, Engine};
use image::ImageReader;
use serde::Serialize;
use std::io::Cursor;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};

/// Re-encode width to keep stored covers small. Card display is 180×280
/// in CSS, so 800 px gives roughly 2x-3x retina headroom while staying
/// far under the 1 MB cliff that motivated this work. JPEG quality 72
/// is the lowest setting that still looks clean at thumbnail size.
const COVER_MAX_WIDTH: u32 = 800;
const COVER_JPEG_QUALITY: u8 = 72;

/// Single enforcement point for cover storage size. Every cover write
/// path — pipeline-produced covers, user uploads in the edit form, the
/// one-shot `db_recompress_covers` migration — runs through this so a
/// 5 MB original JPEG and a 200 KB PNG icon both land in the DB as a
/// consistent ≤200 KB JPEG.
///
/// Decode-failure surfaces an error so the caller can decide: pipelines
/// fall back to the original bytes, the migration logs + skips, the
/// upload command surfaces it as an error. The function itself never
/// silently passes through unreadable input.
pub fn compress_cover_bytes(data: &[u8]) -> Result<Vec<u8>, String> {
    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| format!("Cover format detect failed: {e}"))?;
    let img = reader
        .decode()
        .map_err(|e| format!("Cover decode failed: {e}"))?;

    let resized = if img.width() > COVER_MAX_WIDTH {
        let ratio = COVER_MAX_WIDTH as f32 / img.width() as f32;
        let new_h = (img.height() as f32 * ratio).round() as u32;
        img.resize(COVER_MAX_WIDTH, new_h, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    // JPEG doesn't support alpha; RGB8 is the lowest-friction target.
    let rgb = resized.to_rgb8();
    let mut out = Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, COVER_JPEG_QUALITY);
    rgb.write_with_encoder(encoder)
        .map_err(|e| format!("JPEG encode failed: {e}"))?;
    Ok(out.into_inner())
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

#[derive(Serialize)]
pub struct CoverGetResponse {
    pub data: String,
    pub mime_type: String,
}

#[tauri::command]
pub async fn cover_set(
    app: AppHandle,
    file_id: i64,
    data: Vec<u8>,
    mime_type: Option<String>,
) -> Result<CoverSetResponse, String> {
    // `mime_type` ignored — the helper always returns JPEG. Keeping the
    // parameter in the signature so existing call sites (frontend
    // bridges, replace flow) don't have to change.
    let _ = mime_type;
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let compressed = compress_cover_bytes(&data)?;

    sqlx::query(
        "INSERT OR REPLACE INTO covers (file_id, data, mime_type) VALUES (?, ?, ?)"
    )
    .bind(file_id)
    .bind(&compressed)
    .bind("image/jpeg")
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(CoverSetResponse { success: true })
}

#[derive(Serialize)]
pub struct CoverSetResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn cover_get(
    app: AppHandle,
    file_id: i64,
) -> Result<CoverGetResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let result: Option<(Vec<u8>, String)> = sqlx::query_as(
        "SELECT data, mime_type FROM covers WHERE file_id = ?"
    )
    .bind(file_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    match result {
        Some((data, mime_type)) => {
            let base64_data = STANDARD.encode(&data);
            Ok(CoverGetResponse {
                data: base64_data,
                mime_type,
            })
        }
        None => Err("COVER_NOT_FOUND".to_string()),
    }
}

#[tauri::command]
pub async fn cover_delete(
    app: AppHandle,
    file_id: i64,
) -> Result<CoverDeleteResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    sqlx::query("DELETE FROM covers WHERE file_id = ?")
        .bind(file_id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CoverDeleteResponse { success: true })
}

#[derive(Serialize)]
pub struct CoverDeleteResponse {
    pub success: bool,
}

#[derive(Serialize, Clone)]
struct RecompressProgressEvent {
    done: usize,
    total: usize,
    skipped: usize,
}

#[derive(Serialize)]
pub struct RecompressCoversResponse {
    pub total: usize,
    pub recompressed: usize,
    pub skipped: usize,
}

/// One-shot migration that walks every row in `covers`, re-encodes the
/// stored bytes through `compress_cover_bytes`, and writes the result
/// back as `image/jpeg`. Exposed in Debug Settings so the user can run
/// it explicitly — the operation is **lossy and irreversible**, and on
/// a library with thousands of covers can take a minute or two.
///
/// Per-row decode failure (e.g. exotic format the `image` crate doesn't
/// like) logs to stderr and continues; the goal is "shrink everything I
/// can," not "all or nothing."
///
/// Progress arrives on the frontend via `recompress-covers-progress`
/// events after each row so the Debug Settings UI can render a live
/// counter. Decode + encode happens on a blocking thread per row so
/// the Tauri async runtime stays responsive.
#[tauri::command]
pub async fn db_recompress_covers(
    app: AppHandle,
) -> Result<RecompressCoversResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Load only the id list — fetching every row's BLOB up-front would
    // hold the whole `covers` table in memory before the loop starts
    // (500 covers × 1 MB average = a 500 MB pre-load). Per-id streaming
    // keeps the working set to one cover at a time.
    let ids: Vec<i64> = sqlx::query_scalar("SELECT file_id FROM covers ORDER BY file_id")
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let total = ids.len();
    let mut recompressed = 0usize;
    let mut skipped = 0usize;

    // Emit a baseline event so the UI can show "0 / N" before the first
    // row finishes — covers thousands-of-rows libraries where the first
    // decode might take a noticeable fraction of a second.
    let _ = app.emit(
        "recompress-covers-progress",
        RecompressProgressEvent {
            done: 0,
            total,
            skipped: 0,
        },
    );

    for file_id in ids {
        let data: Option<Vec<u8>> = sqlx::query_scalar("SELECT data FROM covers WHERE file_id = ?")
            .bind(file_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;
        let Some(data) = data else {
            // Row vanished between the id scan and the body fetch
            // (concurrent delete). Treat as skipped — nothing to do.
            skipped += 1;
            let _ = app.emit(
                "recompress-covers-progress",
                RecompressProgressEvent {
                    done: recompressed + skipped,
                    total,
                    skipped,
                },
            );
            continue;
        };

        let compressed = tokio::task::spawn_blocking(move || compress_cover_bytes(&data))
            .await
            .map_err(|e| format!("Cover compress join error: {e}"))?;

        match compressed {
            Ok(bytes) => {
                if let Err(e) = sqlx::query(
                    "UPDATE covers SET data = ?, mime_type = 'image/jpeg' WHERE file_id = ?",
                )
                .bind(&bytes)
                .bind(file_id)
                .execute(&pool)
                .await
                {
                    eprintln!("Recompress UPDATE failed for file_id={file_id}: {e}");
                    skipped += 1;
                } else {
                    recompressed += 1;
                }
            }
            Err(e) => {
                eprintln!("Recompress decode failed for file_id={file_id}: {e}");
                skipped += 1;
            }
        }

        let _ = app.emit(
            "recompress-covers-progress",
            RecompressProgressEvent {
                done: recompressed + skipped,
                total,
                skipped,
            },
        );
    }

    Ok(RecompressCoversResponse {
        total,
        recompressed,
        skipped,
    })
}