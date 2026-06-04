//! VNDB (Visual Novel Database) Kana API client used by the galgame import
//! flow to autofill metadata.
//!
//! Read-only queries need no authentication. We use a single POST to
//! `https://api.vndb.org/kana/vn` with a `["search","=",<title>]` filter and
//! ask for just the fields the galgame schema consumes: origin title (from
//! `alttitle`, falling back to the romanized `title`), the cover image URL,
//! and the developer name (→ author).
//!
//! Rate limit (per the API docs): 200 requests / 5 minutes and ~1s of
//! execution time per minute. The import flow issues one search per game on
//! demand, well under that ceiling. Failures degrade gracefully — the caller
//! shows an empty candidate list and the user types metadata by hand.

use serde::{Deserialize, Serialize};

const VNDB_VN_ENDPOINT: &str = "https://api.vndb.org/kana/vn";

/// Hard ceiling on a single VNDB request so a hung connection can't stall the
/// import review. The API itself aborts requests over 3s; give it a little
/// headroom for transport.
const VNDB_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// One candidate visual novel surfaced to the review UI. Serializes to the
/// frontend so the user can pick the right match.
#[derive(Debug, Clone, Serialize)]
pub struct VndbCandidate {
    /// vndbid (e.g. "v17"). Stable key for the picked match.
    pub id: String,
    /// Romanized main title — always present, shown as the secondary line.
    pub title: String,
    /// Original-script title (`alttitle`). The galgame schema treats this as
    /// the canonical display name; `None` when VNDB has no alt title.
    pub alttitle: Option<String>,
    /// Full cover image URL, if the VN has one. Fetched (through the Rust
    /// `fetch_cover` command, never rendered cross-origin) and stored on
    /// confirm.
    pub image_url: Option<String>,
    /// Smaller cover thumbnail URL for the candidate list. Same host as
    /// `image_url` (`t.vndb.org`); the UI fetches it via the backend and
    /// renders a local data URL so the webview CSP stays `self`-only.
    pub thumbnail: Option<String>,
    /// Release date string (`YYYY`, `YYYY-MM`, or `YYYY-MM-DD`); shown to help
    /// the user disambiguate between candidates.
    pub released: Option<String>,
    /// Developer names (producers flagged as developer). Prefers the
    /// original-script name (e.g. Japanese 「アトリエかぐや」) over the
    /// romanized one, mirroring how `alttitle` is preferred for the title.
    /// First is used as the author on confirm.
    pub developers: Vec<String>,
}

// ── Wire types (VNDB response shape) ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct VnSearchResponse {
    results: Vec<VnResult>,
}

#[derive(Debug, Deserialize)]
struct VnResult {
    id: String,
    title: Option<String>,
    alttitle: Option<String>,
    released: Option<String>,
    image: Option<VnImage>,
    #[serde(default)]
    developers: Vec<VnProducer>,
}

#[derive(Debug, Deserialize)]
struct VnImage {
    url: Option<String>,
    thumbnail: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VnProducer {
    name: Option<String>,
    /// Original-script name (Japanese, etc.). Null for producers VNDB only
    /// has a romanized name for.
    original: Option<String>,
}

impl VnProducer {
    /// Original-script name when present, else the romanized name. Empty/
    /// whitespace-only values fall through to the next preference.
    fn preferred_name(self) -> Option<String> {
        self.original
            .filter(|s| !s.trim().is_empty())
            .or_else(|| self.name.filter(|s| !s.trim().is_empty()))
    }
}

/// Search VNDB for visual novels matching `query`. Returns up to 10 candidates
/// best-first (the API's `searchrank` order). An empty query short-circuits to
/// an empty list so a blank filename doesn't fire a useless request.
pub async fn search(query: &str) -> Result<Vec<VndbCandidate>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let body = serde_json::json!({
        "filters": ["search", "=", trimmed],
        "fields": "title, alttitle, released, image.url, image.thumbnail, developers.name, developers.original",
        "sort": "searchrank",
        "results": 10,
    });

    let client = reqwest::Client::builder()
        .timeout(VNDB_REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("VNDB client build failed: {e}"))?;

    let resp = client
        .post(VNDB_VN_ENDPOINT)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("VNDB request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("VNDB returned HTTP {}", resp.status()));
    }

    let parsed: VnSearchResponse = resp
        .json()
        .await
        .map_err(|e| format!("VNDB response parse failed: {e}"))?;

    Ok(parsed
        .results
        .into_iter()
        .map(|r| {
            let (image_url, thumbnail) = match r.image {
                Some(i) => (
                    i.url.filter(|s| !s.trim().is_empty()),
                    i.thumbnail.filter(|s| !s.trim().is_empty()),
                ),
                None => (None, None),
            };
            VndbCandidate {
            id: r.id,
            title: r.title.unwrap_or_default(),
            alttitle: r.alttitle.filter(|s| !s.trim().is_empty()),
            image_url,
            thumbnail,
            released: r.released.filter(|s| !s.trim().is_empty()),
            developers: r
                .developers
                .into_iter()
                .filter_map(|d| d.preferred_name())
                .collect(),
            }
        })
        .collect())
}

/// Fetch the bytes of a VNDB cover image by URL. Returns `(bytes, mime_type)`.
/// The URL must be one returned by `search` (a `t.vndb.org` image URL); we
/// don't validate the host here since the caller only ever passes our own
/// candidate URLs back.
pub async fn fetch_cover(url: &str) -> Result<(Vec<u8>, String), String> {
    let client = reqwest::Client::builder()
        .timeout(VNDB_REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("VNDB client build failed: {e}"))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("VNDB cover request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("VNDB cover returned HTTP {}", resp.status()));
    }

    let mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "image/jpeg".to_string());

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("VNDB cover read failed: {e}"))?
        .to_vec();

    Ok((bytes, mime))
}
