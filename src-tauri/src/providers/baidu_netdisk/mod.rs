//! Baidu Netdisk (xpan Open Platform) client used by biblio's remote
//! upload path. Auth uses implicit grant OAuth: user visits authorize URL
//! and extracts access_token from the URL fragment after login.
//!
//! Operations exposed:
//! - `build_authorize_url` — construct OAuth authorize URL with implicit grant
//! - `upload_file`         — precreate → slice upload → create
//! - `delete_file`         — filemanager:delete by path
//!
//! Not implemented: listing, download, move/rename, rapid-upload by MD5
//! match. Biblio doesn't read files back from Baidu (cover + metadata
//! live locally), so download isn't on the roadmap.

mod auth;
mod delete;
mod types;
mod upload;

pub use auth::build_authorize_url;
pub use delete::delete_file;
pub use types::{BaiduError, UploadResult};
pub use upload::upload_file;
