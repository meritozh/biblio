use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool};

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
        Box::new(FilenameProcessor),
        Box::new(FileTypeDetector),
        Box::new(PdfMetadataProcessor),
        Box::new(ExifProcessor),
    ]
}

pub struct FilenameProcessor;

pub struct FileTypeDetector;

pub struct PdfMetadataProcessor;

pub struct ExifProcessor;

impl FileProcessor for FilenameProcessor {
    fn name(&self) -> &str {
        "filename"
    }

    fn supported_types(&self) -> &[&str] {
        &["*"]
    }

    fn process(&self, file_path: &Path) -> Result<ExtractedMetadata, String> {
        let stem = file_path
            .file_stem()
            .ok_or_else(|| "Cannot extract file stem".to_string())?
            .to_string_lossy()
            .to_string();

        let author_title_re = Regex::new(r"^(.+?)\s*-\s*(.+)$").map_err(|e| e.to_string())?;
        let year_re = Regex::new(r"^(.+?)\s*\((\d{4})\)\s*$").map_err(|e| e.to_string())?;

        let clean = |s: &str| -> String {
            s.replace(['_', '-', '.'], " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };

        if let Some(caps) = author_title_re.captures(&stem) {
            let author = clean(caps[1].trim());
            let title = clean(caps[2].trim());
            return Ok(ExtractedMetadata {
                display_name: Some(title),
                suggested_authors: vec![author],
                metadata: vec![],
                category_hint: None,
            });
        }

        if let Some(caps) = year_re.captures(&stem) {
            let title = clean(caps[1].trim());
            let year = caps[2].trim().to_string();
            return Ok(ExtractedMetadata {
                display_name: Some(title),
                suggested_authors: vec![],
                metadata: vec![ExtractedField {
                    key: "year".to_string(),
                    value: year,
                    data_type: "text".to_string(),
                }],
                category_hint: None,
            });
        }

        Ok(ExtractedMetadata {
            display_name: Some(clean(&stem)),
            suggested_authors: vec![],
            metadata: vec![],
            category_hint: None,
        })
    }
}

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

        let category_hint = if mime_type == "application/pdf" {
            Some("Novels".to_string())
        } else if mime_type.starts_with("image/") {
            Some("Comics".to_string())
        } else if mime_type.starts_with("text/") {
            Some("Documents".to_string())
        } else {
            None
        };

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

fn extract_pdf_text(file_path: &Path, max_chars: usize) -> Option<String> {
    let doc = lopdf::Document::load(file_path).ok()?;
    let mut text = String::new();
    let pages = doc.get_pages();

    for (_, page_id) in pages {
        let page_obj = match doc.objects.get(&page_id) {
            Some(obj) => obj,
            None => continue,
        };

        let page_dict = match page_obj.as_dict() {
            Ok(d) => d,
            Err(_) => continue,
        };

        let contents_ref = match page_dict.get(b"Contents") {
            Ok(c) => c,
            Err(_) => continue,
        };

        let content_ids: Vec<lopdf::ObjectId> = match contents_ref {
            lopdf::Object::Reference(id) => vec![*id],
            lopdf::Object::Array(arr) => arr
                .iter()
                .filter_map(|o| {
                    if let lopdf::Object::Reference(id) = o { Some(*id) } else { None }
                })
                .collect(),
            _ => vec![],
        };

        for cid in content_ids {
            if let Some(obj) = doc.objects.get(&cid) {
                if let Ok(stream) = obj.as_stream() {
                    if let Ok(content_bytes) = stream.decompressed_content() {
                        let content_str = String::from_utf8_lossy(&content_bytes);
                        for token in content_str.split_whitespace() {
                            if token.starts_with('(') && token.ends_with(')') {
                                let inner = &token[1..token.len() - 1];
                                text.push_str(inner);
                                text.push(' ');
                            }
                        }
                    }
                }
            }
        }

        if text.len() >= max_chars {
            break;
        }
    }

    if text.is_empty() {
        None
    } else {
        Some(text.chars().take(max_chars).collect())
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
pub async fn file_prepare_import(
    app: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<Vec<FilePreparedImport>, String> {
    use crate::commands::{Author, Category};

    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let categories: Vec<Category> = sqlx::query_as(
        "SELECT id, name, icon, is_default, folder_name, created_at FROM categories",
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

    let category_map: std::collections::HashMap<String, i64> = categories
        .iter()
        .map(|c| (c.name.to_lowercase(), c.id))
        .collect();

    let author_map: std::collections::HashMap<String, i64> = authors
        .iter()
        .map(|a| (a.name.to_lowercase(), a.id))
        .collect();

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
            let mut known_mime: Option<String> = None;

            for processor in &processors {
                let supported = type_matches(
                    processor.supported_types(),
                    known_mime.as_deref(),
                );

                if !supported {
                    continue;
                }

                match processor.process(file_path) {
                    Ok(extracted) => {
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
                    }
                    Err(_) => {}
                }
            }

            let category_id = merged_category_hint
                .as_ref()
                .and_then(|hint| category_map.get(&hint.to_lowercase()).copied());

            let mut author_ids = Vec::new();
            let mut unresolved_author_names = Vec::new();
            for author_name in &merged_authors {
                if let Some(&id) = author_map.get(&author_name.to_lowercase()) {
                    author_ids.push(id);
                } else {
                    unresolved_author_names.push(author_name.clone());
                }
            }

            let mime_type_field = merged_metadata.iter().find(|m| m.key == "mime_type");
            let is_archive = mime_type_field.as_ref().map_or(false, |m| {
                let mt = m.value.to_lowercase();
                mt.contains("zip") || mt.contains("cbz")
            });
            let is_comic_image = mime_type_field
                .as_ref()
                .map_or(false, |m| m.value.starts_with("image/"));

            let (cover_data, cover_mime_type) = if is_archive {
                match extract_cover_from_archive(file_path) {
                    Ok((data, mime)) => (Some(data), Some(mime)),
                    Err(e) => {
                        eprintln!("Cover extraction failed for {}: {}", file_name, e);
                        (None, None)
                    }
                }
            } else if is_comic_image {
                match std::fs::read(file_path) {
                    Ok(data) => {
                        let mime = mime_type_field
                            .map(|m| m.value.clone())
                            .unwrap_or("image/jpeg".to_string());
                        (Some(data), Some(mime))
                    }
                    Err(_) => (None, None),
                }
            } else {
                (None, None)
            };

            results.push(FilePreparedImport {
                path: path_str.clone(),
                file_name: file_name.clone(),
                display_name: merged_display_name.unwrap_or(file_name),
                category_id,
                tag_ids: vec![],
                author_ids,
                metadata: merged_metadata,
                unresolved_author_names,
                cover_data,
                cover_mime_type,
            });
        }

        results
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    let llm_enabled: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'llm_enabled'",
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let llm_enabled = llm_enabled
        .and_then(|(v,)| v.parse::<bool>().ok())
        .unwrap_or(false);

    let mut results = results;

    if llm_enabled {
        let config = super::llm::load_config(&pool).await?;

        let categories: Vec<Category> = sqlx::query_as(
            "SELECT id, name, icon, is_default, folder_name, created_at FROM categories",
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

        let llm_category_map: std::collections::HashMap<String, i64> = categories
            .iter()
            .map(|c| (c.name.to_lowercase(), c.id))
            .collect();

        let llm_author_map: std::collections::HashMap<String, i64> = authors
            .iter()
            .map(|a| (a.name.to_lowercase(), a.id))
            .collect();

        for result in &mut results {
            let content = if config.mode == "with_content" {
                let path = Path::new(&result.path);
                let mime = result
                    .metadata
                    .iter()
                    .find(|m| m.key == "mime_type")
                    .map(|m| m.value.as_str());
                match mime {
                    Some("application/pdf") => extract_pdf_text(path, 4000),
                    _ => None,
                }
            } else {
                None
            };

            let category_name = result.category_id
                .and_then(|id| categories.iter().find(|c| c.id == id).map(|c| c.name.as_str()));

            match super::llm::extract_metadata_with_llm(
                &config,
                &pool,
                &result.display_name,
                &result.metadata,
                content.as_deref(),
                category_name,
            )
            .await
            {
                Ok(llm_result) => {
                    if let Some(name) = llm_result.display_name {
                        result.display_name = name;
                    }
                    if let Some(category) = &llm_result.category {
                        result.category_id = llm_category_map.get(&category.to_lowercase()).copied();
                    }
                    for author in &llm_result.authors {
                        if let Some(&id) = llm_author_map.get(&author.to_lowercase()) {
                            if !result.author_ids.contains(&id) {
                                result.author_ids.push(id);
                            }
                        } else if !result.unresolved_author_names.contains(author) {
                            result.unresolved_author_names.push(author.clone());
                        }
                    }
                    for tag in &llm_result.tags {
                        result.metadata.push(ExtractedField {
                            key: "suggested_tag".to_string(),
                            value: tag.clone(),
                            data_type: "text".to_string(),
                        });
                    }
                    if let Some(desc) = llm_result.description {
                        result.metadata.push(ExtractedField {
                            key: "description".to_string(),
                            value: desc,
                            data_type: "text".to_string(),
                        });
                    }
                    if let Some(val) = llm_result.isbn {
                        result.metadata.push(ExtractedField {
                            key: "isbn".to_string(),
                            value: val,
                            data_type: "text".to_string(),
                        });
                    }
                    if let Some(val) = llm_result.publisher {
                        result.metadata.push(ExtractedField {
                            key: "publisher".to_string(),
                            value: val,
                            data_type: "text".to_string(),
                        });
                    }
                    if let Some(val) = llm_result.year {
                        result.metadata.push(ExtractedField {
                            key: "year".to_string(),
                            value: val,
                            data_type: "text".to_string(),
                        });
                    }
                    if let Some(val) = llm_result.language {
                        result.metadata.push(ExtractedField {
                            key: "language".to_string(),
                            value: val,
                            data_type: "text".to_string(),
                        });
                    }
                    if let Some(val) = llm_result.series {
                        result.metadata.push(ExtractedField {
                            key: "series".to_string(),
                            value: val,
                            data_type: "text".to_string(),
                        });
                    }
                    if let Some(val) = llm_result.volume {
                        result.metadata.push(ExtractedField {
                            key: "volume".to_string(),
                            value: val,
                            data_type: "text".to_string(),
                        });
                    }
                    if let Some(val) = llm_result.issue_number {
                        result.metadata.push(ExtractedField {
                            key: "issue_number".to_string(),
                            value: val,
                            data_type: "text".to_string(),
                        });
                    }
                }
                Err(e) => {
                    eprintln!(
                        "LLM enhancement failed for {}: {}",
                        result.file_name, e
                    );
                }
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_filename_basic() {
        let processor = FilenameProcessor;
        let path = PathBuf::from("/some/path/My Document.pdf");
        let result = processor.process(&path).unwrap();
        assert_eq!(result.display_name.unwrap(), "My Document");
    }

    #[test]
    fn test_filename_author_title() {
        let processor = FilenameProcessor;
        let path = PathBuf::from("/some/John Doe - Great Book.epub");
        let result = processor.process(&path).unwrap();
        assert_eq!(result.display_name.unwrap(), "Great Book");
        assert!(result.suggested_authors.contains(&"John Doe".to_string()));
    }

    #[test]
    fn test_filename_with_year() {
        let processor = FilenameProcessor;
        let path = PathBuf::from("/some/Amazing Story (2024).pdf");
        let result = processor.process(&path).unwrap();
        assert_eq!(result.display_name.unwrap(), "Amazing Story");
        let year_meta = result.metadata.iter().find(|m| m.key == "year");
        assert!(year_meta.is_some());
        assert_eq!(year_meta.unwrap().value, "2024");
    }

    #[test]
    fn test_filename_underscores() {
        let processor = FilenameProcessor;
        let path = PathBuf::from("/some/my_awesome_file.txt");
        let result = processor.process(&path).unwrap();
        assert_eq!(result.display_name.unwrap(), "my awesome file");
    }

    #[test]
    fn test_processor_names() {
        assert_eq!(FilenameProcessor.name(), "filename");
        assert_eq!(FileTypeDetector.name(), "file_type");
        assert_eq!(PdfMetadataProcessor.name(), "pdf_metadata");
        assert_eq!(ExifProcessor.name(), "exif");
    }

    #[test]
    fn test_processor_supported_types() {
        assert_eq!(FilenameProcessor.supported_types(), &["*"]);
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
}
