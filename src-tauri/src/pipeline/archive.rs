//! Format-agnostic archive helpers shared by the comic-pipeline nodes.
//!
//! Comics arrive as ZIP/CBZ, RAR/CBR, or as a directory of loose images
//! that the import flow auto-zips on commit. The three backends expose
//! very different APIs (random-access central directory vs sequential
//! header walk vs filesystem walk), so callers go through this module
//! instead of branching at every call site. Helpers return:
//!
//! - `list_image_entries` — enumerate image entries paired with an
//!   archive-internal index.
//! - `read_entry_bytes` — read the bytes of one entry, given its index.
//! - `pick_first_cover` — Phase-1 baseline cover pick (first/alphabetical
//!   image with cover-name preference).
//!
//! Index semantics: the index is the entry's position in a fresh
//! sequential walk of the archive (0-based, counting *all* entries
//! including non-images and directories). For ZIP that maps to the
//! central-directory index, so reads are O(1). For RAR it's the position
//! in `open_for_listing` order, so reads walk forward from the start —
//! still cheap because the comic pipeline only reads ≤5 candidates. For
//! `ImageDir` it's the position in the sorted recursive walk of the
//! directory (image files only).

use std::io::Read;
use std::path::Path;

use crate::pipeline::Cover;

/// One image entry inside an archive, format-agnostic.
#[derive(Debug, Clone)]
pub struct ArchiveImageEntry {
    pub basename: String,
    pub archive_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Zip,
    Rar,
    /// A directory of loose images on disk. Used by the folder-import
    /// flow when the user picks a folder whose recursive contents are
    /// all images — the pipeline runs against the directory directly,
    /// and `file_create` zips it on commit.
    ImageDir,
}

fn format_for_path(path: &Path) -> Result<Format, String> {
    if path.is_dir() {
        return Ok(Format::ImageDir);
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "zip" | "cbz" => Ok(Format::Zip),
        "rar" | "cbr" => Ok(Format::Rar),
        other => Err(format!("Unsupported archive extension: {other}")),
    }
}

/// Public predicate so callers outside this module (e.g. the folder-import
/// scanner) share the exact same image-extension definition.
pub fn is_image_filename(name: &str) -> bool {
    is_image_basename(name)
}

fn is_image_basename(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".webp")
        || lower.ends_with(".gif")
}

fn basename_of(raw_name: &str) -> String {
    Path::new(raw_name)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| raw_name.to_string())
}

/// Reject macOS AppleDouble cruft that lives inside ZIP/RAR archives
/// produced on macOS. `__MACOSX/._cover.jpg` and stray `._foo.jpg`
/// sidecars carry an image extension but are resource-fork metadata,
/// not real pages — treating them as the cover yields a corrupt
/// thumbnail. Mirrors the dotfile skip the directory backend already
/// does in `collect_dir_image_paths`. `raw_name` is the full
/// archive-internal path; `basename` its last component.
fn is_apple_double(raw_name: &str, basename: &str) -> bool {
    raw_name.split('/').any(|seg| seg == "__MACOSX")
        || basename.starts_with("._")
        || basename.starts_with('.')
}

/// MIME type to attach when uploading one of the listed image entries to
/// a vision LLM. Defaults to JPEG when the extension is unknown because
/// most comic dumps use JPEG.
pub fn guess_image_mime(basename: &str) -> String {
    let lower = basename.to_lowercase();
    if lower.ends_with(".png") {
        "image/png".into()
    } else if lower.ends_with(".webp") {
        "image/webp".into()
    } else if lower.ends_with(".gif") {
        "image/gif".into()
    } else {
        "image/jpeg".into()
    }
}

pub fn list_image_entries(path: &Path) -> Result<Vec<ArchiveImageEntry>, String> {
    match format_for_path(path)? {
        Format::Zip => list_image_entries_zip(path),
        Format::Rar => list_image_entries_rar(path),
        Format::ImageDir => list_image_entries_dir(path),
    }
}

pub fn read_entry_bytes(path: &Path, archive_index: usize) -> Result<Vec<u8>, String> {
    match format_for_path(path)? {
        Format::Zip => read_entry_bytes_zip(path, archive_index),
        Format::Rar => read_entry_bytes_rar(path, archive_index),
        Format::ImageDir => read_entry_bytes_dir(path, archive_index),
    }
}

/// Walk a directory recursively and collect every image file's path
/// relative to `root`, sorted lexicographically. Used by the ImageDir
/// backend for both listing and indexed reads — keep both in sync.
fn collect_dir_image_paths(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    fn walk(
        dir: &Path,
        out: &mut Vec<std::path::PathBuf>,
    ) -> Result<(), String> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("Failed to read directory {}: {e}", dir.display()))?
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
                walk(&p, out)?;
            } else if p.is_file() && is_image_basename(&name_str) {
                out.push(p);
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, &mut out)?;
    Ok(out)
}

/// Phase-1 baseline cover pick. Returns the first image (cover/000/001
/// preferred) along with the bytes and a guessed MIME type. `Err` when
/// the archive is unreadable or has no image entries.
pub fn pick_first_cover(path: &Path) -> Result<Cover, String> {
    let entries = list_image_entries(path)?;
    if entries.is_empty() {
        return Err("No images found in archive".to_string());
    }

    let mut sorted = entries.clone();
    sorted.sort_by(|a, b| a.basename.cmp(&b.basename));

    let pick = sorted
        .iter()
        .find(|e| {
            let lower = e.basename.to_lowercase();
            lower.contains("cover") || lower.starts_with("000") || lower.starts_with("001")
        })
        .cloned()
        .unwrap_or_else(|| sorted[0].clone());

    let data = read_entry_bytes(path, pick.archive_index)?;
    let mime_type = guess_image_mime(&pick.basename);
    Ok(Cover { data, mime_type })
}

// ── ZIP backend ──────────────────────────────────────────────────────────

fn list_image_entries_zip(path: &Path) -> Result<Vec<ArchiveImageEntry>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Failed to open archive: {e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| format!("Failed to read ZIP archive: {e}"))?;

    let mut entries: Vec<ArchiveImageEntry> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index_raw(i)
            .map_err(|e| format!("ZIP entry error: {e}"))?;
        let raw_name = entry.name().to_string();
        let basename = basename_of(&raw_name);
        if is_apple_double(&raw_name, &basename) || !is_image_basename(&basename) {
            continue;
        }
        entries.push(ArchiveImageEntry {
            basename,
            archive_index: i,
        });
    }
    Ok(entries)
}

fn read_entry_bytes_zip(path: &Path, archive_index: usize) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open archive: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("read archive: {e}"))?;
    let mut entry = archive
        .by_index(archive_index)
        .map_err(|e| format!("zip entry {archive_index}: {e}"))?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read entry: {e}"))?;
    Ok(bytes)
}

// ── RAR backend ──────────────────────────────────────────────────────────

fn list_image_entries_rar(path: &Path) -> Result<Vec<ArchiveImageEntry>, String> {
    let archive = unrar::Archive::new(path)
        .open_for_listing()
        .map_err(|e| format!("Failed to open RAR archive: {e}"))?;

    let mut entries: Vec<ArchiveImageEntry> = Vec::new();
    for (i, item) in archive.enumerate() {
        let entry = item.map_err(|e| format!("RAR entry error: {e}"))?;
        if entry.is_directory() {
            continue;
        }
        let raw_name = entry.filename.to_string_lossy().to_string();
        let basename = basename_of(&raw_name);
        if is_apple_double(&raw_name, &basename) || !is_image_basename(&basename) {
            continue;
        }
        entries.push(ArchiveImageEntry {
            basename,
            archive_index: i,
        });
    }
    Ok(entries)
}

fn read_entry_bytes_rar(path: &Path, archive_index: usize) -> Result<Vec<u8>, String> {
    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(|e| format!("open archive: {e}"))?;

    let mut current = 0usize;
    while let Some(header) = archive
        .read_header()
        .map_err(|e| format!("read header: {e}"))?
    {
        if current == archive_index {
            let (data, _rest) = header.read().map_err(|e| format!("read entry: {e}"))?;
            return Ok(data);
        }
        archive = header.skip().map_err(|e| format!("skip entry: {e}"))?;
        current += 1;
    }
    Err(format!("rar entry {archive_index} not found"))
}

// ── ImageDir backend ─────────────────────────────────────────────────────

fn list_image_entries_dir(path: &Path) -> Result<Vec<ArchiveImageEntry>, String> {
    let paths = collect_dir_image_paths(path)?;
    let entries = paths
        .into_iter()
        .enumerate()
        .map(|(i, p)| ArchiveImageEntry {
            basename: p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            archive_index: i,
        })
        .collect();
    Ok(entries)
}

fn read_entry_bytes_dir(path: &Path, archive_index: usize) -> Result<Vec<u8>, String> {
    let paths = collect_dir_image_paths(path)?;
    let p = paths
        .get(archive_index)
        .ok_or_else(|| format!("dir entry {archive_index} not found"))?;
    std::fs::read(p).map_err(|e| format!("read entry: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_image_basename_accepts_common_formats() {
        for ext in ["jpg", "JPG", "jpeg", "png", "PNG", "webp", "gif"] {
            assert!(is_image_basename(&format!("001.{ext}")));
        }
    }

    #[test]
    fn is_image_basename_rejects_non_images() {
        for name in ["README.txt", "meta.json", "001", "folder/", "001.bmp"] {
            assert!(!is_image_basename(name), "{name} should not be image");
        }
    }

    #[test]
    fn format_for_path_recognizes_zip_and_rar() {
        assert_eq!(
            format_for_path(Path::new("foo.zip")).unwrap(),
            Format::Zip
        );
        assert_eq!(
            format_for_path(Path::new("foo.cbz")).unwrap(),
            Format::Zip
        );
        assert_eq!(
            format_for_path(Path::new("foo.rar")).unwrap(),
            Format::Rar
        );
        assert_eq!(
            format_for_path(Path::new("foo.cbr")).unwrap(),
            Format::Rar
        );
        assert_eq!(
            format_for_path(Path::new("FOO.RAR")).unwrap(),
            Format::Rar
        );
        assert!(format_for_path(Path::new("foo.txt")).is_err());
    }

    #[test]
    fn format_for_path_recognizes_directory() {
        let tmp = std::env::temp_dir().join(format!(
            "biblio_archive_dirfmt_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        assert_eq!(format_for_path(&tmp).unwrap(), Format::ImageDir);
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn imagedir_backend_lists_and_reads_in_sorted_order() {
        let tmp = std::env::temp_dir().join(format!(
            "biblio_archive_dir_{}",
            std::process::id()
        ));
        let nested = tmp.join("chapter-2");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(tmp.join("002.jpg"), b"002bytes").unwrap();
        std::fs::write(tmp.join("001.png"), b"001bytes").unwrap();
        std::fs::write(tmp.join("readme.txt"), b"ignored").unwrap();
        std::fs::write(nested.join("003.webp"), b"003bytes").unwrap();

        let entries = list_image_entries(&tmp).unwrap();
        assert_eq!(
            entries.iter().map(|e| e.basename.as_str()).collect::<Vec<_>>(),
            vec!["001.png", "002.jpg", "003.webp"]
        );

        let bytes = read_entry_bytes(&tmp, 1).unwrap();
        assert_eq!(bytes, b"002bytes".to_vec());

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn guess_image_mime_uses_extension() {
        assert_eq!(guess_image_mime("foo.png"), "image/png");
        assert_eq!(guess_image_mime("FOO.WEBP"), "image/webp");
        assert_eq!(guess_image_mime("foo.gif"), "image/gif");
        assert_eq!(guess_image_mime("foo.jpg"), "image/jpeg");
        assert_eq!(guess_image_mime("foo"), "image/jpeg");
    }
}
