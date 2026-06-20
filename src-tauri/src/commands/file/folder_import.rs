use super::*;
/// True iff every non-hidden direct child of `dir` is an image file
/// AND `dir` contains no subdirectories. Hidden entries are ignored.
fn folder_is_image_leaf(dir: &std::path::Path) -> std::io::Result<bool> {
    use crate::pipeline::archive::{is_ignorable_metadata, is_image_filename};
    let mut saw_image = false;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // OS-metadata junk (dotfiles, Thumbs.db, desktop.ini) is transparent:
        // a folder of pages plus a stray Thumbs.db is still a comic leaf.
        if is_ignorable_metadata(&name_str) {
            continue;
        }
        let p = entry.path();
        if p.is_dir() {
            return Ok(false);
        }
        if p.is_file() {
            if !is_image_filename(&name_str) {
                return Ok(false);
            }
            saw_image = true;
        }
    }
    Ok(saw_image)
}

/// Recursively enumerate non-hidden files under `dir`. Image-leaf
/// subdirectories collapse to a single directory entry (the comic
/// pipeline zips them on commit). See `list_files_in_folder` for the
/// rationale behind the leaf-only collapse rule.
fn folder_walk(dir: &std::path::Path, out: &mut Vec<String>) -> std::io::Result<()> {
    use crate::pipeline::archive::is_ignorable_metadata;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Skip OS-metadata junk so it isn't emitted as a bogus standalone
        // import path (matches the leaf detector's transparency rule).
        if is_ignorable_metadata(&name_str) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            if folder_is_image_leaf(&path)? {
                if let Some(s) = path.to_str() {
                    out.push(s.to_string());
                }
            } else {
                folder_walk(&path, out)?;
            }
        } else if path.is_file() {
            if let Some(s) = path.to_str() {
                out.push(s.to_string());
            }
        }
    }
    Ok(())
}

/// Scan a single folder root using the import-aware walker. Result is
/// sorted so repeated scans produce stable ordering. If the root itself
/// is an image leaf, the root path is the only entry returned.
///
/// `schema` decides folder semantics. Galgame treats the picked folder as
/// ONE unit (a game is an opaque tree of executables/scripts/assets, not a
/// collapsible set of leaves) — it returns `[root]` and never walks or
/// image-leaf-tests, so `file_create` zips the whole directory on commit.
/// Comic/novel keep the image-leaf collapse + recursive walk.
fn scan_folder_root(
    root: &std::path::Path,
    schema: crate::schema::SchemaSlug,
) -> std::io::Result<Vec<String>> {
    if schema == crate::schema::SchemaSlug::Galgame {
        // The picked folder is the game. Don't descend — there's no
        // filesystem signal that would make walking it correct, and the
        // commit step archives the whole tree.
        return Ok(root
            .to_str()
            .map(|s| vec![s.to_string()])
            .unwrap_or_default());
    }
    let mut files = Vec::new();
    if folder_is_image_leaf(root)? {
        if let Some(s) = root.to_str() {
            files.push(s.to_string());
        }
    } else {
        folder_walk(root, &mut files)?;
    }
    files.sort();
    Ok(files)
}

/// Image-folder leaf collapse: a directory whose non-hidden direct
/// children are all image files (and which has no subdirectories) is
/// emitted as a single path. `file_prepare_import` routes such dir
/// paths through the comic pipeline; `file_create` zips them on commit.
/// The walker descends through every other directory, so a
/// `library/[author]/[work]/*.jpg` tree resolves to one comic per
/// `[work]` folder. Multi-level structures like
/// `vol/chapter-1/*.jpg, vol/chapter-2/*.jpg` are split into per-chapter
/// comics — there is no filesystem-only signal that distinguishes
/// sibling chapters of one comic from sibling comics of one author, so
/// this leaf-only rule errs on the side of finer-grained imports.
/// Result is sorted so repeated folder picks produce stable ordering.
#[tauri::command]
pub async fn list_files_in_folder(
    path: String,
    schema_slug: Option<String>,
) -> Result<Vec<String>, String> {
    let root = std::path::Path::new(&path);
    if !root.exists() {
        return Err("PATH_NOT_FOUND".to_string());
    }
    if !root.is_dir() {
        return Err("NOT_A_DIRECTORY".to_string());
    }
    let schema = crate::schema::SchemaSlug::from_str(schema_slug.as_deref().unwrap_or("novel"));
    scan_folder_root(root, schema).map_err(|e| format!("Failed to scan folder: {e}"))
}

#[derive(Serialize)]
pub struct DropExpansion {
    /// Resolved file paths: standalone files passed through, plus the
    /// recursive contents of every dropped folder (with image-leaf
    /// collapse, matching `list_files_in_folder`).
    pub files: Vec<String>,
    /// Maps each enumerated path to the folder root the user dropped.
    /// Standalone-file drops are absent from this map — they take the
    /// same code path as `FilePicker.handlePickFiles`.
    pub path_folder_roots: std::collections::HashMap<String, String>,
    /// Folder roots that contained no importable entries. Reported so
    /// the UI can surface them, mirroring `FilePicker.handlePickFolder`.
    pub empty_folders: Vec<String>,
}

/// Resolve OS-level drop paths into the same shape `FilePicker` produces.
/// Handles a mixed batch where some paths are files and others are
/// folders — files pass through untouched; folders are walked with the
/// same image-leaf rules as the explicit folder picker. Missing paths
/// are skipped silently (a stale Finder drag can race a filesystem move
/// and a hard error would block the rest of the batch).
#[tauri::command]
pub async fn expand_drop_paths(
    paths: Vec<String>,
    schema_slug: Option<String>,
) -> Result<DropExpansion, String> {
    let mut files = Vec::new();
    let mut path_folder_roots = std::collections::HashMap::new();
    let mut empty_folders = Vec::new();
    let schema = crate::schema::SchemaSlug::from_str(schema_slug.as_deref().unwrap_or("novel"));

    for raw in paths {
        let p = std::path::Path::new(&raw);
        if !p.exists() {
            continue;
        }
        if p.is_file() {
            files.push(raw);
        } else if p.is_dir() {
            let scanned = scan_folder_root(p, schema)
                .map_err(|e| format!("Failed to scan folder {raw}: {e}"))?;
            if scanned.is_empty() {
                empty_folders.push(raw);
                continue;
            }
            for f in scanned {
                path_folder_roots.insert(f.clone(), raw.clone());
                files.push(f);
            }
        }
    }

    Ok(DropExpansion {
        files,
        path_folder_roots,
        empty_folders,
    })
}

/// Post-commit cleanup for folder imports. Called by the frontend after
/// every per-file `file_create` in the batch succeeds.
///
/// Behavior:
/// - No-op when `had_folder_imports` is false (pure-archive folder picks
///   keep their picked root untouched, matching pre-feature behavior).
/// - No-op when `import_mode` is `'copy'` (copy semantics keep originals).
/// - Refuses to touch anything inside `storage_path` (defense in depth).
/// - Walks `folder_root` bottom-up and removes empty subdirectories.
///   If after the walk the root itself is empty, removes it. If
///   non-empty (the user had stray non-image files), leaves it alone
///   and logs to stderr — the import already succeeded; cleanup is
///   best-effort and never fails the call.
#[tauri::command]
pub async fn import_finalize(
    app: AppHandle,
    folder_root: String,
    had_folder_imports: bool,
) -> Result<(), String> {
    if !had_folder_imports {
        return Ok(());
    }

    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let import_mode: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_settings WHERE key = 'import_mode'")
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;
    if import_mode.map(|(v,)| v == "copy").unwrap_or(false) {
        return Ok(());
    }

    let root = PathBuf::from(&folder_root);
    if !root.exists() {
        // file_create already removed every leaf; nothing to do.
        return Ok(());
    }
    if !root.is_dir() {
        return Err("FOLDER_ROOT_NOT_A_DIRECTORY".to_string());
    }

    // Defense in depth: never recurse into anything under storage_path.
    let storage_path: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_settings WHERE key = 'storage_path'")
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;
    if let Some((sp,)) = storage_path {
        if !sp.is_empty() {
            let storage_canonical = std::path::Path::new(&sp)
                .canonicalize()
                .map_err(|e| format!("Failed to resolve storage path: {e}"))?;
            let root_canonical = root
                .canonicalize()
                .map_err(|e| format!("Failed to resolve folder root: {e}"))?;
            if root_canonical.starts_with(&storage_canonical) {
                return Err("FOLDER_ROOT_INSIDE_STORAGE".to_string());
            }
        }
    }

    /// True iff `dir` recursively contains no real content — only
    /// OS-metadata junk. Dotfiles (`.DS_Store`, `.localized`, …) plus
    /// Windows Explorer junk (`Thumbs.db`, `desktop.ini`) are transparent:
    /// the OS seeds them everywhere it's been opened, and they would
    /// otherwise block cleanup of folders that are otherwise empty after
    /// `file_create` removed the leaf source dirs. Mirrors the
    /// `is_ignorable_metadata` convention used by `folder_is_image_leaf`
    /// and `folder_walk`.
    fn has_only_ignorable_content(dir: &std::path::Path) -> std::io::Result<bool> {
        use crate::pipeline::archive::is_ignorable_metadata;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if is_ignorable_metadata(&name_str) {
                continue;
            }
            let p = entry.path();
            if p.is_dir() {
                if !has_only_ignorable_content(&p)? {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }
        Ok(true)
    }

    match has_only_ignorable_content(&root) {
        Ok(true) => {
            // `remove_dir_all` nukes the dir tree including the hidden
            // metadata we treated as transparent above.
            if let Err(e) = std::fs::remove_dir_all(&root) {
                eprintln!(
                    "import_finalize: remove_dir_all failed for {}: {e}",
                    root.display()
                );
            }
            Ok(())
        }
        Ok(false) => {
            eprintln!(
                "import_finalize: {} not removed (real files remain after leaf cleanup)",
                root.display()
            );
            Ok(())
        }
        Err(e) => {
            eprintln!(
                "import_finalize: cleanup failed for {}: {e}",
                root.display()
            );
            Ok(())
        }
    }
}
