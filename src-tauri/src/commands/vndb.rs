//! Tauri commands bridging the galgame import UI to the VNDB provider.
//!
//! `vndb_search` is fired by the review dialog when a galgame row appears
//! (seeded with the LLM-cleaned name) and again on manual re-search.
//! `vndb_fetch_cover` pulls the chosen candidate's cover bytes so the commit
//! reuses the existing inline-`cover_data` path — no new cover-staging
//! machinery.

use crate::providers::vndb::{self, VndbCandidate};

/// Search VNDB for visual novels matching `query`. Returns up to 10 candidates
/// best-first. Errors surface to the UI, which falls back to manual entry.
#[tauri::command]
pub async fn vndb_search(query: String) -> Result<Vec<VndbCandidate>, String> {
    vndb::search(&query).await
}

/// Cover bytes for a chosen VNDB candidate, base64-encoded with the mime type
/// — the same shape `cover_get` returns, so the form drops it straight into
/// `cover_data` and the existing commit path compresses + stores it.
#[derive(serde::Serialize)]
pub struct VndbCover {
    pub data: String,
    pub mime_type: String,
}

#[tauri::command]
pub async fn vndb_fetch_cover(url: String) -> Result<VndbCover, String> {
    use base64::Engine;
    let (bytes, mime_type) = vndb::fetch_cover(&url).await?;
    Ok(VndbCover {
        data: base64::engine::general_purpose::STANDARD.encode(&bytes),
        mime_type,
    })
}
