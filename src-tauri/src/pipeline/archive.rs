//! Format-agnostic archive helpers shared by the comic-pipeline nodes.
//!
//! Comics arrive as ZIP/CBZ or RAR/CBR. The two backends expose very
//! different APIs (random-access central directory vs sequential header
//! walk), so callers go through this module instead of branching at every
//! call site. Helpers return:
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
//! still cheap because the comic pipeline only reads ≤5 candidates.

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
}

fn format_for_path(path: &Path) -> Result<Format, String> {
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
    }
}

pub fn read_entry_bytes(path: &Path, archive_index: usize) -> Result<Vec<u8>, String> {
    match format_for_path(path)? {
        Format::Zip => read_entry_bytes_zip(path, archive_index),
        Format::Rar => read_entry_bytes_rar(path, archive_index),
    }
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
        let basename = basename_of(entry.name());
        if !is_image_basename(&basename) {
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
        if !is_image_basename(&basename) {
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
    fn guess_image_mime_uses_extension() {
        assert_eq!(guess_image_mime("foo.png"), "image/png");
        assert_eq!(guess_image_mime("FOO.WEBP"), "image/webp");
        assert_eq!(guess_image_mime("foo.gif"), "image/gif");
        assert_eq!(guess_image_mime("foo.jpg"), "image/jpeg");
        assert_eq!(guess_image_mime("foo"), "image/jpeg");
    }
}
