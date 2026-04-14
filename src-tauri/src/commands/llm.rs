use serde::{Deserialize, Serialize};
use serde_json::json;
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
            model: "llama3.2".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LlmUnifiedMetadata {
    pub display_name: Option<String>,
    pub category: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub progress: Option<String>,
    pub series: Option<String>,
    pub volume: Option<String>,
    pub issue_number: Option<String>,
    pub year: Option<String>,
    pub language: Option<String>,
}

// Chat Completions API types
#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<ToolCallResponse>>,
}

#[derive(Deserialize)]
struct ToolCallResponse {
    function: ToolCallFunction,
}

#[derive(Deserialize)]
struct ToolCallFunction {
    arguments: String,
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
        .unwrap_or_else(|| "llama3.2".to_string());

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

/// Send a request to the LLM and extract the response text from any field
async fn send_llm_request(
    config: &LlmConfig,
    body: serde_json::Value,
) -> Result<ChatResponseMessage, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

    let mut req = client.post(&url).json(&body);
    if !config.api_key.is_empty() {
        req = req.bearer_auth(&config.api_key);
    }

    let response = req
        .send()
        .await
        .map_err(|e| format!("LLM request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("LLM returned {status}: {body}"));
    }

    let chat_response: ChatResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse LLM response: {e}"))?;

    chat_response
        .choices
        .into_iter()
        .next()
        .map(|c| c.message)
        .ok_or_else(|| "LLM returned no choices".to_string())
}

#[tauri::command]
pub async fn llm_test_connection(app: tauri::AppHandle) -> Result<String, String> {
    let config = llm_config_get(app).await?;

    let body = json!({
        "model": config.model,
        "messages": [
            {"role": "system", "content": "Respond with exactly: OK"},
            {"role": "user", "content": "Say OK"}
        ],
        "temperature": 0.0,
        "max_tokens": 64
    });

    let msg = send_llm_request(&config, body).await?;

    msg.content
        .filter(|s| !s.trim().is_empty())
        .or(msg.reasoning_content)
        .ok_or_else(|| "LLM returned empty response".to_string())
}

/// The tool schema that tells the model exactly what JSON structure to produce
fn metadata_tool_schema() -> serde_json::Value {
    json!([{
        "type": "function",
        "function": {
            "name": "submit",
            "description": "Submit the extracted file metadata",
            "parameters": {
                "type": "object",
                "properties": {
                    "display_name": {"type": ["string", "null"], "description": "Clean title"},
                    "category": {"type": ["string", "null"], "description": "Pick from available categories"},
                    "authors": {"type": "array", "items": {"type": "string"}},
                    "tags": {"type": "array", "items": {"type": "string"}},
                    "description": {"type": ["string", "null"], "description": "Brief description"},
                    "progress": {"type": ["string", "null"], "description": "Reading progress, e.g. chapter number, 完结, 连载中"},
                    "series": {"type": ["string", "null"]},
                    "volume": {"type": ["string", "null"]},
                    "issue_number": {"type": ["string", "null"]},
                    "year": {"type": ["string", "null"]},
                    "language": {"type": ["string", "null"]}
                },
                "required": ["display_name", "category", "authors", "tags"]
            }
        }
    }])
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

    let body = json!({
        "model": config.model,
        "messages": [
            {"role": "system", "content": preamble},
            {"role": "user", "content": user_input}
        ],
        "tools": metadata_tool_schema(),
        "tool_choice": {"type": "function", "function": {"name": "submit"}},
        "temperature": 0.0,
        "max_tokens": 2048
    });

    let msg = send_llm_request(&config, body).await?;

    // Strategy 1: Proper tool_calls array (well-behaved models)
    if let Some(tool_calls) = &msg.tool_calls {
        if let Some(tc) = tool_calls.first() {
            return serde_json::from_str(&tc.function.arguments)
                .map_err(|e| format!("Failed to parse tool call arguments: {e}"));
        }
    }

    // Strategy 2: Tool call in reasoning_content (reasoning models like QwQ)
    if let Some(reasoning) = &msg.reasoning_content {
        let json_str = extract_json(reasoning);
        if !json_str.is_empty() {
            if let Ok(metadata) = serde_json::from_str::<LlmUnifiedMetadata>(&json_str) {
                return Ok(metadata);
            }
        }
    }

    // Strategy 3: JSON in content field
    if let Some(content) = &msg.content {
        if !content.trim().is_empty() {
            let json_str = extract_json(content);
            if !json_str.is_empty() {
                return serde_json::from_str(&json_str)
                    .map_err(|e| format!("Failed to parse LLM JSON: {e}\nRaw: {content}"));
            }
        }
    }

    Err(format!(
        "LLM returned no extractable data. content={:?}, reasoning={}, tool_calls={}",
        msg.content,
        msg.reasoning_content.as_deref().map(|s| &s[..s.len().min(200)]).unwrap_or("none"),
        msg.tool_calls.as_ref().map(|t| t.len()).unwrap_or(0)
    ))
}

/// Extract JSON from text, handling <tool_call> tags, code blocks, and plain JSON
fn extract_json(text: &str) -> String {
    let trimmed = text.trim();

    // Try <tool_call> tags (reasoning models trained on tool-calling)
    if let Some(start) = trimmed.find("<tool_call>") {
        let inner_start = start + 11;
        let inner_end = trimmed[inner_start..].find("</tool_call>")
            .map(|e| inner_start + e)
            .unwrap_or(trimmed.len());
        let tool_json = trimmed[inner_start..inner_end].trim();
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(tool_json) {
            if let Some(args) = val.get("arguments") {
                return args.to_string();
            }
        }
        return tool_json.to_string();
    }

    // Try ```json ... ``` blocks
    if let Some(start) = trimmed.find("```json") {
        let json_start = start + 7;
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim().to_string();
        }
    }

    // Try ``` ... ``` blocks
    if let Some(start) = trimmed.find("```") {
        let json_start = start + 3;
        let json_start = trimmed[json_start..]
            .find('\n')
            .map(|n| json_start + n + 1)
            .unwrap_or(json_start);
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim().to_string();
        }
    }

    // Try to find a JSON object directly
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            return trimmed[start..=end].to_string();
        }
    }

    trimmed.to_string()
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
    "Extract file metadata. Pick category from the list. Use existing tags/authors when possible. Extract progress if present (chapter number, 完结, 连载中). Use null for unknown fields.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_plain() {
        let input = r#"{"display_name": "test", "authors": []}"#;
        assert_eq!(extract_json(input), input);
    }

    #[test]
    fn test_extract_json_code_block() {
        let input = "Here is the result:\n```json\n{\"display_name\": \"test\"}\n```\nDone.";
        assert_eq!(extract_json(input), "{\"display_name\": \"test\"}");
    }

    #[test]
    fn test_extract_json_tool_call_tags() {
        let input = "<tool_call>\n{\"name\": \"submit\", \"arguments\": {\"display_name\": \"test\", \"authors\": [\"Author\"]}}\n</tool_call>";
        let result = extract_json(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["display_name"], "test");
        assert_eq!(parsed["authors"][0], "Author");
    }

    #[test]
    fn test_extract_json_with_extra_text() {
        let input = "Sure! Here is the metadata:\n{\"display_name\": \"test\", \"authors\": []}";
        assert_eq!(
            extract_json(input),
            "{\"display_name\": \"test\", \"authors\": []}"
        );
    }
}
