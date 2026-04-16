use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::openai;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool};

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
    pub analyze_content: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "http://localhost:1234/v1".to_string(),
            api_key: String::new(),
            model: "gemma-4".to_string(),
            analyze_content: true,
        }
    }
}

/// Schema for Call 1: extract metadata from filename only
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LlmFilenameMetadata {
    /// Clean title extracted from filename
    pub display_name: Option<String>,
    /// Author names from filename patterns (e.g. "作者：xxx" or "xxx - title")
    pub authors: Vec<String>,
    /// Reading progress from filename, e.g. "第1-45章 未完结", "完结", "连载中"
    pub progress: Option<String>,
}

/// Schema for Call 2: analyze content samples for classification
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LlmContentMetadata {
    /// Category from the available list
    pub category: Option<String>,
    /// Genre/theme tags
    pub tags: Vec<String>,
    /// Brief description based on content
    pub description: Option<String>,
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
        .unwrap_or_else(|| "http://localhost:1234/v1".to_string());

    let api_key = read_setting(pool, "llm_api_key").await.unwrap_or_default();

    let model = read_setting(pool, "llm_model")
        .await
        .unwrap_or_else(|| "gemma-4".to_string());

    let analyze_content = read_setting(pool, "llm_analyze_content")
        .await
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(true);

    Ok(LlmConfig {
        enabled,
        base_url,
        api_key,
        model,
        analyze_content,
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
    write_setting(&pool, "llm_analyze_content", &config.analyze_content.to_string()).await?;

    Ok(())
}

fn build_client(config: &LlmConfig) -> Result<openai::CompletionsClient, String> {
    let api_key = if config.api_key.is_empty() {
        "dummy".to_string()
    } else {
        config.api_key.clone()
    };

    openai::CompletionsClient::builder()
        .api_key(api_key)
        .base_url(&config.base_url)
        .build()
        .map_err(|e| format!("Failed to create LLM client: {e}"))
}

#[tauri::command]
pub async fn llm_test_connection(app: tauri::AppHandle) -> Result<String, String> {
    let config = llm_config_get(app).await?;
    let client = build_client(&config)?;

    let agent = client
        .agent(&config.model)
        .preamble("Respond with exactly: OK")
        .max_tokens(64)
        .build();

    let response: String = agent
        .prompt("Say OK")
        .await
        .map_err(|e| format!("LLM connection test failed: {e}"))?;

    Ok(response)
}

/// Call 1: Extract display_name, authors, progress from filename only.
/// No DB context needed — filename parsing is structurally deterministic.
pub async fn extract_filename_metadata(
    config: &LlmConfig,
    file_name: &str,
) -> Result<LlmFilenameMetadata, String> {
    let client = build_client(config)?;

    let preamble = "Extract metadata from this filename only. Rules:\n\
        - display_name: the clean title (remove site prefixes like [sxsy.org], brackets, file extension)\n\
        - authors: if filename has \"作者：xxx\" or \"xxx - title\" pattern, extract the author\n\
        - progress: combine chapter range + status, e.g. \"第1-45章 未完结\", \"完结\", \"连载中\"\n\
        - Use null for unknown fields";

    let input = format!("File name: {}", file_name);

    let extractor = client
        .extractor::<LlmFilenameMetadata>(&config.model)
        .preamble(preamble)
        .max_tokens(512)
        .build();

    extractor
        .extract(&input)
        .await
        .map_err(|e| format!("LLM filename extraction failed: {e}"))
}

/// Call 2: Analyze content samples for classification.
/// Takes the display_name hint from Call 1 and DB context for categories/tags.
pub async fn extract_content_metadata(
    config: &LlmConfig,
    content: &str,
    display_name_hint: Option<&str>,
    categories: &[String],
    tags: &[String],
) -> Result<LlmContentMetadata, String> {
    let client = build_client(config)?;

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

    let preamble = format!(
        "Analyze the content samples to classify this novel.\n\
        Available categories: {}\n\
        Existing tags: {}\n\n\
        Rules:\n\
        - category: return ONLY the category name. If a category is shown as \"name (description)\", the parenthesized text is just a hint — return \"name\" without the parentheses or description. Example: for \"h-novel (novel with sexual content)\", return \"h-novel\".\n\
        - tags: prefer existing tags, suggest new ones if needed\n\
        - description: 1-2 sentence plot summary based on content\n\
        - Use null for unknown fields",
        categories_str, tags_str
    );

    let mut input = String::new();
    if let Some(name) = display_name_hint {
        input.push_str(&format!("Title: {}\n\n", name));
    }
    input.push_str("File content samples:\n");
    input.push_str(content);

    let extractor = client
        .extractor::<LlmContentMetadata>(&config.model)
        .preamble(&preamble)
        .max_tokens(1024)
        .build();

    extractor
        .extract(&input)
        .await
        .map_err(|e| format!("LLM content analysis failed: {e}"))
}

