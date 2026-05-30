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
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub display_name: Option<String>,
    /// Author names from filename patterns (e.g. "作者：xxx" or "xxx - title")
    #[serde(default, deserialize_with = "lenient_string_vec")]
    pub authors: Vec<String>,
    /// Reading progress from filename, e.g. "第1-45章 未完结", "完结", "连载中"
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub progress: Option<String>,
}

/// Schema for Call 2: analyze content samples for classification
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LlmContentMetadata {
    /// Category from the available list
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub category: Option<String>,
    /// Genre/theme tags
    #[serde(default, deserialize_with = "lenient_string_vec")]
    pub tags: Vec<String>,
}

/// Comic-path Call 1: LLM ranks up to 5 filenames most likely to be the
/// cover image of an archive based on their names (no image data).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LlmCoverCandidates {
    /// Filenames chosen verbatim from the input, ranked best-first.
    #[serde(default, deserialize_with = "lenient_string_vec")]
    pub candidates: Vec<String>,
}

/// Coerce `null`, missing keys, non-string values, and empty strings to
/// `None`. The LLM extractor's strict deserializer otherwise rejects an
/// entire response when one nullable field comes back as JSON `null` — a
/// common shape for prompts where the model has nothing to extract.
fn lenient_opt_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    Ok(match v {
        serde_json::Value::String(s) if !s.trim().is_empty() => Some(s),
        _ => None,
    })
}

/// Coerce `null`, missing keys, non-array values, and `null`/empty
/// elements to a clean `Vec<String>`. A single `null` element inside an
/// `authors`/`tags` array would otherwise sink the whole extraction.
fn lenient_string_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    Ok(match v {
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .filter_map(|item| match item {
                serde_json::Value::String(s) if !s.trim().is_empty() => Some(s),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    })
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

    let response: String = match tokio::time::timeout(LLM_REQUEST_TIMEOUT, agent.prompt("Say OK")).await {
        Ok(result) => result.map_err(|e| format!("LLM connection test failed: {e}"))?,
        Err(_) => {
            return Err(format!(
                "LLM connection test timed out after {}s",
                LLM_REQUEST_TIMEOUT.as_secs()
            ));
        }
    };

    Ok(response)
}

/// Hard deadline for a single LLM request. A hung local server (LM Studio
/// under load, a tool-calling model that returns `stop_reason: tool_use`
/// and waits forever for a tool result we never send, etc.) would otherwise
/// stall the entire import loop indefinitely — past the timeout we give up
/// on this file and let Phase 2 continue with its error handling.
const LLM_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Prepended to every extraction preamble so model output stays in Simplified
/// Chinese regardless of which active prompt the user has chosen.
const LANGUAGE_INSTRUCTION: &str = "Output all Chinese text in Simplified Chinese (简体中文). Never use Traditional Chinese (繁體中文) characters.";

/// Call 1: Extract display_name, authors, progress from filename only.
/// Preamble is loaded from the active `(schema_slug, step)` prompt in
/// the DB. Step is normally `"filename"`; the comic pipeline passes
/// `"filename_folder"` instead when the source is an image folder so
/// the LLM uses different rules (folder name doesn't carry the author).
pub async fn extract_filename_metadata(
    config: &LlmConfig,
    pool: &sqlx::SqlitePool,
    file_name: &str,
    schema_slug: crate::schema::SchemaSlug,
    step: &str,
) -> Result<LlmFilenameMetadata, String> {
    let client = build_client(config)?;
    let user_preamble =
        crate::commands::prompts::prompt_get_active(pool, schema_slug, step).await?;
    let preamble = format!("{}\n\n{}", LANGUAGE_INSTRUCTION, user_preamble);

    let input = format!("File name: {}", file_name);

    let extractor = client
        .extractor::<LlmFilenameMetadata>(&config.model)
        .preamble(&preamble)
        .max_tokens(512)
        .build();

    match tokio::time::timeout(LLM_REQUEST_TIMEOUT, extractor.extract(&input)).await {
        Ok(result) => result.map_err(|e| format!("LLM filename extraction failed: {e}")),
        Err(_) => Err(format!(
            "LLM filename extraction timed out after {}s",
            LLM_REQUEST_TIMEOUT.as_secs()
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
    let rules = crate::commands::prompts::prompt_get_active(
        pool,
        crate::schema::SchemaSlug::Novel,
        "content",
    )
    .await?;

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

    match tokio::time::timeout(LLM_REQUEST_TIMEOUT, extractor.extract(&input)).await {
        Ok(result) => result.map_err(|e| format!("LLM content analysis failed: {e}")),
        Err(_) => Err(format!(
            "LLM content analysis timed out after {}s",
            LLM_REQUEST_TIMEOUT.as_secs()
        )),
    }
}

/// Comic-path Call 1: given the list of image filenames inside an archive,
/// ask the LLM to rank up to 5 most likely to be the cover. Rules come
/// from the active `(archive, cover_pick)` prompt so the user can tune
/// the heuristics without recompiling.
pub async fn extract_cover_candidates(
    config: &LlmConfig,
    pool: &sqlx::SqlitePool,
    entry_names: &[&str],
) -> Result<Vec<String>, String> {
    let client = build_client(config)?;
    let user_preamble = crate::commands::prompts::prompt_get_active(
        pool,
        crate::schema::SchemaSlug::Comic,
        "cover_pick",
    )
    .await?;
    let preamble = format!("{}\n\n{}", LANGUAGE_INSTRUCTION, user_preamble);

    let input = format!("Filenames (archive order):\n{}", entry_names.join("\n"));

    let extractor = client
        .extractor::<LlmCoverCandidates>(&config.model)
        .preamble(&preamble)
        .max_tokens(512)
        .build();

    match tokio::time::timeout(LLM_REQUEST_TIMEOUT, extractor.extract(&input)).await {
        Ok(result) => result
            .map(|r| r.candidates)
            .map_err(|e| format!("LLM cover-candidates failed: {e}")),
        Err(_) => Err(format!(
            "LLM cover-candidates timed out after {}s",
            LLM_REQUEST_TIMEOUT.as_secs()
        )),
    }
}

/// Schema for the comic-path vision call: a single boolean from the LLM.
/// Rig serializes this as `response_format: {type: "json_schema", ...}`
/// via the OpenAI provider — matching what every other LLM call here
/// already does, and what most OpenAI-compatible endpoints (DashScope,
/// Doubao, OpenRouter, vLLM, llama.cpp) accept.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct LlmIsCover {
    /// True if the image looks like a book / comic cover (prominent title,
    /// main subject, no page or panel numbers).
    pub is_cover: bool,
}

/// Map our existing `image/...` mime strings onto rig's `ImageMediaType`.
/// Defaults to JPEG when the extension is unknown — most comic dumps use
/// JPEG, and providers that need an explicit type to accept a base64
/// image will treat JPEG as a safe fallback.
fn image_media_type_for(mime: &str) -> rig::completion::message::ImageMediaType {
    use rig::completion::message::ImageMediaType;
    match mime.to_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => ImageMediaType::JPEG,
        "image/png" => ImageMediaType::PNG,
        "image/webp" => ImageMediaType::WEBP,
        "image/gif" => ImageMediaType::GIF,
        _ => ImageMediaType::JPEG,
    }
}

/// Comic-path Call 2: ask the LLM whether a given image is a cover. Goes
/// through `client.extractor()` so the request shape (json_schema,
/// schema-validated retry) matches the other three extractor calls in
/// this module. Returns `true` / `false` / `Err` — caller falls back to
/// the first candidate on Err (text-only model, endpoint refusal, etc.).
pub async fn check_is_cover(
    config: &LlmConfig,
    image_bytes: &[u8],
    mime_type: &str,
) -> Result<bool, String> {
    use base64::Engine;
    use rig::OneOrMany;
    use rig::completion::Message;
    use rig::completion::message::UserContent;

    let preamble = format!(
        "{}\n\n{}",
        LANGUAGE_INSTRUCTION,
        "Decide whether the supplied image is the cover of a book or comic. \
         Covers typically show the title prominently and feature the main \
         subject without panel or page numbers. Reply with the structured \
         schema only."
    );

    // Send the image as a `data:` URL via `image_url`. Rig's OpenAI
    // provider strictly requires `ImageDetail` for the `image_base64`
    // path (providers/openai/completion/mod.rs:473) but applies a default
    // for the URL path (line 459). The wire payload is identical — both
    // serialize to `{type:"image_url", image_url:{url:"data:..."}}`.
    let data_url = format!(
        "data:{};base64,{}",
        mime_type,
        base64::engine::general_purpose::STANDARD.encode(image_bytes)
    );
    let contents = OneOrMany::many([
        UserContent::text("Is this image a cover?"),
        UserContent::image_url(data_url, Some(image_media_type_for(mime_type)), None),
    ])
    .map_err(|e| format!("Vision message build failed: {e}"))?;
    let message: Message = contents.into();

    let client = build_client(config)?;
    let extractor = client
        .extractor::<LlmIsCover>(&config.model)
        .preamble(&preamble)
        .max_tokens(64)
        .build();

    match tokio::time::timeout(LLM_REQUEST_TIMEOUT, extractor.extract(message)).await {
        Ok(result) => result
            .map(|r| r.is_cover)
            .map_err(|e| format!("LLM vision check failed: {e}")),
        Err(_) => Err(format!(
            "LLM vision check timed out after {}s",
            LLM_REQUEST_TIMEOUT.as_secs()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: a single null element inside `authors` used to fail the
    /// whole extraction with "invalid type: null, expected a string",
    /// causing the folder-name cleanup to fall back to the raw `[作者]`.
    #[test]
    fn filename_metadata_tolerates_null_array_element() {
        let raw = r#"{"display_name":"系列","authors":[null,"作者"],"progress":null}"#;
        let meta: LlmFilenameMetadata = serde_json::from_str(raw).expect("should deserialize");
        assert_eq!(meta.display_name.as_deref(), Some("系列"));
        assert_eq!(meta.authors, vec!["作者".to_string()]);
        assert!(meta.progress.is_none());
    }

    #[test]
    fn filename_metadata_tolerates_null_optional_string() {
        let raw = r#"{"display_name":null,"authors":["作者"],"progress":null}"#;
        let meta: LlmFilenameMetadata = serde_json::from_str(raw).expect("should deserialize");
        assert!(meta.display_name.is_none());
        assert_eq!(meta.authors, vec!["作者".to_string()]);
    }

    #[test]
    fn filename_metadata_tolerates_missing_fields() {
        let raw = r#"{"authors":["作者"]}"#;
        let meta: LlmFilenameMetadata = serde_json::from_str(raw).expect("should deserialize");
        assert!(meta.display_name.is_none());
        assert!(meta.progress.is_none());
        assert_eq!(meta.authors, vec!["作者".to_string()]);
    }

    #[test]
    fn filename_metadata_tolerates_null_array() {
        let raw = r#"{"display_name":"系列","authors":null}"#;
        let meta: LlmFilenameMetadata = serde_json::from_str(raw).expect("should deserialize");
        assert!(meta.authors.is_empty());
    }

    #[test]
    fn filename_metadata_strips_empty_strings() {
        let raw = r#"{"display_name":"  ","authors":["","作者","   "],"progress":""}"#;
        let meta: LlmFilenameMetadata = serde_json::from_str(raw).expect("should deserialize");
        assert!(meta.display_name.is_none());
        assert!(meta.progress.is_none());
        assert_eq!(meta.authors, vec!["作者".to_string()]);
    }
}
