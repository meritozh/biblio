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
