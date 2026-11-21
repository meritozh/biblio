//! External provider implementations.
//!
//! `baidu_netdisk` is a remote storage backend for off-loaded metadata-only
//! rows. `vndb` is a read-only metadata source (the Visual Novel Database)
//! used to autofill galgame imports. The module is organized so additional
//! providers can be added without reshaping the import flow.

pub mod baidu_netdisk;
pub mod vndb;
