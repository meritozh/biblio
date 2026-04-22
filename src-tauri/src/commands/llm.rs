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

/// Comic-path Call 1: LLM ranks up to 5 filenames most likely to be the
/// cover image of an archive based on their names (no image data).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LlmCoverCandidates {
    /// Filenames chosen verbatim from the input, ranked best-first.
    pub candidates: Vec<String>,
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

/// Hard deadlines for a single LLM request. A hung local server (LM Studio
/// under load, mis-sized context, etc.) would otherwise stall the entire
/// import loop indefinitely — past the timeout we give up on this file and
/// let Phase 2 continue with its error handling.
const FILENAME_EXTRACTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const CONTENT_EXTRACTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);
const VISION_CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Prepended to every extraction preamble so model output stays in Simplified
/// Chinese regardless of which active prompt the user has chosen.
const LANGUAGE_INSTRUCTION: &str = "Output all Chinese text in Simplified Chinese (简体中文). Never use Traditional Chinese (繁體中文) characters.";

/// Call 1: Extract display_name, authors, progress from filename only.
/// Preamble is loaded from the active `filename` prompt in the DB.
pub async fn extract_filename_metadata(
    config: &LlmConfig,
    pool: &sqlx::SqlitePool,
    file_name: &str,
) -> Result<LlmFilenameMetadata, String> {
    let client = build_client(config)?;
    let user_preamble = crate::commands::prompts::prompt_get_active(pool, "filename").await?;
    let preamble = format!("{}\n\n{}", LANGUAGE_INSTRUCTION, user_preamble);

    let input = format!("File name: {}", file_name);

    let extractor = client
        .extractor::<LlmFilenameMetadata>(&config.model)
        .preamble(&preamble)
        .max_tokens(512)
        .build();

    match tokio::time::timeout(FILENAME_EXTRACTION_TIMEOUT, extractor.extract(&input)).await {
        Ok(result) => result.map_err(|e| format!("LLM filename extraction failed: {e}")),
        Err(_) => Err(format!(
            "LLM filename extraction timed out after {}s",
            FILENAME_EXTRACTION_TIMEOUT.as_secs()
        )),
    }
}

/// Call 2: Analyze content samples for classification.
/// Rules are loaded from the active `content` prompt in the DB; the
/// context header (available categories, existing tags) is prepended
/// here so the stored prompt can focus on rules only.
pub async fn extract_content_metadata(
    config: &LlmConfig,
    pool: &sqlx::SqlitePool,
    content: &str,
    display_name_hint: Option<&str>,
    categories: &[String],
    tags: &[String],
) -> Result<LlmContentMetadata, String> {
    let client = build_client(config)?;
    let rules = crate::commands::prompts::prompt_get_active(pool, "content").await?;

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
        "{}\n\n\
        Analyze the content samples to classify this novel.\n\
        Available categories: {}\n\
        Existing tags: {}\n\n\
        Rules:\n{}",
        LANGUAGE_INSTRUCTION, categories_str, tags_str, rules
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

    match tokio::time::timeout(CONTENT_EXTRACTION_TIMEOUT, extractor.extract(&input)).await {
        Ok(result) => result.map_err(|e| format!("LLM content analysis failed: {e}")),
        Err(_) => Err(format!(
            "LLM content analysis timed out after {}s",
            CONTENT_EXTRACTION_TIMEOUT.as_secs()
        )),
    }
}

/// Comic-path Call 1: given the list of image filenames inside an archive,
/// ask the LLM to rank up to 5 most likely to be the cover. The model is
/// explicitly allowed to return them in confidence order.
pub async fn extract_cover_candidates(
    config: &LlmConfig,
    entry_names: &[&str],
) -> Result<Vec<String>, String> {
    let client = build_client(config)?;

    let preamble = format!(
        "{}\n\n\
         You are picking the cover image of a comic archive given the list \
         of image filenames inside it. Rules:\n\
         - Return up to 5 filenames from the input list, ordered best-first.\n\
         - The first file in the input (often named 000.jpg, 001.png, cover.jpg, \
           cover01.jpg) is usually the cover — rank it first when plausible.\n\
         - Return the filenames verbatim — do not invent entries.\n\
         - If nothing looks like a cover, return an empty list.",
        LANGUAGE_INSTRUCTION
    );

    let input = format!("Filenames (archive order):\n{}", entry_names.join("\n"));

    let extractor = client
        .extractor::<LlmCoverCandidates>(&config.model)
        .preamble(&preamble)
        .max_tokens(512)
        .build();

    match tokio::time::timeout(FILENAME_EXTRACTION_TIMEOUT, extractor.extract(&input)).await {
        Ok(result) => result
            .map(|r| r.candidates)
            .map_err(|e| format!("LLM cover-candidates failed: {e}")),
        Err(_) => Err(format!(
            "LLM cover-candidates timed out after {}s",
            FILENAME_EXTRACTION_TIMEOUT.as_secs()
        )),
    }
}

/// Comic-path Call 2: ask the LLM whether a given image is a cover. Uses a
/// raw OpenAI-compatible multimodal POST because `rig`'s extractor is
/// text-only; any OpenAI-compatible endpoint with a multimodal model will
/// accept this format. Returns `true` / `false` / `Err` (text-only model
/// or network failure → caller should fall back to the first candidate).
pub async fn check_is_cover(
    config: &LlmConfig,
    image_bytes: &[u8],
    mime_type: &str,
) -> Result<bool, String> {
    use base64::Engine;

    let data_url = format!(
        "data:{};base64,{}",
        mime_type,
        base64::engine::general_purpose::STANDARD.encode(image_bytes)
    );

    // Endpoint: {base_url}/chat/completions. `base_url` already ends in /v1 in
    // typical configs so we avoid double-slashing by trimming trailing /.
    let base = config.base_url.trim_end_matches('/');
    let endpoint = format!("{base}/chat/completions");

    let body = serde_json::json!({
        "model": config.model,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": "Is this image the cover of a book or comic? Reply strictly as JSON: {\"is_cover\": true} or {\"is_cover\": false}. Covers typically show the title, art prominently featuring the main subject, and no panel/page numbers."
                },
                {
                    "type": "image_url",
                    "image_url": {"url": data_url}
                }
            ]
        }],
        "max_tokens": 64,
        "response_format": {"type": "json_object"}
    });

    let mut req = reqwest::Client::new().post(&endpoint).json(&body);
    if !config.api_key.is_empty() {
        req = req.bearer_auth(&config.api_key);
    }

    let fut = async move {
        let resp = req
            .send()
            .await
            .map_err(|e| format!("Vision request failed: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            let err = resp.text().await.unwrap_or_default();
            return Err(format!("Vision endpoint returned {status}: {err}"));
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Vision response parse failed: {e}"))?;
        let content = json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| "Vision response missing choices[0].message.content".to_string())?;

        let parsed: serde_json::Value = serde_json::from_str(content)
            .map_err(|e| format!("Vision content is not valid JSON: {e} — got: {content}"))?;
        let is_cover = parsed
            .get("is_cover")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| {
                format!("Vision response missing 'is_cover' bool field — got: {content}")
            })?;
        Ok::<bool, String>(is_cover)
    };

    match tokio::time::timeout(VISION_CALL_TIMEOUT, fut).await {
        Ok(r) => r,
        Err(_) => Err(format!(
            "LLM vision check timed out after {}s",
            VISION_CALL_TIMEOUT.as_secs()
        )),
    }
}

