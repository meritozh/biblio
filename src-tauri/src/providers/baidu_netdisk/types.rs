use serde::{Deserialize, Serialize};

/// Surface for errors during a Baidu API call. String-typed (same as the
/// rest of biblio) so the frontend gets a human-readable message; the
/// caller decides what to do with specific failures.
#[derive(Debug, Clone)]
pub struct BaiduError(pub String);

impl std::fmt::Display for BaiduError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for BaiduError {
    fn from(s: String) -> Self {
        BaiduError(s)
    }
}

impl From<&str> for BaiduError {
    fn from(s: &str) -> Self {
        BaiduError(s.to_string())
    }
}

impl From<reqwest::Error> for BaiduError {
    fn from(e: reqwest::Error) -> Self {
        BaiduError(format!("HTTP error: {e}"))
    }
}

impl From<std::io::Error> for BaiduError {
    fn from(e: std::io::Error) -> Self {
        BaiduError(format!("I/O error: {e}"))
    }
}

/// Result of a successful `upload_file` call. Fields match what the
/// frontend and command layer need to persist in the `files` row.
#[derive(Debug, Clone, Serialize)]
pub struct UploadResult {
    pub fs_id: String,
    pub md5: String,
    pub size: i64,
    pub path: String,
}

// ── Internal wire types ─────────────────────────────────────────────

/// Response from `/xpan/file?method=precreate`.
#[derive(Debug, Deserialize)]
pub(super) struct PrecreateResponse {
    pub errno: i32,
    pub path: Option<String>,
    pub uploadid: Option<String>,
    pub return_type: Option<i32>,
    // Echoed back by Baidu's precreate but biblio doesn't consume it
    // (we already track the block layout client-side). Keep for wire-
    // format fidelity in case a future caller needs it.
    #[allow(dead_code)]
    #[serde(default)]
    pub block_list: Vec<u32>,
}

/// Response from `/pcs/superfile2?method=upload`.
#[derive(Debug, Deserialize)]
pub(super) struct SliceUploadResponse {
    pub md5: Option<String>,
    /// The `errno` field only appears on failure; success returns just md5.
    #[serde(default)]
    pub errno: Option<i32>,
    #[serde(default)]
    pub error_msg: Option<String>,
}

/// Response from `/xpan/file?method=create`.
#[derive(Debug, Deserialize)]
pub(super) struct CreateResponse {
    pub errno: i32,
    pub fs_id: Option<serde_json::Value>,
    pub md5: Option<String>,
    pub size: Option<i64>,
    pub path: Option<String>,
}

/// Response from `/xpan/file?method=filemanager&opera=delete`.
#[derive(Debug, Deserialize)]
pub(super) struct FileManagerResponse {
    pub errno: i32,
    #[serde(default)]
    #[allow(dead_code)]
    pub info: Option<serde_json::Value>,
}

/// Single entry in `/xpan/multimedia?method=filemetas` response. We only
/// read `dlink`; `fs_id` and `size` are present too but unused here.
#[derive(Debug, Deserialize)]
pub(super) struct FilemetasEntry {
    pub dlink: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct FilemetasResponse {
    pub errno: i32,
    #[serde(default)]
    pub list: Vec<FilemetasEntry>,
}
