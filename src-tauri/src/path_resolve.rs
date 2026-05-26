//! Path resolution layer.
//!
//! Files are stored in the DB with paths RELATIVE to their respective
//! roots: local files relative to `storage_path`, remote files relative
//! to `app_root`, and local cache copies relative to `storage_path`.
//! This lets the user rename or move either root without touching the DB.
//!
//! Every site that needs an absolute filesystem path (open, stat, read,
//! delete, archive operations) goes through these helpers. Every site
//! that writes a path to the DB calls the inverse (`to_relative`) to
//! strip the root prefix first.
//!
//! Resolution is defensive: if a stored value is already absolute
//! (starts with `/`), it passes through unchanged. This survives:
//!   1. Rows the path-relativization migration couldn't relativize (they
//!      didn't start with the configured root prefix).
//!   2. Manual DB edits.
//!   3. Future schema evolution where some columns might remain absolute.

use std::path::{Path, PathBuf};

/// Resolve a stored path to its current absolute form. `storage_kind`
/// picks which root to join against — `'local'` (or any non-`'remote'`
/// value) → `storage_path`; `'remote'` → `app_root`.
///
/// Absolute paths pass through unchanged (see module docs).
///
/// Empty stored paths return an empty `PathBuf` — callers shouldn't
/// reach this path with empty data, but failing soft beats panicking.
pub fn to_absolute(
    storage_kind: &str,
    stored: &str,
    storage_path: &str,
    app_root: &str,
) -> PathBuf {
    if stored.is_empty() {
        return PathBuf::new();
    }
    // Pass through already-absolute paths. On Unix, leading `/` is the
    // canonical marker; Windows uses `Path::is_absolute` which also
    // recognises drive letters and UNC paths.
    if Path::new(stored).is_absolute() {
        return PathBuf::from(stored);
    }
    let root = match storage_kind {
        "remote" => app_root,
        _ => storage_path,
    };
    join_root(root, stored)
}

/// Resolve a cache path (`files.local_cache_path`) to absolute form.
/// Cache lives under `storage_path` regardless of the file's
/// `storage_kind` (cache is the LOCAL copy of a remote file). Same
/// pass-through-on-absolute semantics as `to_absolute`.
pub fn cache_to_absolute(stored: &str, storage_path: &str) -> PathBuf {
    if stored.is_empty() {
        return PathBuf::new();
    }
    if Path::new(stored).is_absolute() {
        return PathBuf::from(stored);
    }
    join_root(storage_path, stored)
}

/// Strip the appropriate root prefix from an absolute path. Returns the
/// relative form on success; on failure (path doesn't live under the
/// root) returns the absolute path unchanged so the row is still
/// retrievable via the resolver's absolute-passthrough branch.
///
/// `path` must be absolute and canonicalized by the caller — this is a
/// pure string operation, not a filesystem walk.
pub fn to_relative_local(absolute: &str, storage_path: &str) -> String {
    strip_root(absolute, storage_path)
}

/// Inverse for cache writes. Cache always lives under `storage_path`.
pub fn to_relative_cache(absolute: &str, storage_path: &str) -> String {
    strip_root(absolute, storage_path)
}

fn join_root(root: &str, rel: &str) -> PathBuf {
    // Trim a trailing slash on `root` and a leading slash on `rel` so
    // the join produces exactly one separator between them. Avoids
    // PathBuf::join's quirk where an absolute-style `rel` would discard
    // `root` entirely — which can't happen here because we already
    // pass-through truly absolute paths, but defensive is cheap.
    let root_trimmed = root.trim_end_matches('/');
    let rel_trimmed = rel.trim_start_matches('/');
    let mut buf = PathBuf::from(root_trimmed);
    buf.push(rel_trimmed);
    buf
}

fn strip_root(absolute: &str, root: &str) -> String {
    if root.is_empty() {
        return absolute.to_string();
    }
    let root_trimmed = root.trim_end_matches('/');
    // Match `root/...` exactly; require the separator so `/a/biblio` is
    // not treated as starting with `/a/bib`. Equality-with-root case
    // (absolute == root) returns "" which is treated as empty stored
    // path by the resolver — semantically "the root itself", which a
    // file row should never be, but harmless.
    if absolute == root_trimmed {
        return String::new();
    }
    let with_sep = format!("{}/", root_trimmed);
    if let Some(rest) = absolute.strip_prefix(&with_sep) {
        return rest.to_string();
    }
    // Path doesn't live under the root. Caller can choose to log; the
    // pass-through behavior of `to_absolute` keeps the row functional.
    absolute.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_passes_through() {
        let p = to_absolute("local", "/abs/file.txt", "/storage", "/apps/biblio");
        assert_eq!(p, PathBuf::from("/abs/file.txt"));
    }

    #[test]
    fn local_joins_storage_path() {
        let p = to_absolute("local", "novel/三体.txt", "/Users/x/Books", "/apps/biblio");
        assert_eq!(p, PathBuf::from("/Users/x/Books/novel/三体.txt"));
    }

    #[test]
    fn remote_joins_app_root() {
        let p = to_absolute("remote", "abc.cbz", "/Users/x/Books", "/apps/biblio");
        assert_eq!(p, PathBuf::from("/apps/biblio/abc.cbz"));
    }

    #[test]
    fn unknown_kind_falls_back_to_local() {
        // Unknown storage_kind values resolve as local. Treats a
        // newly-introduced kind that hasn't been wired here as local
        // rather than nothing.
        let p = to_absolute("legacy", "x.txt", "/Users/x/Books", "/apps/biblio");
        assert_eq!(p, PathBuf::from("/Users/x/Books/x.txt"));
    }

    #[test]
    fn trailing_slash_on_root_is_idempotent() {
        let p = to_absolute("local", "x.txt", "/Users/x/Books/", "/apps/biblio");
        assert_eq!(p, PathBuf::from("/Users/x/Books/x.txt"));
    }

    #[test]
    fn empty_stored_path_returns_empty() {
        let p = to_absolute("local", "", "/Users/x/Books", "/apps/biblio");
        assert_eq!(p, PathBuf::new());
    }

    #[test]
    fn cache_resolves_under_storage_regardless_of_kind() {
        let p = cache_to_absolute(".cache/abc.cbz", "/Users/x/Books");
        assert_eq!(p, PathBuf::from("/Users/x/Books/.cache/abc.cbz"));
    }

    #[test]
    fn cache_absolute_passes_through() {
        let p = cache_to_absolute("/old/cache/file", "/Users/x/Books");
        assert_eq!(p, PathBuf::from("/old/cache/file"));
    }

    #[test]
    fn relativize_local_strips_storage_prefix() {
        let r = to_relative_local("/Users/x/Books/novel/三体.txt", "/Users/x/Books");
        assert_eq!(r, "novel/三体.txt");
    }

    #[test]
    fn relativize_local_handles_trailing_slash() {
        let r = to_relative_local("/Users/x/Books/novel/三体.txt", "/Users/x/Books/");
        assert_eq!(r, "novel/三体.txt");
    }

    #[test]
    fn relativize_returns_input_when_outside_root() {
        // Row that doesn't live under storage — the relativizer leaves
        // it alone so the resolver's absolute-passthrough kicks in.
        let r = to_relative_local("/Volumes/External/x.txt", "/Users/x/Books");
        assert_eq!(r, "/Volumes/External/x.txt");
    }

    #[test]
    fn relativize_does_not_match_sibling_prefix() {
        // `/Users/x/Books2` is NOT under `/Users/x/Books`. Require the
        // separator so prefix-match doesn't false-positive.
        let r = to_relative_local("/Users/x/Books2/x.txt", "/Users/x/Books");
        assert_eq!(r, "/Users/x/Books2/x.txt");
    }

}
