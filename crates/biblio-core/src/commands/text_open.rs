//! Resolve a catalog action before starting a download, entering the reader, or launching an app.
use crate::{
    Library,
    text_reader::store::{ReaderStore, TxtOpener},
};
use std::path::Path;

pub use super::cache::{CopyState, ExternalOpener, SystemOpener, readable_copy};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Entry {
    Shelf,
    DetailOpen,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenTarget {
    Details(i64),
    CategoryFile(i64),
    Reader(i64),
    ExternalTxt { id: i64, program: String },
}

#[derive(sqlx::FromRow)]
struct CatalogFile {
    path: String,
    storage_kind: String,
    original_path: Option<String>,
    remote_container: Option<String>,
    local_cache_path: Option<String>,
}

impl CatalogFile {
    fn filename_source(&self) -> Option<&Path> {
        let path = Path::new(&self.path);
        if self.storage_kind != "remote" {
            return Some(path);
        }
        if let Some(original) = self
            .original_path
            .as_deref()
            .filter(|p| !p.trim().is_empty())
        {
            return Some(Path::new(original));
        }
        // Only legacy raw uploads can have a meaningful extension on the remote path.
        if self.remote_container.is_none() && path.extension().is_some_and(|ext| !ext.is_empty()) {
            return Some(path);
        }
        self.local_cache_path.as_deref().map(Path::new)
    }
}

/// Classify by the recorded filename even when the file is missing or uncached.
pub async fn target(db: &Library, id: i64, entry: Entry) -> Result<OpenTarget, String> {
    let file: CatalogFile = sqlx::query_as(
        "SELECT path, COALESCE(storage_kind, 'local') AS storage_kind, original_path,
                remote_container, local_cache_path FROM files WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&db.pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("FILE_NOT_FOUND")?;

    let extension = file.filename_source().and_then(Path::extension);
    if !extension.is_some_and(|ext| ext.eq_ignore_ascii_case("txt")) {
        return Ok(match entry {
            Entry::Shelf => OpenTarget::Details(id),
            Entry::DetailOpen => OpenTarget::CategoryFile(id),
        });
    }
    let opener = ReaderStore::new(db.clone())
        .opener()
        .await
        .map_err(|e| e.to_string())?;
    Ok(match opener {
        TxtOpener::BuiltIn => OpenTarget::Reader(id),
        TxtOpener::External(program) => OpenTarget::ExternalTxt { id, program },
    })
}

#[cfg(test)]
mod tests;
