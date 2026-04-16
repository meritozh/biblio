use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::Ordering;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool};

use crate::ProcessingCancelled;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedMetadata {
    pub display_name: Option<String>,
    pub suggested_authors: Vec<String>,
    pub metadata: Vec<ExtractedField>,
    pub category_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedField {
    pub key: String,
    pub value: String,
    pub data_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessorResult {
    pub processor_name: String,
    pub status: ProcessorStatus,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub enum ProcessorStatus {
    Success,
    Skipped,
    Failed(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct FileAnalysisResult {
    pub path: String,
    pub file_name: String,
    pub display_name: String,
    pub suggested_authors: Vec<String>,
    pub metadata: Vec<ExtractedField>,
    pub category_hint: Option<String>,
    pub processor_results: Vec<ProcessorResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessingProgress {
    pub current: usize,
    pub total: usize,
    pub current_file: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilePreparedImport {
    pub path: String,
    pub file_name: String,
    pub display_name: String,
    pub category_id: Option<i64>,
    pub tag_ids: Vec<i64>,
    pub author_ids: Vec<i64>,
    pub metadata: Vec<ExtractedField>,
    pub unresolved_author_names: Vec<String>,
    pub cover_data: Option<Vec<u8>>,
    pub cover_mime_type: Option<String>,
    pub progress: Option<String>,
    pub suggested_tags: Vec<String>,
    pub duplicate_of: Option<DuplicateInfo>,
    pub batch_duplicate_group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DuplicateAction {
    Replace,
    Skip,
    ImportAnyway,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateInfo {
    pub existing_file_id: i64,
    pub existing_display_name: String,
    pub existing_progress: Option<String>,
    pub recommendation: DuplicateAction,
}

/// Check if a file path is a novel (text-based book format).
/// Only novels go through the two-call LLM pipeline.
pub fn is_novel_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".txt") || lower.ends_with(".epub")
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

pub trait FileProcessor: Send + Sync {
    fn name(&self) -> &str;
    fn supported_types(&self) -> &[&str];
    fn process(&self, file_path: &Path) -> Result<ExtractedMetadata, String>;
}

pub fn get_processors() -> Vec<Box<dyn FileProcessor>> {
    vec![
        Box::new(FileTypeDetector),
        Box::new(PdfMetadataProcessor),
        Box::new(ExifProcessor),
    ]
}

pub struct FileTypeDetector;

pub struct PdfMetadataProcessor;

pub struct ExifProcessor;


impl FileProcessor for FileTypeDetector {
    fn name(&self) -> &str {
        "file_type"
    }

    fn supported_types(&self) -> &[&str] {
        &["*"]
    }

    fn process(&self, file_path: &Path) -> Result<ExtractedMetadata, String> {
        let bytes = std::fs::read(file_path).map_err(|e| format!("Failed to read file: {e}"))?;

        let sample = &bytes[..bytes.len().min(8192)];

        let mime_type = match infer::get(sample) {
            Some(t) => t.mime_type().to_string(),
            None => file_path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| match ext.to_lowercase().as_str() {
                    "pdf" => "application/pdf",
                    "jpg" | "jpeg" => "image/jpeg",
                    "png" => "image/png",
                    "gif" => "image/gif",
                    "txt" => "text/plain",
                    "html" | "htm" => "text/html",
                    "epub" => "application/epub+zip",
                    _ => "application/octet-stream",
                })
                .unwrap_or("application/octet-stream")
                .to_string(),
        };

        let category_hint = None;

        Ok(ExtractedMetadata {
            display_name: None,
            suggested_authors: vec![],
            metadata: vec![ExtractedField {
                key: "mime_type".to_string(),
                value: mime_type,
                data_type: "text".to_string(),
            }],
            category_hint,
        })
    }
}

fn extract_pdf_info(dict: &lopdf::Dictionary, metadata: &mut Vec<ExtractedField>, display_name: &mut Option<String>, suggested_authors: &mut Vec<String>) {
    let get_str = |key: &[u8]| -> Option<String> {
        dict.get(key).ok().and_then(|v| v.as_str().ok()).map(|s| String::from_utf8_lossy(s).to_string())
    };

    if let Some(title) = get_str(b"Title") {
        if !title.is_empty() {
            *display_name = Some(title);
        }
    }
    if let Some(author) = get_str(b"Author") {
        if !author.is_empty() {
            suggested_authors.push(author);
        }
    }
    let fields: &[(&[u8], &str)] = &[
        (b"Subject", "subject"),
        (b"Keywords", "keywords"),
        (b"Creator", "creator"),
        (b"Producer", "producer"),
    ];
    for &(key, label) in fields {
        if let Some(val) = get_str(key) {
            if !val.is_empty() {
                metadata.push(ExtractedField {
                    key: label.to_string(),
                    value: val,
                    data_type: "text".to_string(),
                });
            }
        }
    }
}

impl FileProcessor for PdfMetadataProcessor {
    fn name(&self) -> &str {
        "pdf_metadata"
    }

    fn supported_types(&self) -> &[&str] {
        &["application/pdf"]
    }

    fn process(&self, file_path: &Path) -> Result<ExtractedMetadata, String> {
        let doc = lopdf::Document::load(file_path).map_err(|e| format!("Failed to load PDF: {e}"))?;

        let page_count = doc.get_pages().len();

        let mut display_name = None::<String>;
        let mut suggested_authors = Vec::new();
        let mut metadata = vec![ExtractedField {
            key: "page_count".to_string(),
            value: page_count.to_string(),
            data_type: "text".to_string(),
        }];

        let info_ref = match doc.trailer.get(b"Info") {
            Ok(obj) => obj,
            Err(_) => {
                return Ok(ExtractedMetadata {
                    display_name,
                    suggested_authors,
                    metadata,
                    category_hint: None,
                });
            }
        };

        match info_ref {
            lopdf::Object::Reference(id) => {
                if let Some(info_obj) = doc.objects.get(id) {
                    if let Ok(info_dict) = info_obj.as_dict() {
                        extract_pdf_info(info_dict, &mut metadata, &mut display_name, &mut suggested_authors);
                    }
                }
            }
            lopdf::Object::Dictionary(dict) => {
                extract_pdf_info(dict, &mut metadata, &mut display_name, &mut suggested_authors);
            }
            _ => {}
        }

        Ok(ExtractedMetadata {
            display_name,
            suggested_authors,
            metadata,
            category_hint: None,
        })
    }
}

impl FileProcessor for ExifProcessor {
    fn name(&self) -> &str {
        "exif"
    }

    fn supported_types(&self) -> &[&str] {
        &["image/jpeg", "image/png", "image/tiff", "image/webp"]
    }

    fn process(&self, file_path: &Path) -> Result<ExtractedMetadata, String> {
        let file = std::fs::File::open(file_path).map_err(|e| format!("Failed to open file: {e}"))?;
        let mut buf_reader = std::io::BufReader::new(file);

        let exif_reader = exif::Reader::new();
        let exif_data = match exif_reader.read_from_container(&mut buf_reader) {
            Ok(data) => data,
            Err(_) => {
                return Ok(ExtractedMetadata {
                    display_name: None,
                    suggested_authors: vec![],
                    metadata: vec![],
                    category_hint: None,
                });
            }
        };

        let mut display_name = None::<String>;
        let mut metadata = Vec::new();

        if let Some(field) = exif_data.get_field(exif::Tag::ImageDescription, exif::In::PRIMARY) {
            let val = field.display_value().to_string();
            if !val.is_empty() {
                display_name = Some(val);
            }
        }

        if let Some(field) = exif_data.get_field(exif::Tag::Model, exif::In::PRIMARY) {
            metadata.push(ExtractedField {
                key: "camera".to_string(),
                value: field.display_value().to_string(),
                data_type: "text".to_string(),
            });
        }

        if let Some(field) = exif_data.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
            metadata.push(ExtractedField {
                key: "date_taken".to_string(),
                value: field.display_value().to_string(),
                data_type: "date".to_string(),
            });
        }

        let w = exif_data.get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY);
        let h = exif_data.get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY);
        if let (Some(wf), Some(hf)) = (w, h) {
            metadata.push(ExtractedField {
                key: "dimensions".to_string(),
                value: format!("{}x{}", wf.display_value(), hf.display_value()),
                data_type: "text".to_string(),
            });
        }

        Ok(ExtractedMetadata {
            display_name,
            suggested_authors: vec![],
            metadata,
            category_hint: None,
        })
    }
}

fn type_matches(processor_types: &[&str], known_mime: Option<&str>) -> bool {
    if processor_types.contains(&"*") {
        return true;
    }
    match known_mime {
        Some(mime) => processor_types.iter().any(|t| {
            if t.ends_with("/*") {
                let prefix = &t[..t.len() - 2];
                mime.starts_with(prefix)
            } else {
                *t == mime
            }
        }),
        None => false,
    }
}

fn extract_cover_from_archive(file_path: &Path) -> Result<(Vec<u8>, String), String> {
    use std::io::Read;
    use zip::ZipArchive;

    let file = std::fs::File::open(file_path)
        .map_err(|e| format!("Failed to open archive: {e}"))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| format!("Failed to read ZIP archive: {e}"))?;

    // Collect all image entries with their names
    let mut image_entries: Vec<(String, usize)> = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| format!("ZIP entry error: {e}"))?;
        let name = file.name().to_string();
        let lower = name.to_lowercase();
        if lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".png")
            || lower.ends_with(".webp")
            || lower.ends_with(".gif")
        {
            image_entries.push((name, i));
        }
    }

    if image_entries.is_empty() {
        return Err("No images found in archive".to_string());
    }

    // Sort by filename to find cover (alphabetical)
    image_entries.sort_by(|a, b| a.0.cmp(&b.0));

    // Prefer files named "cover" or starting with "cover" or "000"/"001"
    let cover_idx = image_entries
        .iter()
        .find(|(name, _)| {
            let lower = name.to_lowercase();
            lower.contains("cover") || lower.starts_with("000") || lower.starts_with("001")
        })
        .map(|&(_, idx)| idx)
        .unwrap_or(image_entries[0].1);

    let mut file = archive
        .by_index(cover_idx)
        .map_err(|e| format!("Failed to read cover entry: {e}"))?;
    let name = file.name().to_string();

    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|e| format!("Failed to read cover data: {e}"))?;

    let mime_type = if name.to_lowercase().ends_with(".png") {
        "image/png".to_string()
    } else if name.to_lowercase().ends_with(".webp") {
        "image/webp".to_string()
    } else if name.to_lowercase().ends_with(".gif") {
        "image/gif".to_string()
    } else {
        "image/jpeg".to_string()
    };

    Ok((data, mime_type))
}

/// Detect encoding and decode bytes to UTF-8.
/// Returns None on low confidence (<0.7), unknown encoding label,
/// or when decoding produces replacement characters (corrupted bytes).
fn decode_to_utf8(bytes: &[u8]) -> Option<String> {
    let detected = chardet::detect_bytes(bytes, chardet::EncodingEra::All, 200_000);
    if detected.confidence < 0.7 {
        return None;
    }

    let encoding_label = detected.encoding?;
    let encoding = encoding_rs::Encoding::for_label(encoding_label.as_bytes())?;
    let (text, _, had_errors) = encoding.decode(bytes);
    if had_errors {
        return None;
    }

    Some(text.into_owned())
}

fn sample_text_content(file_path: &Path, num_samples: usize, sample_size: usize) -> Option<String> {
    let bytes = std::fs::read(file_path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let content = decode_to_utf8(&bytes)?;
    let total_chars: usize = content.chars().count();

    if total_chars == 0 {
        return None;
    }

    let total_needed = num_samples * sample_size;
    if total_chars <= total_needed {
        return Some(content);
    }

    let chars: Vec<char> = content.chars().collect();
    let mut result = String::new();
    let labels = ["Beginning", "25%", "50%", "75%", "Near End"];

    for i in 0..num_samples {
        let position = if i == num_samples - 1 {
            (total_chars as f64 * 0.95) as usize
        } else {
            (total_chars as f64 * (i as f64 / (num_samples - 1) as f64)) as usize
        };

        let start = position.min(total_chars.saturating_sub(sample_size));
        let end = (start + sample_size).min(total_chars);
        let sample: String = chars[start..end].iter().collect();

        let label = labels.get(i).unwrap_or(&"Sample");
        result.push_str(&format!("[Sample {} - {}]\n{}\n\n", i + 1, label, sample));
    }

    Some(result)
}

#[tauri::command]
pub async fn file_analyze(
    app: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<Vec<FileAnalysisResult>, String> {
    let total = paths.len();
    let app_clone = app.clone();

    let results = tauri::async_runtime::spawn_blocking(move || {
        let processors = get_processors();
        let mut results = Vec::new();

        for (idx, path_str) in paths.iter().enumerate() {
            let file_path = Path::new(path_str);
            let file_name = file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let progress = ProcessingProgress {
                current: idx + 1,
                total,
                current_file: file_name.clone(),
                status: "processing".to_string(),
            };
            let _ = app_clone.emit("processing-progress", &progress);

            let mut merged_display_name = None::<String>;
            let mut merged_authors = Vec::new();
            let mut seen_authors = HashSet::new();
            let mut merged_metadata: Vec<ExtractedField> = Vec::new();
            let mut merged_category_hint = None::<String>;
            let mut processor_results = Vec::new();

            let mut known_mime: Option<String> = None;

            for processor in &processors {
                let supported = type_matches(
                    processor.supported_types(),
                    known_mime.as_deref(),
                );

                if !supported {
                    processor_results.push(ProcessorResult {
                        processor_name: processor.name().to_string(),
                        status: ProcessorStatus::Skipped,
                        fields: vec![],
                    });
                    continue;
                }

                match processor.process(file_path) {
                    Ok(extracted) => {
                        let field_keys: Vec<String> =
                            extracted.metadata.iter().map(|f| f.key.clone()).collect();

                        if merged_display_name.is_none() && extracted.display_name.is_some() {
                            merged_display_name = extracted.display_name;
                        }

                        for author in extracted.suggested_authors {
                            if seen_authors.insert(author.clone()) {
                                merged_authors.push(author);
                            }
                        }

                        for field in extracted.metadata {
                            if field.key == "mime_type" {
                                known_mime = Some(field.value.clone());
                            }
                            if let Some(existing) = merged_metadata.iter_mut().find(|f| f.key == field.key) {
                                *existing = field;
                            } else {
                                merged_metadata.push(field);
                            }
                        }

                        if merged_category_hint.is_none() && extracted.category_hint.is_some() {
                            merged_category_hint = extracted.category_hint;
                        }

                        processor_results.push(ProcessorResult {
                            processor_name: processor.name().to_string(),
                            status: ProcessorStatus::Success,
                            fields: field_keys,
                        });
                    }
                    Err(e) => {
                        processor_results.push(ProcessorResult {
                            processor_name: processor.name().to_string(),
                            status: ProcessorStatus::Failed(e),
                            fields: vec![],
                        });
                    }
                }
            }

            results.push(FileAnalysisResult {
                path: path_str.clone(),
                file_name: file_name.clone(),
                display_name: merged_display_name.unwrap_or(file_name),
                suggested_authors: merged_authors,
                metadata: merged_metadata,
                category_hint: merged_category_hint,
                processor_results,
            });
        }

        results
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    Ok(results)
}

#[tauri::command]
pub async fn cancel_processing(app: tauri::AppHandle) {
    let cancelled = app.state::<ProcessingCancelled>();
    cancelled.0.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub async fn file_prepare_import(
    app: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<Vec<FilePreparedImport>, String> {
    use crate::commands::{Author, Category, Tag};

    // Reset cancellation flag
    let cancelled = app.state::<ProcessingCancelled>();
    cancelled.0.store(false, Ordering::Relaxed);

    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let categories: Vec<Category> = sqlx::query_as(
        "SELECT id, name, description, icon, is_default, folder_name, created_at FROM categories",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("Failed to load categories: {e}"))?;

    let authors: Vec<Author> = sqlx::query_as(
        "SELECT id, name, created_at FROM authors",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("Failed to load authors: {e}"))?;

    let tags: Vec<Tag> = sqlx::query_as(
        "SELECT id, name, color, created_at FROM tags",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("Failed to load tags: {e}"))?;

    let category_map: std::collections::HashMap<String, i64> = categories
        .iter()
        .map(|c| (c.name.to_lowercase(), c.id))
        .collect();

    let author_map: std::collections::HashMap<String, i64> = authors
        .iter()
        .map(|a| (a.name.to_lowercase(), a.id))
        .collect();

    let tag_map: std::collections::HashMap<String, i64> = tags
        .iter()
        .map(|t| (t.name.to_lowercase(), t.id))
        .collect();

    let category_names: Vec<String> = categories.iter().map(|c| {
        match &c.description {
            Some(desc) if !desc.is_empty() => format!("{} ({})", c.name, desc),
            _ => c.name.clone(),
        }
    }).collect();
    let tag_names: Vec<String> = tags.iter().map(|t| t.name.clone()).collect();

    // Load LLM config early to check analyze_content setting
    let llm_analyze_content = super::llm::load_config(&pool).await
        .map(|c| c.analyze_content)
        .unwrap_or(true);

    // Phase 1: Run Rust processors (signal gathering)
    let total = paths.len();
    let app_clone = app.clone();

    let phase1_results = tauri::async_runtime::spawn_blocking(move || {
        let processors = get_processors();
        let mut results = Vec::new();

        for (idx, path_str) in paths.iter().enumerate() {
            let file_path = Path::new(path_str);
            let file_name = file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let progress = ProcessingProgress {
                current: idx + 1,
                total,
                current_file: file_name.clone(),
                status: "gathering_signals".to_string(),
            };
            let _ = app_clone.emit("processing-progress", &progress);

            let mut merged_metadata: Vec<ExtractedField> = Vec::new();
            let mut merged_authors = Vec::new();
            let mut seen_authors = HashSet::new();
            let mut known_mime: Option<String> = None;

            for processor in &processors {
                let supported = type_matches(
                    processor.supported_types(),
                    known_mime.as_deref(),
                );
                if !supported {
                    continue;
                }
                if let Ok(extracted) = processor.process(file_path) {
                    for author in extracted.suggested_authors {
                        if seen_authors.insert(author.clone()) {
                            merged_authors.push(author);
                        }
                    }
                    for field in extracted.metadata {
                        if field.key == "mime_type" {
                            known_mime = Some(field.value.clone());
                        }
                        if let Some(existing) = merged_metadata.iter_mut().find(|f| f.key == field.key) {
                            *existing = field;
                        } else {
                            merged_metadata.push(field);
                        }
                    }
                }
            }

            // Content sampling for .txt files (only when analyze_content is enabled)
            let mime = known_mime.as_deref().unwrap_or("");
            let content = if llm_analyze_content && (mime == "text/plain" || file_path.extension().and_then(|e| e.to_str()) == Some("txt")) {
                sample_text_content(file_path, 5, 1000)
            } else {
                None
            };

            // Cover extraction for archives/images
            let is_archive = mime.contains("zip") || mime.contains("cbz");
            let is_comic_image = mime.starts_with("image/");

            let (cover_data, cover_mime_type) = if is_archive {
                match extract_cover_from_archive(file_path) {
                    Ok((data, mime)) => (Some(data), Some(mime)),
                    Err(_) => (None, None),
                }
            } else if is_comic_image {
                match std::fs::read(file_path) {
                    Ok(data) => (Some(data), Some(mime.to_string())),
                    Err(_) => (None, None),
                }
            } else {
                (None, None)
            };

            results.push((path_str.clone(), file_name, merged_metadata, merged_authors, content, cover_data, cover_mime_type));
        }

        results
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    // Phase 2: LLM extraction — two-call pipeline for novels
    let llm_config = super::llm::load_config(&pool).await?;
    let mut results: Vec<FilePreparedImport> = Vec::new();

    for (path_str, file_name, merged_metadata, merged_authors, content, cover_data, cover_mime_type) in phase1_results {
        // Check cancellation before each file
        if cancelled.0.load(Ordering::Relaxed) {
            break;
        }

        let mut progress_val: Option<String> = None;
        let mut category_id: Option<i64> = None;
        let mut tag_ids: Vec<i64> = Vec::new();
        let mut suggested_tags: Vec<String> = Vec::new();
        let mut author_ids: Vec<i64> = Vec::new();
        let mut unresolved_author_names: Vec<String> = Vec::new();
        let mut final_display_name = file_name.clone();
        let mut final_metadata = merged_metadata.clone();

        // Resolve processor-found authors
        for author_name in &merged_authors {
            if let Some(&id) = author_map.get(&author_name.to_lowercase()) {
                if !author_ids.contains(&id) {
                    author_ids.push(id);
                }
            } else if !unresolved_author_names.contains(author_name) {
                unresolved_author_names.push(author_name.clone());
            }
        }

        let is_novel = is_novel_file(&path_str);

        if is_novel && llm_config.enabled {
            // === Call 1: filename extraction ===
            let _ = app.emit("processing-progress", &ProcessingProgress {
                current: results.len() + 1,
                total,
                current_file: file_name.clone(),
                status: "extracting_name".to_string(),
            });

            let name_result = super::llm::extract_filename_metadata(
                &llm_config,
                &file_name,
            ).await;

            // Apply name result (independent fallback: if this call fails, keep defaults)
            if let Ok(ref name_meta) = name_result {
                if let Some(ref name) = name_meta.display_name {
                    if !name.is_empty() {
                        final_display_name = name.clone();
                    }
                }
                if let Some(ref p) = name_meta.progress {
                    if !p.is_empty() {
                        progress_val = Some(p.clone());
                    }
                }
                for author in &name_meta.authors {
                    if let Some(&id) = author_map.get(&author.to_lowercase()) {
                        if !author_ids.contains(&id) {
                            author_ids.push(id);
                        }
                    } else if !unresolved_author_names.contains(author) {
                        unresolved_author_names.push(author.clone());
                    }
                }
            } else if let Err(ref e) = name_result {
                eprintln!("LLM filename extraction failed for {}: {}", file_name, e);
            }

            // === Call 2: content analysis ===
            let _ = app.emit("processing-progress", &ProcessingProgress {
                current: results.len() + 1,
                total,
                current_file: file_name.clone(),
                status: "analyzing_content".to_string(),
            });

            let content_result = if llm_config.analyze_content {
                match content.as_deref() {
                    Some(c) => {
                        let hint = name_result.as_ref().ok()
                            .and_then(|r| r.display_name.as_deref());
                        super::llm::extract_content_metadata(
                            &llm_config,
                            c,
                            hint,
                            &category_names,
                            &tag_names,
                        ).await
                    }
                    None => Err("No content sampled".to_string()),
                }
            } else {
                Err("Content analysis disabled".to_string())
            };

            if let Ok(ref content_meta) = content_result {
                if let Some(ref cat) = content_meta.category {
                    // Defensive: LLM may include the parenthesized description
                    // (e.g. "h-novel (novel with sexual content)"). Strip it.
                    let cat_clean = cat.split('(').next().unwrap_or(cat).trim().to_lowercase();
                    category_id = category_map.get(&cat_clean).copied();
                }
                for tag in &content_meta.tags {
                    if let Some(&id) = tag_map.get(&tag.to_lowercase()) {
                        if !tag_ids.contains(&id) {
                            tag_ids.push(id);
                        }
                    } else if !suggested_tags.contains(tag) {
                        suggested_tags.push(tag.clone());
                    }
                }
                if let Some(ref desc) = content_meta.description {
                    if !desc.is_empty() {
                        if let Some(existing) = final_metadata.iter_mut().find(|f| f.key == "description") {
                            existing.value = desc.clone();
                        } else {
                            final_metadata.push(ExtractedField {
                                key: "description".to_string(),
                                value: desc.clone(),
                                data_type: "text".to_string(),
                            });
                        }
                    }
                }
            } else if let Err(ref e) = content_result {
                eprintln!("LLM content analysis failed for {}: {}", file_name, e);
            }

            // Emit final per-file status
            // If content analysis is disabled by config, treat a missing content result
            // as "ready" (not "partial") — partial means a real LLM failure.
            let content_analysis_active = llm_config.analyze_content;
            let final_status = match (&name_result, &content_result, content_analysis_active) {
                (Ok(_), Ok(_), _) => "ready",
                (Ok(_), Err(_), false) => "ready",
                (Err(_), Err(_), false) => "error",
                (Ok(_), Err(_), true) | (Err(_), Ok(_), _) => "partial",
                (Err(_), Err(_), true) => "error",
            };
            let _ = app.emit("processing-progress", &ProcessingProgress {
                current: results.len() + 1,
                total,
                current_file: file_name.clone(),
                status: final_status.to_string(),
            });
        } else {
            // Non-novel or LLM disabled: emit ready immediately (no LLM extraction)
            let _ = app.emit("processing-progress", &ProcessingProgress {
                current: results.len() + 1,
                total,
                current_file: file_name.clone(),
                status: "ready".to_string(),
            });
        }

        results.push(FilePreparedImport {
            path: path_str,
            file_name,
            display_name: final_display_name,
            category_id,
            tag_ids,
            author_ids,
            metadata: final_metadata,
            unresolved_author_names,
            cover_data,
            cover_mime_type,
            progress: progress_val,
            suggested_tags,
            duplicate_of: None,
            batch_duplicate_group: None,
        });
    }

    // Phase 3: Duplicate detection
    detect_duplicates(&pool, &mut results).await?;

    Ok(results)
}

async fn detect_duplicates(
    pool: &sqlx::SqlitePool,
    results: &mut Vec<FilePreparedImport>,
) -> Result<(), String> {
    use crate::commands::FileEntry;

    let existing_files: Vec<FileEntry> = sqlx::query_as(
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, created_at, updated_at FROM files",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to load existing files: {e}"))?;

    for result in results.iter_mut() {
        let normalized_name = result.display_name.trim().to_lowercase();

        if let Some(existing) = existing_files.iter().find(|f| f.display_name.trim().to_lowercase() == normalized_name) {
            let recommendation = match (&result.progress, &existing.progress) {
                (Some(new_p), Some(old_p)) if new_p >= old_p => DuplicateAction::Replace,
                (Some(_), None) => DuplicateAction::Replace,
                (None, Some(_)) => DuplicateAction::Skip,
                _ => DuplicateAction::Replace,
            };

            result.duplicate_of = Some(DuplicateInfo {
                existing_file_id: existing.id,
                existing_display_name: existing.display_name.clone(),
                existing_progress: existing.progress.clone(),
                recommendation,
            });
        }
    }

    let mut name_groups: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
    for (idx, result) in results.iter().enumerate() {
        let normalized = result.display_name.trim().to_lowercase();
        name_groups.entry(normalized).or_default().push(idx);
    }

    for (name, indices) in &name_groups {
        if indices.len() > 1 {
            let group_id = format!("batch_{}", name);
            for &idx in indices {
                results[idx].batch_duplicate_group = Some(group_id.clone());
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_names() {
        assert_eq!(FileTypeDetector.name(), "file_type");
        assert_eq!(PdfMetadataProcessor.name(), "pdf_metadata");
        assert_eq!(ExifProcessor.name(), "exif");
    }

    #[test]
    fn test_processor_supported_types() {
        assert_eq!(FileTypeDetector.supported_types(), &["*"]);
        assert_eq!(PdfMetadataProcessor.supported_types(), &["application/pdf"]);
        assert!(ExifProcessor.supported_types().contains(&"image/jpeg"));
    }

    #[test]
    fn test_type_matches_wildcard() {
        assert!(type_matches(&["*"], None));
        assert!(type_matches(&["*"], Some("application/pdf")));
    }

    #[test]
    fn test_type_matches_specific() {
        assert!(type_matches(&["application/pdf"], Some("application/pdf")));
        assert!(!type_matches(&["application/pdf"], Some("image/jpeg")));
        assert!(!type_matches(&["application/pdf"], None));
    }

    #[test]
    fn test_is_novel_file() {
        assert!(is_novel_file("book.txt"));
        assert!(is_novel_file("book.TXT"));
        assert!(is_novel_file("book.epub"));
        assert!(is_novel_file("/path/to/book.txt"));
        assert!(!is_novel_file("book.zip"));
        assert!(!is_novel_file("book.pdf"));
        assert!(!is_novel_file("book"));
    }

    #[test]
    fn test_decode_to_utf8_plain_ascii() {
        let bytes = b"Hello, world!";
        let result = decode_to_utf8(bytes);
        assert_eq!(result.as_deref(), Some("Hello, world!"));
    }

    #[test]
    fn test_decode_to_utf8_utf8_chinese() {
        let text = "你好，世界！这是一段中文测试内容，包含足够的字符让 chardet 有信心识别为 UTF-8。三体，刘慈欣。";
        let bytes = text.as_bytes();
        let result = decode_to_utf8(bytes);
        assert_eq!(result.as_deref(), Some(text));
    }

    #[test]
    fn test_decode_to_utf8_gb18030_chinese() {
        // Encode a realistic-length varied Chinese text to GB18030,
        // then verify decode_to_utf8 round-trips it correctly.
        // chardet needs enough varied bytes to reach confidence threshold.
        let sample = "这是一段较长的中文测试内容，用于验证 GB18030 编码的文件能够被正确识别和转换为 UTF-8。内容包含了标点符号、数字 123、以及一些常见的汉字，比如：你好世界、春夏秋冬、日月星辰、山川河流。小说标题示例：三体、流浪地球、活着、平凡的世界。作者示例：刘慈欣、余华、路遥、莫言。";
        // Repeat to ensure ~2KB of varied text, enough for chardet detection.
        let full_text = sample.repeat(5);
        let gb18030 = encoding_rs::GB18030;
        let (encoded_bytes, _, had_errors) = gb18030.encode(&full_text);
        assert!(!had_errors, "Test setup: encoding to GB18030 should not produce errors");

        let result = decode_to_utf8(&encoded_bytes);
        assert!(result.is_some(), "GB18030 bytes should decode successfully");
        let decoded = result.unwrap();
        // Verify round-trip produced matching Chinese content.
        assert!(
            decoded.contains("三体") && decoded.contains("刘慈欣"),
            "decoded text should contain expected Chinese chars from the sample, got first 100 chars: {}",
            &decoded.chars().take(100).collect::<String>()
        );
    }

    #[test]
    fn test_decode_to_utf8_garbage_returns_none() {
        // Random non-text bytes should result in low confidence or decode errors.
        let bytes: Vec<u8> = (0..200).map(|i| (i as u8).wrapping_mul(31)).collect();
        let result = decode_to_utf8(&bytes);
        assert!(result.is_none(), "Garbage bytes should return None");
    }
}
