//! Baidu Netdisk (xpan Open Platform) client used by biblio's remote
//! upload path. Auth uses implicit grant OAuth: user visits authorize URL
//! and extracts access_token from the URL fragment after login.
//!
//! Operations exposed:
//! - `build_authorize_url` — construct OAuth authorize URL with implicit grant
//! - `upload_file`         — precreate → slice upload → create
//! - `delete_file`         — filemanager:delete by path
//! - `get_download_dlink`  — filemetas to fetch a short-lived download URL
//! - `download_to`         — stream a dlink to disk with atomic rename
//!
//! Not implemented: listing, move/rename, rapid-upload by MD5 match.

mod auth;
mod delete;
mod download;
mod types;
mod upload;

pub use auth::build_authorize_url;
pub use delete::delete_file;
pub use download::{download_to, get_download_dlink};
pub use types::{BaiduError, UploadResult};
pub use upload::upload_file;

use std::time::Duration;

/// Connect-phase timeout shared by every Baidu request. Deliberately scoped to
/// the connection handshake, NOT total request time: a large file download
/// buffers the whole body and can legitimately run for minutes, so a total
/// timeout here would abort big transfers. Per-request total timeouts are
/// applied locally where appropriate (e.g. the upload slice POSTs).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Build a reqwest client preconfigured for Baidu's PCS / xpan endpoints.
///
/// Baidu's `*.pcs.baidu.com` and xpan hosts require the literal
/// `User-Agent: pan.baidu.com`; other UAs get intermittent 403/500 responses
/// even with a valid access_token. The download dlink path already learned
/// this the hard way; routing every call through this helper makes the header
/// impossible to forget. Returns an error string on the (effectively
/// impossible) TLS-backend build failure so callers stay `Result`-based.
pub(super) fn http_client() -> Result<reqwest::Client, BaiduError> {
    reqwest::Client::builder()
        .user_agent("pan.baidu.com")
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .map_err(|e| BaiduError(format!("Failed to build HTTP client: {}", e.without_url())))
}
