//! Remote storage provider implementations.
//!
//! Biblio supports uploading comic/novel archives to remote storage for
//! off-loaded metadata-only rows. Today there's one provider
//! (`baidu_netdisk`); the module is organized so additional providers can
//! be added without reshaping the import flow.

pub mod baidu_netdisk;
