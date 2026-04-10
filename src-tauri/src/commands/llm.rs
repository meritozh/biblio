use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::openai;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool};

use super::processing::ExtractedField;

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub enabled: bool,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub mode: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "http://localhost:11434/v1".to_string(),
            api_key: String::new(),
            model: "llama3.2".to_string(),
            mode: "metadata_only".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LlmFileMetadata {
    pub display_name: Option<String>,
    pub category: Option<String>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LlmNovelMetadata {
    pub display_name: Option<String>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub isbn: Option<String>,
    pub publisher: Option<String>,
    pub year: Option<String>,
    pub language: Option<String>,
    pub series: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LlmComicMetadata {
    pub display_name: Option<String>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub volume: Option<String>,
    pub series: Option<String>,
    pub issue_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmExtractedMetadata {
    pub display_name: Option<String>,
    pub category: Option<String>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    // Novel-specific
    pub isbn: Option<String>,
    pub publisher: Option<String>,
    pub year: Option<String>,
    pub language: Option<String>,
    pub series: Option<String>,
    // Comic-specific
    pub volume: Option<String>,
    pub issue_number: Option<String>,
}

impl LlmExtractedMetadata {
    fn from_generic(m: LlmFileMetadata) -> Self {
        Self {
            display_name: m.display_name,
            category: m.category,
            authors: m.authors,
            tags: m.tags,
            description: m.description,
            isbn: None,
            publisher: None,
            year: None,
            language: None,
            series: None,
            volume: None,
            issue_number: None,
        }
    }

    fn from_novel(m: LlmNovelMetadata) -> Self {
        Self {
            display_name: m.display_name,
            category: None,
            authors: m.authors,
            tags: m.tags,
            description: m.description,
            isbn: m.isbn,
            publisher: m.publisher,
            year: m.year,
            language: m.language,
            series: m.series,
            volume: None,
            issue_number: None,
        }
    }

    fn from_comic(m: LlmComicMetadata) -> Self {
        Self {
            display_name: m.display_name,
            category: None,
            authors: m.authors,
            tags: m.tags,
            description: m.description,
            isbn: None,
            publisher: None,
            year: None,
            language: None,
            series: m.series,
            volume: m.volume,
            issue_number: m.issue_number,
        }
    }
}

async fn read_setting(pool: &sqlx::SqlitePool, key: &str) -> Option<String> {
    let result: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = ?",
    )
    .bind(key)
    .fetch_optional(pool)
    .await
    .ok()?;
    result.map(|r| r.0)
}

async fn write_setting(pool: &sqlx::SqlitePool, key: &str, value: &str) -> Result<(), String> {
    sqlx::query("INSERT OR REPLACE INTO app_settings (key, value) VALUES (?, ?)")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn load_config(pool: &sqlx::SqlitePool) -> Result<LlmConfig, String> {
    let enabled = read_setting(pool, "llm_enabled")
        .await
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    let base_url = read_setting(pool, "llm_base_url")
        .await
        .unwrap_or_else(|| "http://localhost:11434/v1".to_string());

    let api_key = read_setting(pool, "llm_api_key").await.unwrap_or_default();

    let model = read_setting(pool, "llm_model")
        .await
        .unwrap_or_else(|| "llama3.2".to_string());

    let mode = read_setting(pool, "llm_mode")
        .await
        .unwrap_or_else(|| "metadata_only".to_string());

    Ok(LlmConfig {
        enabled,
        base_url,
        api_key,
        model,
        mode,
    })
}

#[tauri::command]
pub async fn llm_config_get(app: tauri::AppHandle) -> Result<LlmConfig, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    load_config(&pool).await
}

#[tauri::command]
pub async fn llm_config_set(app: tauri::AppHandle, config: LlmConfig) -> Result<(), String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    write_setting(&pool, "llm_enabled", &config.enabled.to_string()).await?;
    write_setting(&pool, "llm_base_url", &config.base_url).await?;
    write_setting(&pool, "llm_api_key", &config.api_key).await?;
    write_setting(&pool, "llm_model", &config.model).await?;
    write_setting(&pool, "llm_mode", &config.mode).await?;

    Ok(())
}

#[tauri::command]
pub async fn llm_test_connection(app: tauri::AppHandle) -> Result<String, String> {
    let config = llm_config_get(app).await?;

    let api_key = if config.api_key.is_empty() {
        "dummy".to_string()
    } else {
        config.api_key
    };

    let client = openai::Client::builder()
        .api_key(api_key)
        .base_url(&config.base_url)
        .build()
        .map_err(|e| format!("Failed to create LLM client: {e}"))?;

    let agent = client
        .agent(&config.model)
        .preamble("You are a helpful assistant. Respond with exactly: OK")
        .build();

    let response = agent
        .prompt("Say OK")
        .await
        .map_err(|e| format!("LLM connection test failed: {e}"))?;

    Ok(response)
}

pub async fn extract_metadata_with_llm(
    config: &LlmConfig,
    pool: &sqlx::SqlitePool,
    file_name: &str,
    existing_metadata: &[ExtractedField],
    file_content: Option<&str>,
    category: Option<&str>,
) -> Result<LlmExtractedMetadata, String> {
    let api_key = if config.api_key.is_empty() {
        "dummy".to_string()
    } else {
        config.api_key.clone()
    };

    let client = openai::Client::builder()
        .api_key(api_key)
        .base_url(&config.base_url)
        .build()
        .map_err(|e| format!("Failed to create LLM client: {e}"))?;

    let preamble = resolve_preamble(pool, category).await?;

    let mut input = format!("File name: {}\n", file_name);

    if !existing_metadata.is_empty() {
        input.push_str("Existing metadata:\n");
        for field in existing_metadata {
            input.push_str(&format!("  {}: {}\n", field.key, field.value));
        }
    }

    if let Some(content) = file_content {
        input.push_str(&format!("\nFile content preview:\n{}\n", content));
    }

    match category {
        Some("Novels") => {
            let extractor = client
                .extractor::<LlmNovelMetadata>(&config.model)
                .preamble(&preamble)
                .build();
            let result = extractor
                .extract(&input)
                .await
                .map_err(|e| format!("LLM extraction failed: {e}"))?;
            Ok(LlmExtractedMetadata::from_novel(result))
        }
        Some("Comics") => {
            let extractor = client
                .extractor::<LlmComicMetadata>(&config.model)
                .preamble(&preamble)
                .build();
            let result = extractor
                .extract(&input)
                .await
                .map_err(|e| format!("LLM extraction failed: {e}"))?;
            Ok(LlmExtractedMetadata::from_comic(result))
        }
        _ => {
            let extractor = client
                .extractor::<LlmFileMetadata>(&config.model)
                .preamble(&preamble)
                .build();
            let result = extractor
                .extract(&input)
                .await
                .map_err(|e| format!("LLM extraction failed: {e}"))?;
            Ok(LlmExtractedMetadata::from_generic(result))
        }
    }
}

async fn resolve_preamble(
    pool: &sqlx::SqlitePool,
    category: Option<&str>,
) -> Result<String, String> {
    if let Some(cat) = category {
        let db_prompt: Option<(String,)> = sqlx::query_as(
            "SELECT content FROM prompts WHERE category = ? AND is_default = 1 LIMIT 1",
        )
        .bind(cat)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

        if let Some(prompt) = db_prompt {
            return Ok(prompt.0);
        }
    }

    let db_prompt: Option<(String,)> = sqlx::query_as(
        "SELECT content FROM prompts WHERE category IS NULL AND is_default = 1 LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some(prompt) = db_prompt {
        return Ok(prompt.0);
    }

    Ok(fallback_preamble(category))
}

fn fallback_preamble(category: Option<&str>) -> String {
    match category {
        Some("Novels") => "You are a novel metadata extraction assistant. Given a file name, existing metadata, and optionally some file content (first pages of a book), extract structured metadata about this novel. Return a JSON object with these fields:
- display_name: the clean, full title of the novel
- authors: list of author names
- tags: relevant genre/theme tags
- description: a brief plot summary or description
- isbn: ISBN number if found
- publisher: publisher name
- year: year of publication
- language: the language the novel is written in
- series: series name if part of a series
Only fill in fields you can determine. Use null for unknown fields.".to_string(),
        Some("Comics") => "You are a comic/manga metadata extraction assistant. Given a file name and existing metadata, extract structured metadata about this comic. Return a JSON object with these fields:
- display_name: the clean, full title of the comic
- authors: list of author/artist names
- tags: relevant genre/theme tags
- description: a brief description
- volume: volume number if applicable
- series: series name
- issue_number: issue/chapter number
Only fill in fields you can determine. Use null for unknown fields.".to_string(),
        _ => "You are a file metadata extraction assistant. Given a file name, existing metadata, and optionally some file content, extract structured metadata. \
Return a JSON object with these fields: \
- display_name: a clean, human-readable title for the file \
- category: the most appropriate category (e.g. Novels, Comics, Documents, Academic, Music, Video, Other) \
- authors: a list of author names found \
- tags: a list of relevant tags/keywords \
- description: a brief description of the file content \
Only fill in fields you can determine from the provided information. Use null for fields you cannot determine.".to_string(),
    }
}
