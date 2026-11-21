use super::*;
pub(super) fn get_unique_destination(dest: &std::path::Path) -> PathBuf {
    if !dest.exists() {
        return dest.to_path_buf();
    }

    let parent = dest.parent().unwrap_or(std::path::Path::new("."));
    let stem = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = dest
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    let mut counter = 1;
    loop {
        let new_name = if ext.is_empty() {
            format!("{} ({})", stem, counter)
        } else {
            format!("{} ({}){}", stem, counter, ext)
        };
        let new_path = parent.join(&new_name);
        if !new_path.exists() {
            return new_path;
        }
        counter += 1;
    }
}

/// Build a clean filename from metadata: "<display_name> <progress> <authors>.ext"
pub fn build_novel_filename(
    display_name: &str,
    progress: Option<&str>,
    author_names: &[String],
    ext_with_dot: &str,
) -> String {
    let mut parts: Vec<String> = vec![display_name.to_string()];
    if let Some(p) = progress {
        if !p.is_empty() {
            parts.push(p.to_string());
        }
    }
    if !author_names.is_empty() {
        parts.push(author_names.join(", "));
    }
    format!("{}{}", parts.join(" "), ext_with_dot)
}

/// Remove filesystem-invalid characters from a filename
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect()
}

/// Copy a file to destination
/// Returns the final destination path
pub(super) fn copy_file(
    source: &std::path::Path,
    dest: &std::path::Path,
) -> Result<PathBuf, String> {
    let final_dest = get_unique_destination(dest);

    fs::copy(source, &final_dest).map_err(|e| {
        let err_str = e.to_string().to_lowercase();
        if err_str.contains("permission denied") {
            "PERMISSION_DENIED".to_string()
        } else if err_str.contains("disk full") || err_str.contains("no space") {
            "DISK_FULL".to_string()
        } else if err_str.contains("being used") || err_str.contains("locked") {
            "FILE_LOCKED".to_string()
        } else {
            format!("Failed to copy file: {}", e)
        }
    })?;

    Ok(final_dest)
}

/// Write every image file under `source_dir` (recursively, sorted) into
/// a `.zip` at `dest`. Stored (no compression) — comic images are already
/// JPEG/PNG/WebP, so deflate burns CPU for ~0% gain. Returns the final
/// destination path (after any unique-name disambiguation). Hidden files
/// and non-image files are skipped, matching the importer's collapse
/// rule.
pub(super) fn zip_image_dir(
    source_dir: &std::path::Path,
    dest: &std::path::Path,
) -> Result<PathBuf, String> {
    use crate::pipeline::archive::is_image_filename;
    use std::io::Write;

    let final_dest = get_unique_destination(dest);
    let f = std::fs::File::create(&final_dest).map_err(|e| format!("Failed to create zip: {e}"))?;
    let mut zw = zip::ZipWriter::new(f);
    let opts: zip::write::SimpleFileOptions =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    fn walk(
        root: &std::path::Path,
        dir: &std::path::Path,
        zw: &mut zip::ZipWriter<std::fs::File>,
        opts: &zip::write::SimpleFileOptions,
    ) -> Result<(), String> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("Failed to read {}: {e}", dir.display()))?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') {
                continue;
            }
            let p = entry.path();
            if p.is_dir() {
                walk(root, &p, zw, opts)?;
            } else if p.is_file() && is_image_filename(&name_str) {
                let rel = p
                    .strip_prefix(root)
                    .map_err(|e| format!("strip_prefix: {e}"))?;
                // ZIP paths use forward slashes by spec.
                let zip_name = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join("/");
                zw.start_file(zip_name, *opts)
                    .map_err(|e| format!("zip start_file: {e}"))?;
                let bytes = std::fs::read(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
                zw.write_all(&bytes)
                    .map_err(|e| format!("zip write: {e}"))?;
            }
        }
        Ok(())
    }

    walk(source_dir, source_dir, &mut zw, &opts)?;
    zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    Ok(final_dest)
}

/// Archive the WHOLE directory tree under `source_dir` into a `.zip` at
/// `dest`, verbatim — used for galgame folder imports, where a game is more
/// than its images (executables, scripts, audio, and the on-disk directory
/// layout all matter). The ZIP format is a flat list of entries, so the tree
/// is enumerated; every file AND every directory (including empty ones) is
/// recorded so the extracted game is byte-for-byte the same shape.
///
/// File bytes are streamed with `io::copy` rather than read whole into RAM —
/// a multi-GB game file would otherwise spike memory (see the `MimeDetectNode`
/// note about per-import RAM growth). Stored (no compression): game assets are
/// already compressed, so deflate burns CPU for ~0% gain. OS-metadata junk
/// (`.DS_Store`, `Thumbs.db`, `desktop.ini`) is skipped via the shared
/// `is_ignorable_metadata` predicate. Returns the final destination path after
/// any unique-name disambiguation.
pub(super) fn zip_dir(
    source_dir: &std::path::Path,
    dest: &std::path::Path,
) -> Result<PathBuf, String> {
    use crate::pipeline::archive::is_ignorable_metadata;

    let final_dest = get_unique_destination(dest);
    let f = std::fs::File::create(&final_dest).map_err(|e| format!("Failed to create zip: {e}"))?;
    let mut zw = zip::ZipWriter::new(f);
    let opts: zip::write::SimpleFileOptions =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    /// Build the forward-slash ZIP entry name for `path` relative to `root`.
    fn zip_name(root: &std::path::Path, path: &std::path::Path) -> Result<String, String> {
        let rel = path
            .strip_prefix(root)
            .map_err(|e| format!("strip_prefix: {e}"))?;
        Ok(rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("/"))
    }

    fn walk(
        root: &std::path::Path,
        dir: &std::path::Path,
        zw: &mut zip::ZipWriter<std::fs::File>,
        opts: &zip::write::SimpleFileOptions,
    ) -> Result<(), String> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("Failed to read {}: {e}", dir.display()))?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if is_ignorable_metadata(&name_str) {
                continue;
            }
            let p = entry.path();
            if p.is_dir() {
                // Record the directory entry so empty dirs the game expects at
                // runtime (e.g. `save/`) survive the round-trip, then recurse.
                zw.add_directory(zip_name(root, &p)?, *opts)
                    .map_err(|e| format!("zip add_directory: {e}"))?;
                walk(root, &p, zw, opts)?;
            } else if p.is_file() {
                zw.start_file(zip_name(root, &p)?, *opts)
                    .map_err(|e| format!("zip start_file: {e}"))?;
                // Stream the file rather than slurping it — keeps RAM flat for
                // multi-GB game assets.
                let mut src =
                    std::fs::File::open(&p).map_err(|e| format!("open {}: {e}", p.display()))?;
                std::io::copy(&mut src, zw)
                    .map_err(|e| format!("zip write {}: {e}", p.display()))?;
            }
        }
        Ok(())
    }

    walk(source_dir, source_dir, &mut zw, &opts)?;
    zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    Ok(final_dest)
}

/// Move a file, handling cross-drive moves
/// Returns the final destination path
pub(super) fn move_file(
    source: &std::path::Path,
    dest: &std::path::Path,
) -> Result<PathBuf, String> {
    let final_dest = get_unique_destination(dest);

    // Try rename first (fast, same filesystem)
    if fs::rename(source, &final_dest).is_ok() {
        return Ok(final_dest);
    }

    // Fall back to copy + delete (cross-drive)
    fs::copy(source, &final_dest).map_err(|e| {
        // Map common filesystem errors to user-friendly codes
        let err_str = e.to_string().to_lowercase();
        if err_str.contains("permission denied") {
            "PERMISSION_DENIED".to_string()
        } else if err_str.contains("disk full") || err_str.contains("no space") {
            "DISK_FULL".to_string()
        } else if err_str.contains("being used") || err_str.contains("locked") {
            "FILE_LOCKED".to_string()
        } else {
            format!("Failed to copy file: {}", e)
        }
    })?;

    fs::remove_file(source).map_err(|e| format!("Failed to remove original: {}", e))?;

    Ok(final_dest)
}
