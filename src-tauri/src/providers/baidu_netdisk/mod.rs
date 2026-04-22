//! Baidu Netdisk (xpan Open Platform) client used by biblio's remote
//! upload path. Auth flow follows OpenList: a user-supplied refresh token
//! drives either OpenList's proxy refresh endpoint (default, zero-setup)
//! or the user's own registered Baidu app (advanced).
//!
//! Operations exposed:
//! - `refresh_access_token` — renew the short-lived access token
//! - `upload_file`          — precreate → slice upload → create
//! - `delete_file`          — filemanager:delete by path
//!
//! Not implemented: listing, download, move/rename, rapid-upload by MD5
//! match. Biblio doesn't read files back from Baidu (cover + metadata
//! live locally), so download isn't on the roadmap.

mod auth;
mod delete;
mod types;
mod upload;

pub use auth::{AuthMode, BaiduCredentials, refresh_access_token};
pub use delete::delete_file;
pub use types::{BaiduError, UploadResult};
pub use upload::upload_file;
