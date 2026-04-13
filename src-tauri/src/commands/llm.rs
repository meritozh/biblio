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
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: "http://localhost:1234/v1".to_string(),
            api_key: String::new(),
            model: "mistral-small-24b-instruct".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
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

// OpenAI-compatible API types
#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    response_format: ResponseFormat,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: Option<String>,
}

fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap_or_default()
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
        .unwrap_or_else(|| "mistral-small-24b-instruct".to_string());

    Ok(LlmConfig {
        enabled,
        base_url,
        api_key,
        model,
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

    Ok(())
}

#[tauri::command]
pub async fn llm_test_connection(app: tauri::AppHandle) -> Result<String, String> {
    let config = llm_config_get(app).await?;

    let client = build_http_client();
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let request = ChatRequest {
        model: config.model,
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: "Respond with exactly: OK".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "Say OK".to_string(),
            },
        ],
        temperature: 0.1,
        response_format: ResponseFormat {
            format_type: "text".to_string(),
        },
    };

    let mut req = client.post(&url).json(&request);
    if !config.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", config.api_key));
    }

    let response = req.send().await
        .map_err(|e| format!("Connection failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("LLM returned {}: {}", status, body));
    }

    let chat_response: ChatResponse = response.json().await
        .map_err(|e| format!("Failed to parse response: {e}"))?;

    let content = chat_response.choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .unwrap_or("No response")
        .to_string();

    Ok(content)
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
    let preamble = resolve_preamble(pool, categories, tags, authors).await?;

    let mut user_input = format!("File name: {}\n", file_name);

    if !existing_metadata.is_empty() {
        user_input.push_str("Existing metadata:\n");
        for field in existing_metadata {
            user_input.push_str(&format!("  {}: {}\n", field.key, field.value));
        }
    }

    if let Some(content) = file_content {
        user_input.push_str(&format!("\nFile content samples:\n{}\n", content));
    }

    user_input.push_str("\nRespond with a JSON object containing these fields: display_name, category, authors (array), tags (array), description, progress, series, volume, issue_number, year, language. Use null for unknown fields.");

    let client = build_http_client();
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let request = ChatRequest {
        model: config.model.clone(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: preamble,
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_input,
            },
        ],
        temperature: 0.1,
        response_format: ResponseFormat {
            format_type: "json_object".to_string(),
        },
    };

    let mut req = client.post(&url).json(&request);
    if !config.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", config.api_key));
    }

    let response = req.send().await
        .map_err(|e| format!("LLM request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("LLM returned {}: {}", status, body));
    }

    let chat_response: ChatResponse = response.json().await
        .map_err(|e| format!("Failed to parse LLM response: {e}"))?;

    let content = chat_response.choices
        .first()
        .and_then(|c| c.message.content.as_deref())
        .ok_or("LLM returned empty response")?;

    // Parse JSON from the response, handling possible markdown code blocks
    let json_str = content.trim();
    let json_str = if json_str.starts_with("```") {
        // Strip markdown code block
        let without_prefix = json_str
            .trim_start_matches("```json")
            .trim_start_matches("```");
        without_prefix.trim_end_matches("```").trim()
    } else {
        json_str
    };

    serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse LLM JSON output: {e}\nRaw: {}", json_str))
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
- Only fill in fields you can determine. Use null for unknown fields.\n\
- Always respond with valid JSON."
        .to_string()
}
