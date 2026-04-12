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
pub struct LlmUnifiedMetadata {
    pub display_name: Option<String>,
    pub category: Option<String>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub progress: Option<String>,
    pub series: Option<String>,
    pub volume: Option<String>,
    pub issue_number: Option<String>,
    pub year: Option<String>,
    pub language: Option<String>,
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
    categories: &[String],
    tags: &[String],
    authors: &[String],
) -> Result<LlmUnifiedMetadata, String> {
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

    let preamble = resolve_preamble(pool, categories, tags, authors).await?;

    let mut input = format!("File name: {}\n", file_name);

    if !existing_metadata.is_empty() {
        input.push_str("Existing metadata:\n");
        for field in existing_metadata {
            input.push_str(&format!("  {}: {}\n", field.key, field.value));
        }
    }

    if let Some(content) = file_content {
        input.push_str(&format!("\nFile content samples:\n{}\n", content));
    }

    let extractor = client
        .extractor::<LlmUnifiedMetadata>(&config.model)
        .preamble(&preamble)
        .build();

    extractor
        .extract(&input)
        .await
        .map_err(|e| format!("LLM extraction failed: {e}"))
}

async fn resolve_preamble(
    pool: &sqlx::SqlitePool,
    categories: &[String],
    tags: &[String],
    authors: &[String],
) -> Result<String, String> {
    let db_prompt: Option<(String,)> = sqlx::query_as(
        "SELECT content FROM prompts WHERE is_default = 1 AND category IS NULL LIMIT 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let base = if let Some(prompt) = db_prompt {
        prompt.0
    } else {
        fallback_preamble()
    };

    let categories_str = if categories.is_empty() {
        "None defined yet".to_string()
    } else {
        categories.join(", ")
    };
    let tags_str = if tags.is_empty() {
        "None defined yet".to_string()
    } else {
        tags.join(", ")
    };
    let authors_str = if authors.is_empty() {
        "None known yet".to_string()
    } else {
        authors.join(", ")
    };

    Ok(format!(
        "{}\n\nAvailable categories: {}\nExisting tags: {}\nExisting authors: {}",
        base, categories_str, tags_str, authors_str
    ))
}

fn fallback_preamble() -> String {
    "You are a file metadata extraction assistant. Given a file name, raw metadata signals, \
and optionally sampled file content, extract structured metadata.\n\n\
Instructions:\n\
- Pick the most appropriate category from the available list\n\
- Prefer existing tags and authors when they match, but suggest new ones if needed\n\
- For novels/text files, extract reading progress if detectable (e.g. chapter number, 完结, 连载中)\n\
- Only fill in fields you can determine. Use null for unknown fields."
        .to_string()
}
