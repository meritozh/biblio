# Local Media Storage System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform biblio into a local media management app that moves files into a configurable storage path organized by category.

**Architecture:** Add storage path setting, modify file operations to move files into category folders, and update category operations to manage folder structure. Files are moved on import and reorganized when category changes.

**Tech Stack:** Rust (Tauri backend with sqlx), React (frontend with shadcn/ui), SQLite

---

## File Structure

**New files:**
- `src-tauri/src/commands/settings.rs` - App settings CRUD
- `src/components/StoragePathSetting.tsx` - Storage path configuration UI
- `src/components/SettingsDialog.tsx` - Settings dialog wrapper

**Modified files:**
- `src-tauri/src/database/schema.sql` - Add tables/columns
- `src-tauri/src/commands/mod.rs` - Add structs, export settings
- `src-tauri/src/commands/validation.rs` - Add shared sanitize_folder_name
- `src-tauri/src/commands/file.rs` - Move files on create, add move_category
- `src-tauri/src/commands/category.rs` - Block delete with files, rename folders
- `src-tauri/src/lib.rs` - Register settings commands
- `src/types/index.ts` - Add in_storage, original_path, folder_name
- `src/lib/tauri.ts` - Add settings and file_move_category functions
- `src/routes/index.tsx` - Storage path check, warning banner
- `src/components/DynamicMetadataForm.tsx` - Category change confirmation
- `src/components/FilePicker.tsx` - Handle disabled prop

---

## Task 1: Database Schema Updates

**Files:**
- Modify: `src-tauri/src/database/schema.sql`

- [ ] **Step 1: Add app_settings table**

Add after the existing tables:

```sql
-- App settings table
CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Default storage path setting
INSERT INTO app_settings (key, value) VALUES ('storage_path', '');
```

- [ ] **Step 2: Add columns to files table**

Find the files table definition and add new columns:

```sql
-- Files table
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    category_id INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    file_status TEXT DEFAULT 'available' CHECK (file_status IN ('available', 'missing', 'moved')),
    in_storage BOOLEAN DEFAULT 0,
    original_path TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

- [ ] **Step 3: Add folder_name column to categories table**

Find the categories table and add:

```sql
-- Categories table
CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    icon TEXT,
    is_default BOOLEAN DEFAULT 0,
    folder_name TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

- [ ] **Step 4: Delete database and test**

Delete the existing database file to apply the new schema. The database file is typically located at:
- macOS/Linux: `~/.local/share/com.biblio.app/biblio.db` or in the app's data directory
- Windows: `%APPDATA%\com.biblio.app\biblio.db`

Or use the app's configuration to find the exact path. During development with `pnpm tauri:dev`, the database is usually created in the current working directory or a subdirectory.

Run: `pnpm tauri:dev`
Expected: App starts, database recreated with new schema

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/database/schema.sql
git commit -m "feat: add storage system database schema"
```

---

## Task 2: Update Rust Structs

**Files:**
- Modify: `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: Add in_storage and original_path to FileEntry**

Update the FileEntry struct:

```rust
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct FileEntry {
    pub id: i64,
    pub path: String,
    pub display_name: String,
    pub category_id: Option<i64>,
    pub file_status: String,
    pub in_storage: bool,
    pub original_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

- [ ] **Step 2: Add folder_name to Category struct**

Update the Category struct:

```rust
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub icon: Option<String>,
    pub is_default: bool,
    pub folder_name: Option<String>,
    pub created_at: String,
}
```

- [ ] **Step 3: Update FileWithDetails struct**

Update the FileWithDetails struct to include storage fields:

```rust
#[derive(Debug, Serialize)]
pub struct FileWithDetails {
    pub id: i64,
    pub path: String,
    pub display_name: String,
    pub category_id: Option<i64>,
    pub file_status: String,
    pub in_storage: bool,
    pub original_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub category: Option<Category>,
    pub tags: Vec<Tag>,
    pub authors: Vec<Author>,
    pub metadata: Vec<Metadata>,
}
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/mod.rs
git commit -m "feat: add storage fields to Rust structs"
```

---

## Task 3: Add Shared Utility Functions

**Files:**
- Modify: `src-tauri/src/commands/validation.rs`

**Note**: If `validation.rs` doesn't exist in your codebase, create it as a new file with the content below. If it exists, add the function to the existing file.

- [ ] **Step 1: Add sanitize_folder_name function**

Add to `src-tauri/src/commands/validation.rs`:

```rust
/// Sanitize a name for use as a folder name
pub fn sanitize_folder_name(name: &str) -> String {
    let invalid_chars = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    let mut sanitized: String = name
        .chars()
        .map(|c| if invalid_chars.contains(&c) { '_' } else { c })
        .collect();

    // Trim whitespace and dots
    sanitized = sanitized.trim().trim_matches('.').to_string();

    // Handle empty result
    if sanitized.is_empty() {
        sanitized = "Untitled".to_string();
    }

    // Limit length to 200 characters
    if sanitized.len() > 200 {
        sanitized = sanitized[..200].to_string();
    }

    // Use lowercase for cross-platform consistency
    sanitized.to_lowercase()
}
```

- [ ] **Step 2: Ensure validation module is exported in mod.rs**

If `validation` is not already exported, add to `src-tauri/src/commands/mod.rs`:

```rust
pub mod validation;
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/validation.rs src-tauri/src/commands/mod.rs
git commit -m "feat: add shared sanitize_folder_name utility"
```

---

## Task 4: Create Settings Commands

**Files:**
- Create: `src-tauri/src/commands/settings.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create settings.rs with get/set commands**

Create `src-tauri/src/commands/settings.rs`:

```rust
use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_sql::{DbPool, DbInstances};

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

#[tauri::command]
pub async fn settings_get(
    app: AppHandle,
    key: String,
) -> Result<Option<String>, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let result: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = ?"
    )
    .bind(&key)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(result.map(|r| r.0))
}

#[tauri::command]
pub async fn settings_set(
    app: AppHandle,
    key: String,
    value: String,
) -> Result<(), String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Special handling for storage_path: block change if files exist in storage
    if key == "storage_path" {
        let files_in_storage: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM files WHERE in_storage = 1"
        )
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

        if files_in_storage.0 > 0 {
            return Err("STORAGE_PATH_CHANGE_BLOCKED".to_string());
        }

        // Validate path is accessible
        if !value.is_empty() {
            let path = std::path::PathBuf::from(&value);
            if !path.exists() {
                return Err("STORAGE_PATH_NOT_FOUND".to_string());
            }
            // Block system directories
            let path_str = path.to_string_lossy();
            let lower_path = path_str.to_lowercase();

            // Unix system directories
            let unix_dangerous = ["/", "/system", "/usr", "/bin", "/etc"];
            for dangerous in unix_dangerous {
                if lower_path == dangerous || lower_path.starts_with(&format!("{}/", dangerous)) {
                    // Allow if it's a user subdirectory like /Users/...
                    if dangerous == "/" && lower_path.starts_with("/users/") {
                        continue;
                    }
                    return Err("STORAGE_PATH_SYSTEM_DIRECTORY".to_string());
                }
            }

            // Windows system directories (check for patterns like C:\Windows, D:\Program Files, etc.)
            let windows_patterns = ["\\windows", "\\program files", "\\program files (x86)"];
            for pattern in windows_patterns {
                if lower_path.contains(pattern) {
                    return Err("STORAGE_PATH_SYSTEM_DIRECTORY".to_string());
                }
            }
            // Try to create a test file to verify write permission
            let test_file = path.join(".biblio_test");
            if std::fs::write(&test_file, b"").is_err() {
                return Err("STORAGE_PATH_NOT_WRITABLE".to_string());
            }
            let _ = std::fs::remove_file(&test_file);
        }
    }

    sqlx::query(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?, ?)"
    )
    .bind(&key)
    .bind(&value)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn storage_get_path(app: AppHandle) -> Result<Option<String>, String> {
    settings_get(app, "storage_path".to_string()).await
}

#[tauri::command]
pub async fn storage_check_access(app: AppHandle) -> Result<bool, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let result: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'storage_path'"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let path = match result {
        Some((p,)) if !p.is_empty() => p,
        _ => return Ok(false),
    };

    let path = std::path::PathBuf::from(&path);
    Ok(path.exists() && path.is_dir())
}
```

- [ ] **Step 2: Export settings module in mod.rs**

Add to `src-tauri/src/commands/mod.rs` at the top:

```rust
pub mod settings;
```

- [ ] **Step 3: Register commands in lib.rs**

Find the `invoke_handler` in `src-tauri/src/lib.rs` and add:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    commands::settings::settings_get,
    commands::settings::settings_set,
    commands::settings::storage_get_path,
    commands::settings::storage_check_access,
])
```

- [ ] **Step 4: Test compilation**

Run: `cd src-tauri && cargo check`
Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/settings.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: add settings commands for storage path"
```

---

## Task 5: Update TypeScript Types

**Files:**
- Modify: `src/types/index.ts`

- [ ] **Step 1: Add in_storage and original_path to FileEntry**

Update the FileEntry interface:

```typescript
export interface FileEntry {
  id: number;
  path: string;
  display_name: string;
  category_id: number | null;
  file_status: FileStatus;
  in_storage: boolean;
  original_path: string | null;
  created_at: string;
  updated_at: string;
  category?: Category | null;
  tags?: Tag[];
  authors?: Author[];
  metadata?: Metadata[];
}
```

- [ ] **Step 2: Add folder_name to Category**

Update the Category interface:

```typescript
export interface Category {
  id: number;
  name: string;
  icon: string | null;
  is_default: boolean;
  folder_name: string | null;
  created_at: string;
}
```

- [ ] **Step 3: Commit**

```bash
git add src/types/index.ts
git commit -m "feat: add storage fields to TypeScript types"
```

---

## Task 6: Add Frontend Settings API

**Files:**
- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Add settings functions**

Add to `src/lib/tauri.ts`:

```typescript
// Settings API functions
export async function settingsGet(key: string): Promise<string | null> {
  return invoke('settings_get', { key });
}

export async function settingsSet(key: string, value: string): Promise<void> {
  return invoke('settings_set', { key, value });
}

export async function storageGetPath(): Promise<string | null> {
  return invoke('storage_get_path');
}

export async function storageCheckAccess(): Promise<boolean> {
  return invoke('storage_check_access');
}
```

- [ ] **Step 2: Add file_move_category function**

Add to the file section:

```typescript
export async function fileMoveCategory(file_id: number, new_category_id: number | null): Promise<{ success: boolean }> {
  return invoke('file_move_category', { fileId: file_id, newCategoryId: new_category_id });
}
```

- [ ] **Step 3: Add error code translations**

Add a helper function for translating error codes to user-friendly messages:

```typescript
// Error code translations
const ERROR_MESSAGES: Record<string, string> = {
  'STORAGE_PATH_NOT_CONFIGURED': 'Please configure a storage folder in settings first.',
  'STORAGE_PATH_CHANGE_BLOCKED': 'Cannot change storage path while files are stored. Remove all files first.',
  'STORAGE_PATH_NOT_FOUND': 'The selected folder does not exist.',
  'STORAGE_PATH_NOT_WRITABLE': 'Cannot write to the selected folder. Please choose another location.',
  'STORAGE_PATH_SYSTEM_DIRECTORY': 'Cannot use a system directory. Please choose another location.',
  'SOURCE_FILE_NOT_FOUND': 'The source file could not be found.',
  'FILE_ALREADY_IN_STORAGE': 'This file is already in the managed storage.',
  'FILE_NOT_IN_STORAGE': 'This file is not in managed storage.',
  'FILE_NOT_FOUND': 'The file was not found.',
  'CATEGORY_HAS_FILES': 'Cannot delete category with files. Move or delete files first.',
  'CATEGORY_NOT_FOUND': 'The selected category was not found.',
  'CATEGORY_FOLDER_NOT_SET': 'The category folder is not configured.',
  'CANNOT_DELETE_DEFAULT': 'Cannot delete the default category.',
  'PERMISSION_DENIED': 'Permission denied. Please check folder permissions.',
  'FILE_LOCKED': 'File is in use by another application.',
  'DISK_FULL': 'Not enough disk space to complete the operation.',
};

export function translateError(error: string): string {
  return ERROR_MESSAGES[error] || error;
}
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/tauri.ts
git commit -m "feat: add settings and file_move_category API functions"
```

---

## Task 7: Implement File Move Logic in Rust

**Files:**
- Modify: `src-tauri/src/commands/file.rs`

- [ ] **Step 1: Add helper functions for file movement**

Add at the top of `file.rs` after imports:

```rust
use std::path::PathBuf;
use std::fs;

use crate::commands::validation::sanitize_folder_name;
use tauri::Manager;
use tauri_plugin_sql::{DbPool, DbInstances};

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

const UNCATEGORIZED_FOLDER: &str = "_uncategorized";

/// Generate a unique filename if file already exists
fn get_unique_destination(dest: &PathBuf) -> PathBuf {
    if !dest.exists() {
        return dest.clone();
    }

    let parent = dest.parent().unwrap_or(std::path::Path::new("."));
    let stem = dest.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = dest.extension()
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

/// Move a file, handling cross-drive moves
/// Returns the final destination path
fn move_file(source: &PathBuf, dest: &PathBuf) -> Result<PathBuf, String> {
    let final_dest = get_unique_destination(dest);

    // Try rename first (fast, same filesystem)
    if fs::rename(source, &final_dest).is_ok() {
        return Ok(final_dest);
    }

    // Fall back to copy + delete (cross-drive)
    fs::copy(source, &final_dest)
        .map_err(|e| {
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

    fs::remove_file(source)
        .map_err(|e| format!("Failed to remove original: {}", e))?;

    Ok(final_dest)
}
```

- [ ] **Step 2: Update file_create to move files**

Replace the existing `file_create` function:

```rust
#[tauri::command]
pub async fn file_create(
    app: AppHandle,
    path: String,
    display_name: String,
    category_id: Option<i64>,
    tag_ids: Option<Vec<i64>>,
    author_ids: Option<Vec<i64>>,
    metadata: Option<Vec<MetadataInput>>,
) -> Result<FileCreateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let validated_name = validate_display_name(&display_name)?;

    // Get storage path
    let storage_path: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'storage_path'"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let storage_path = match storage_path {
        Some((p,)) if !p.is_empty() => p,
        _ => return Err("STORAGE_PATH_NOT_CONFIGURED".to_string()),
    };

    // Check source exists
    let source_path = PathBuf::from(&path);
    if !source_path.exists() {
        return Err("SOURCE_FILE_NOT_FOUND".to_string());
    }

    // Check source is not inside storage path
    let canonical_source = source_path.canonicalize()
        .map_err(|e| format!("Failed to resolve source path: {}", e))?;
    let canonical_storage = PathBuf::from(&storage_path).canonicalize()
        .map_err(|e| format!("Failed to resolve storage path: {}", e))?;

    if canonical_source.starts_with(&canonical_storage) {
        return Err("FILE_ALREADY_IN_STORAGE".to_string());
    }

    // Determine destination folder
    let folder_name = if let Some(cat_id) = category_id {
        // Get or compute folder_name for category
        let cat: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT folder_name FROM categories WHERE id = ?"
        )
        .bind(cat_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

        match cat {
            Some((Some(folder),)) => folder,
            Some((None,)) => {
                // Compute and save folder_name
                let cat_name: (String,) = sqlx::query_as(
                    "SELECT name FROM categories WHERE id = ?"
                )
                .bind(cat_id)
                .fetch_one(&pool)
                .await
                .map_err(|e| e.to_string())?;

                let sanitized = sanitize_folder_name(&cat_name.0);
                sqlx::query("UPDATE categories SET folder_name = ? WHERE id = ?")
                    .bind(&sanitized)
                    .bind(cat_id)
                    .execute(&pool)
                    .await
                    .map_err(|e| e.to_string())?;
                sanitized
            }
            None => return Err("CATEGORY_NOT_FOUND".to_string()),
        }
    } else {
        UNCATEGORIZED_FOLDER.to_string()
    };

    // Create destination folder
    let dest_folder = PathBuf::from(&storage_path).join(&folder_name);
    fs::create_dir_all(&dest_folder)
        .map_err(|e| format!("Failed to create folder: {}", e))?;

    // Move file
    let filename = source_path.file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;
    let dest_path = dest_folder.join(filename);
    let final_path = move_file(&source_path, &dest_path)?;

    // Save to database
    let result = sqlx::query(
        "INSERT INTO files (path, display_name, category_id, in_storage, original_path) VALUES (?, ?, ?, 1, ?)"
    )
    .bind(final_path.to_string_lossy().to_string())
    .bind(&validated_name)
    .bind(category_id)
    .bind(&path)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let file_id = result.last_insert_rowid();

    // Handle tags, authors, metadata (same as before)
    if let Some(tags) = tag_ids {
        for tag_id in tags {
            sqlx::query("INSERT INTO file_tags (file_id, tag_id) VALUES (?, ?)")
                .bind(file_id)
                .bind(tag_id)
                .execute(&pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    if let Some(authors) = author_ids {
        for author_id in authors {
            sqlx::query("INSERT INTO file_authors (file_id, author_id) VALUES (?, ?)")
                .bind(file_id)
                .bind(author_id)
                .execute(&pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    if let Some(meta) = metadata {
        for m in meta {
            sqlx::query("INSERT INTO metadata (file_id, key, value, data_type) VALUES (?, ?, ?, ?)")
                .bind(file_id)
                .bind(&m.key)
                .bind(&m.value)
                .bind(m.data_type.unwrap_or_else(|| "text".to_string()))
                .execute(&pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(FileCreateResponse { id: file_id })
}
```

- [ ] **Step 3: Update file_delete to handle in_storage**

Replace the existing `file_delete` function:

```rust
#[tauri::command]
pub async fn file_delete(
    app: AppHandle,
    id: i64,
) -> Result<FileDeleteResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Get file info
    let file: (String, bool) = sqlx::query_as(
        "SELECT path, in_storage FROM files WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("FILE_NOT_FOUND")?;

    // Delete from filesystem if in storage
    if file.1 {
        let path = PathBuf::from(&file.0);
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|e| format!("Failed to delete file: {}", e))?;
        }
    }

    // Delete from database
    sqlx::query("DELETE FROM files WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(FileDeleteResponse { success: true })
}
```

- [ ] **Step 4: Add file_move_category command**

Add after `file_delete`:

```rust
#[tauri::command]
pub async fn file_move_category(
    app: AppHandle,
    file_id: i64,
    new_category_id: Option<i64>,
) -> Result<FileMoveCategoryResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Get current file
    let file: (String, bool) = sqlx::query_as(
        "SELECT path, in_storage FROM files WHERE id = ?"
    )
    .bind(file_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("FILE_NOT_FOUND")?;

    if !file.1 {
        return Err("FILE_NOT_IN_STORAGE".to_string());
    }

    // Get storage path
    let storage_path: (String,) = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'storage_path'"
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;

    // Determine new folder
    let new_folder = if let Some(cat_id) = new_category_id {
        let cat: (Option<String>, String) = sqlx::query_as(
            "SELECT folder_name, name FROM categories WHERE id = ?"
        )
        .bind(cat_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

        match cat.0 {
            Some(folder) => folder,
            None => {
                // Compute and save folder_name for existing category
                let sanitized = sanitize_folder_name(&cat.1);
                sqlx::query("UPDATE categories SET folder_name = ? WHERE id = ?")
                    .bind(&sanitized)
                    .bind(cat_id)
                    .execute(&pool)
                    .await
                    .map_err(|e| e.to_string())?;
                sanitized
            }
        }
    } else {
        UNCATEGORIZED_FOLDER.to_string()
    };

    // Move file
    let current_path = PathBuf::from(&file.0);
    let filename = current_path.file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;
    let dest_folder = PathBuf::from(&storage_path.0).join(&new_folder);

    fs::create_dir_all(&dest_folder)
        .map_err(|e| format!("Failed to create folder: {}", e))?;

    let dest_path = dest_folder.join(filename);
    let final_path = move_file(&current_path, &dest_path)?;

    // Update database
    sqlx::query(
        "UPDATE files SET path = ?, category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
    )
    .bind(final_path.to_string_lossy().to_string())
    .bind(new_category_id)
    .bind(file_id)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(FileMoveCategoryResponse { success: true })
}

#[derive(Serialize)]
pub struct FileMoveCategoryResponse {
    pub success: bool,
}
```

- [ ] **Step 5: Update file_update to remove category_id**

Replace the `file_update` function:

```rust
#[tauri::command]
pub async fn file_update(
    app: AppHandle,
    id: i64,
    display_name: Option<String>,
) -> Result<FileUpdateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    if let Some(name) = display_name {
        let validated_name = validate_display_name(&name)?;
        sqlx::query("UPDATE files SET display_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(&validated_name)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(FileUpdateResponse { success: true })
}
```

**Note**: This removes `category_id` from `file_update`. Check that no frontend code passes `category_id` to `file_update`. Category changes must go through `file_move_category` to ensure proper file movement.

- [ ] **Step 6: Register file_move_category in lib.rs**

Add to `invoke_handler`:

```rust
commands::file::file_move_category,
```

- [ ] **Step 7: Test compilation**

Run: `cd src-tauri && cargo check`
Expected: No errors

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/commands/file.rs src-tauri/src/lib.rs
git commit -m "feat: implement file move and storage logic"
```

---

## Task 8: Update Category Commands

**Files:**
- Modify: `src-tauri/src/commands/category.rs`

- [ ] **Step 1: Update category_create to set folder_name**

Replace `category_create`:

```rust
use crate::commands::validation::sanitize_folder_name;

#[tauri::command]
pub async fn category_create(
    app: AppHandle,
    name: String,
    icon: Option<String>,
) -> Result<CategoryCreateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let validated_name = validate_category_name(&name)?;

    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM categories WHERE name = ?")
        .bind(&validated_name)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Err("CATEGORY_EXISTS".to_string());
    }

    // Generate unique folder name
    let base_folder = sanitize_folder_name(&validated_name);
    let folder_name = get_unique_folder_name(&pool, &base_folder).await?;

    let result = sqlx::query("INSERT INTO categories (name, icon, folder_name) VALUES (?, ?, ?)")
        .bind(&validated_name)
        .bind(&icon)
        .bind(&folder_name)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CategoryCreateResponse {
        id: result.last_insert_rowid(),
    })
}

async fn get_unique_folder_name(pool: &sqlx::SqlitePool, base: &str) -> Result<String, String> {
    let existing: Vec<(String,)> = sqlx::query_as(
        "SELECT folder_name FROM categories WHERE folder_name LIKE ?"
    )
    .bind(format!("{}%", base))
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if !existing.iter().any(|(f,)| f == base) {
        return Ok(base.to_string());
    }

    let mut counter = 1;
    loop {
        let candidate = format!("{}_{}", base, counter);
        if !existing.iter().any(|(f,)| f == &candidate) {
            return Ok(candidate);
        }
        counter += 1;
    }
}
```

- [ ] **Step 2: Update category_update to rename folders**

Replace `category_update`:

```rust
#[tauri::command]
pub async fn category_update(
    app: AppHandle,
    id: i64,
    name: Option<String>,
    icon: Option<String>,
) -> Result<CategoryUpdateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    if let Some(n) = name {
        let validated_name = validate_category_name(&n)?;

        // Check if name is actually changing
        let current: (String, Option<String>) = sqlx::query_as(
            "SELECT name, folder_name FROM categories WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

        if validated_name != current.0 {
            // Get storage path
            let storage_path: Option<(String,)> = sqlx::query_as(
                "SELECT value FROM app_settings WHERE key = 'storage_path'"
            )
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

            // Generate new folder name
            let base_folder = sanitize_folder_name(&validated_name);
            let new_folder = get_unique_folder_name(&pool, &base_folder).await?;

            // Rename folder if it exists
            if let Some((storage,)) = storage_path {
                if !storage.is_empty() {
                    if let Some(old_folder) = &current.1 {
                        let old_path = std::path::PathBuf::from(&storage).join(old_folder);
                        let new_path = std::path::PathBuf::from(&storage).join(&new_folder);

                        if old_path.exists() {
                            std::fs::rename(&old_path, &new_path)
                                .map_err(|e| format!("Failed to rename folder: {}", e))?;
                        }
                    }

                    // Update file paths
                    let files: Vec<(i64, String)> = sqlx::query_as(
                        "SELECT id, path FROM files WHERE category_id = ?"
                    )
                    .bind(id)
                    .fetch_all(&pool)
                    .await
                    .map_err(|e| e.to_string())?;

                    for (file_id, old_path) in files {
                        let path = std::path::PathBuf::from(&old_path);
                        if let Some(filename) = path.file_name() {
                            let new_file_path = std::path::PathBuf::from(&storage)
                                .join(&new_folder)
                                .join(filename);
                            sqlx::query("UPDATE files SET path = ? WHERE id = ?")
                                .bind(new_file_path.to_string_lossy().to_string())
                                .bind(file_id)
                                .execute(&pool)
                                .await
                                .map_err(|e| e.to_string())?;
                        }
                    }
                }
            }

            // Update category
            sqlx::query("UPDATE categories SET name = ?, folder_name = ? WHERE id = ?")
                .bind(&validated_name)
                .bind(&new_folder)
                .bind(id)
                .execute(&pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    if let Some(i) = icon {
        sqlx::query("UPDATE categories SET icon = ? WHERE id = ?")
            .bind(&i)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(CategoryUpdateResponse { success: true })
}
```

- [ ] **Step 3: Update category_delete to block when files exist**

Replace `category_delete`:

```rust
#[tauri::command]
pub async fn category_delete(
    app: AppHandle,
    id: i64,
) -> Result<CategoryDeleteResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let is_default: (bool,) = sqlx::query_as("SELECT is_default FROM categories WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if is_default.0 {
        return Err("CANNOT_DELETE_DEFAULT".to_string());
    }

    let affected: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files WHERE category_id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if affected.0 > 0 {
        return Err("CATEGORY_HAS_FILES".to_string());
    }

    sqlx::query("DELETE FROM categories WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CategoryDeleteResponse {
        success: true,
    })
}
```

- [ ] **Step 4: Update CategoryDeleteResponse**

Remove affected_files from response:

```rust
#[derive(serde::Serialize)]
pub struct CategoryDeleteResponse {
    pub success: bool,
}
```

- [ ] **Step 5: Test compilation**

Run: `cd src-tauri && cargo check`
Expected: No errors

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/category.rs
git commit -m "feat: add folder management to category commands"
```

---

## Task 9: Create Settings Dialog UI

**Files:**
- Create: `src/components/StoragePathSetting.tsx`
- Create: `src/components/SettingsDialog.tsx`

- [ ] **Step 1: Create StoragePathSetting component**

Create `src/components/StoragePathSetting.tsx`:

```tsx
import { useState, useEffect } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { settingsGet, settingsSet, translateError } from '@/lib/tauri';
import { FolderOpen, AlertCircle, Check } from 'lucide-react';

export function StoragePathSetting() {
  const [storagePath, setStoragePath] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  useEffect(() => {
    settingsGet('storage_path').then((path) => {
      setStoragePath(path);
      setLoading(false);
    });
  }, []);

  const handleSelectFolder = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Storage Folder',
    });

    if (selected && typeof selected === 'string') {
      setSaving(true);
      setError(null);
      setSuccess(false);

      try {
        await settingsSet('storage_path', selected);
        setStoragePath(selected);
        setSuccess(true);
        setTimeout(() => setSuccess(false), 3000);
      } catch (err) {
        const errorMsg = err instanceof Error ? err.message : String(err);
        setError(translateError(errorMsg));
      } finally {
        setSaving(false);
      }
    }
  };

  if (loading) {
    return <div className="text-sm text-muted-foreground">Loading...</div>;
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium">Storage Path</h3>
          <p className="text-xs text-muted-foreground">
            Files will be organized in category folders
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={handleSelectFolder}
          disabled={saving}
        >
          <FolderOpen className="h-4 w-4 mr-2" />
          {storagePath ? 'Change' : 'Select Folder'}
        </Button>
      </div>

      {storagePath ? (
        <div className="flex items-center gap-2">
          <Input
            value={storagePath}
            readOnly
            className="text-sm bg-muted"
          />
          {success && (
            <Check className="h-4 w-4 text-green-500" />
          )}
        </div>
      ) : (
        <div className="flex items-center gap-2 p-3 bg-yellow-50 dark:bg-yellow-950/20 rounded-md border border-yellow-200 dark:border-yellow-800">
          <AlertCircle className="h-4 w-4 text-yellow-600 dark:text-yellow-500" />
          <span className="text-sm text-yellow-800 dark:text-yellow-200">
            Select a storage folder to start adding files
          </span>
        </div>
      )}

      {error && (
        <p className="text-sm text-red-500">{error}</p>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Create SettingsDialog component**

Create `src/components/SettingsDialog.tsx`:

```tsx
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Settings } from 'lucide-react';
import { StoragePathSetting } from './StoragePathSetting';

interface SettingsDialogProps {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        <Button variant="ghost" size="icon">
          <Settings className="h-5 w-5" />
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
        </DialogHeader>
        <div className="py-4">
          <StoragePathSetting />
        </div>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 3: Install tauri-plugin-dialog if needed**

Run: `pnpm add @tauri-apps/plugin-dialog`
Run: `cd src-tauri && cargo add tauri-plugin-dialog`

**Note**: The dialog plugin is already registered in `lib.rs` as `tauri_plugin_dialog::init()`. If not present, add it to the builder.

- [ ] **Step 4: Commit**

```bash
git add src/components/StoragePathSetting.tsx src/components/SettingsDialog.tsx src-tauri/Cargo.toml package.json pnpm-lock.yaml
git commit -m "feat: add settings dialog with storage path configuration"
```

---

## Task 10: Add Storage Path Check to Home Page

**Files:**
- Modify: `src/routes/index.tsx`

- [ ] **Step 1: Add storage path state and check**

Add to the imports at the top:

```tsx
import { useState, useEffect, useCallback } from 'react';
import { storageGetPath, storageCheckAccess } from '@/lib/tauri';
import { SettingsDialog } from '@/components/SettingsDialog';
import { AlertCircle } from 'lucide-react';
```

Add state after other useState calls:

```tsx
const [storagePathConfigured, setStoragePathConfigured] = useState<boolean | null>(null);
const [storagePathAccessible, setStoragePathAccessible] = useState(true);
const [settingsOpen, setSettingsOpen] = useState(false);
```

Add the check function and useEffect:

```tsx
const checkStoragePath = useCallback(async () => {
  const path = await storageGetPath();
  if (path && path !== '') {
    const accessible = await storageCheckAccess();
    setStoragePathConfigured(true);
    setStoragePathAccessible(accessible);
  } else {
    setStoragePathConfigured(false);
    setStoragePathAccessible(true);
  }
}, []);

// In the useEffect
useEffect(() => {
  void loadCategories();
  void loadTags();
  void loadAuthors();
  void checkStoragePath();
  void loadFiles(null);
}, [loadCategories, loadTags, loadAuthors, loadFiles, checkStoragePath]);
```

- [ ] **Step 2: Add warning banners**

Add after the main header div:

```tsx
{storagePathConfigured === false && (
  <div className="flex items-center gap-2 p-3 mb-4 bg-yellow-50 dark:bg-yellow-950/20 rounded-md border border-yellow-200 dark:border-yellow-800">
    <AlertCircle className="h-4 w-4 text-yellow-600 dark:text-yellow-500" />
    <span className="text-sm text-yellow-800 dark:text-yellow-200">
      Configure a storage folder in settings to start adding files
    </span>
  </div>
)}

{storagePathConfigured === true && !storagePathAccessible && (
  <div className="flex items-center gap-2 p-3 mb-4 bg-red-50 dark:bg-red-950/20 rounded-md border border-red-200 dark:border-red-800">
    <AlertCircle className="h-4 w-4 text-red-600 dark:text-red-500" />
    <span className="text-sm text-red-800 dark:text-red-200">
      Storage path is not accessible.{' '}
      <button className="underline" onClick={() => setSettingsOpen(true)}>
        Reconfigure
      </button>
    </span>
  </div>
)}
```

- [ ] **Step 3: Add Settings button to header**

Update the header to include the settings button:

```tsx
<div className="flex justify-between items-center mb-6">
  <div>
    <h1 className="text-2xl font-bold">Library</h1>
    <p className="text-muted-foreground">{total} files</p>
  </div>
  <div className="flex items-center gap-2">
    <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    <FilePicker onFilesSelected={handleFilesSelected} disabled={storagePathConfigured === false} />
  </div>
</div>
```

- [ ] **Step 4: Disable FilePicker when not configured**

The FilePicker already has the disabled prop added. Make sure it shows visually:

```tsx
<FilePicker
  onFilesSelected={handleFilesSelected}
  disabled={storagePathConfigured === false}
/>
```

- [ ] **Step 5: Update FilePicker component to handle disabled prop**

Read `src/components/FilePicker.tsx` and update it to handle the disabled prop:

```tsx
import { open } from '@tauri-apps/plugin-dialog';
import { Button } from '@/components/ui/button';
import { FileUp } from 'lucide-react';

interface FilePickerProps {
  onFilesSelected: (paths: string[]) => void;
  disabled?: boolean;
}

export function FilePicker({ onFilesSelected, disabled }: FilePickerProps) {
  const handleClick = async () => {
    const selected = await open({
      multiple: true,
      title: 'Select files to add',
    });

    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      onFilesSelected(paths);
    }
  };

  return (
    <Button onClick={handleClick} disabled={disabled}>
      <FileUp className="h-4 w-4 mr-2" />
      Add Files
    </Button>
  );
}
```

- [ ] **Step 6: Commit**

```bash
git add src/routes/index.tsx src/components/FilePicker.tsx
git commit -m "feat: add storage path check and warning to home page"
```

---

## Task 11: Update DynamicMetadataForm for Category Change

**Files:**
- Modify: `src/components/DynamicMetadataForm.tsx`

- [ ] **Step 1: Add fileId and onCategoryChange props**

Update the interface:

```tsx
interface DynamicMetadataFormProps {
  values: DynamicMetadataFormValues;
  onChange: (values: DynamicMetadataFormValues) => void;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onCategoryCreated?: (category: Category) => void;
  onTagCreate?: (name: string) => Promise<Tag>;
  onAuthorCreate?: (name: string) => Promise<Author>;
  fileId?: number;
  inStorage?: boolean;
  onCategoryChange?: (newCategoryId: number | null) => Promise<void>;
}
```

- [ ] **Step 2: Add confirmation dialog for category change**

Add imports:

```tsx
import { useState } from 'react';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
```

Add state in component:

```tsx
const [pendingCategoryId, setPendingCategoryId] = useState<number | null>(null);
const [isMoving, setIsMoving] = useState(false);
```

- [ ] **Step 3: Update handleCategoryChange**

```tsx
const handleCategoryChange = (category_id: number | null) => {
  // If editing existing file that's in storage, prompt for move
  if (fileId && inStorage && onCategoryChange && values.category_id !== category_id) {
    setPendingCategoryId(category_id);
  } else {
    onChange({ ...values, category_id });
  }
};
```

- [ ] **Step 4: Add confirmation dialog handlers**

```tsx
const handleConfirmMove = async () => {
  if (!onCategoryChange || pendingCategoryId === null) return;

  setIsMoving(true);
  try {
    await onCategoryChange(pendingCategoryId);
    onChange({ ...values, category_id: pendingCategoryId });
    setPendingCategoryId(null); // Close dialog on success
  } catch (error) {
    console.error('Failed to move file:', error);
    // Don't close dialog or clear pendingCategoryId so user can retry
  } finally {
    setIsMoving(false);
  }
};

const handleCancelMove = () => {
  setPendingCategoryId(null);
};
```

- [ ] **Step 5: Add AlertDialog to render**

Add before the closing `</div>`:

```tsx
<AlertDialog open={pendingCategoryId !== null} onOpenChange={(open) => { if (!open) setPendingCategoryId(null); }}>
  <AlertDialogContent>
    <AlertDialogHeader>
      <AlertDialogTitle>Move file to new category folder?</AlertDialogTitle>
      <AlertDialogDescription>
        The file will be moved to the new category's folder. This cannot be undone.
      </AlertDialogDescription>
    </AlertDialogHeader>
    <AlertDialogFooter>
      <AlertDialogCancel onClick={handleCancelMove}>Cancel</AlertDialogCancel>
      <Button onClick={handleConfirmMove} disabled={isMoving}>
        {isMoving ? 'Moving...' : 'Move File'}
      </Button>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
```

- [ ] **Step 6: Create alert-dialog UI component if needed**

Check if `src/components/ui/alert-dialog.tsx` exists. If not, create it:

```tsx
import * as React from "react"
import * as AlertDialogPrimitive from "@radix-ui/react-alert-dialog"

import { cn } from "@/lib/utils"

const AlertDialog = AlertDialogPrimitive.Root
const AlertDialogTrigger = AlertDialogPrimitive.Trigger
const AlertDialogPortal = AlertDialogPrimitive.Portal

const AlertDialogOverlay = React.forwardRef<
  React.ElementRef<typeof AlertDialogPrimitive.Overlay>,
  React.ComponentPropsWithoutRef<typeof AlertDialogPrimitive.Overlay>
>(({ className, ...props }, ref) => (
  <AlertDialogPrimitive.Overlay
    className={cn(
      "fixed inset-0 z-50 bg-black/80 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
      className
    )}
    {...props}
    ref={ref}
  />
))
AlertDialogOverlay.displayName = AlertDialogPrimitive.Overlay.displayName

const AlertDialogContent = React.forwardRef<
  React.ElementRef<typeof AlertDialogPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof AlertDialogPrimitive.Content>
>(({ className, ...props }, ref) => (
  <AlertDialogPortal>
    <AlertDialogOverlay />
    <AlertDialogPrimitive.Content
      ref={ref}
      className={cn(
        "fixed left-[50%] top-[50%] z-50 grid w-full max-w-lg translate-x-[-50%] translate-y-[-50%] gap-4 border bg-background p-6 shadow-lg duration-200 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[state=closed]:slide-out-to-left-1/2 data-[state=closed]:slide-out-to-top-[48%] data-[state=open]:slide-in-from-left-1/2 data-[state=open]:slide-in-from-top-[48%] sm:rounded-lg",
        className
      )}
      {...props}
    />
  </AlertDialogPortal>
))
AlertDialogContent.displayName = AlertDialogPrimitive.Content.displayName

const AlertDialogHeader = ({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) => (
  <div
    className={cn(
      "flex flex-col space-y-2 text-center sm:text-left",
      className
    )}
    {...props}
  />
)
AlertDialogHeader.displayName = "AlertDialogHeader"

const AlertDialogFooter = ({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) => (
  <div
    className={cn(
      "flex flex-col-reverse sm:flex-row sm:justify-end sm:space-x-2",
      className
    )}
    {...props}
  />
)
AlertDialogFooter.displayName = "AlertDialogFooter"

const AlertDialogTitle = React.forwardRef<
  React.ElementRef<typeof AlertDialogPrimitive.Title>,
  React.ComponentPropsWithoutRef<typeof AlertDialogPrimitive.Title>
>(({ className, ...props }, ref) => (
  <AlertDialogPrimitive.Title
    ref={ref}
    className={cn("text-lg font-semibold", className)}
    {...props}
  />
))
AlertDialogTitle.displayName = AlertDialogPrimitive.Title.displayName

const AlertDialogDescription = React.forwardRef<
  React.ElementRef<typeof AlertDialogPrimitive.Description>,
  React.ComponentPropsWithoutRef<typeof AlertDialogPrimitive.Description>
>(({ className, ...props }, ref) => (
  <AlertDialogPrimitive.Description
    ref={ref}
    className={cn("text-sm text-muted-foreground", className)}
    {...props}
  />
))
AlertDialogDescription.displayName =
  AlertDialogPrimitive.Description.displayName

const AlertDialogAction = React.forwardRef<
  React.ElementRef<typeof AlertDialogPrimitive.Action>,
  React.ComponentPropsWithoutRef<typeof AlertDialogPrimitive.Action>
>(({ className, ...props }, ref) => (
  <AlertDialogPrimitive.Action
    ref={ref}
    className={cn(
      "inline-flex h-10 items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-semibold text-primary-foreground ring-offset-background transition-colors hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50",
      className
    )}
    {...props}
  />
))
AlertDialogAction.displayName = AlertDialogPrimitive.Action.displayName

const AlertDialogCancel = React.forwardRef<
  React.ElementRef<typeof AlertDialogPrimitive.Cancel>,
  React.ComponentPropsWithoutRef<typeof AlertDialogPrimitive.Cancel>
>(({ className, ...props }, ref) => (
  <AlertDialogPrimitive.Cancel
    ref={ref}
    className={cn(
      "mt-2 inline-flex h-10 items-center justify-center rounded-md border border-input bg-background px-4 py-2 text-sm font-semibold ring-offset-background transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 sm:mt-0",
      className
    )}
    {...props}
  />
))
AlertDialogCancel.displayName = AlertDialogPrimitive.Cancel.displayName

export {
  AlertDialog,
  AlertDialogPortal,
  AlertDialogOverlay,
  AlertDialogTrigger,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogFooter,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogAction,
  AlertDialogCancel,
}
```

- [ ] **Step 7: Install radix alert-dialog if needed**

Run: `pnpm add @radix-ui/react-alert-dialog`

- [ ] **Step 8: Commit**

```bash
git add src/components/DynamicMetadataForm.tsx src/components/ui/alert-dialog.tsx package.json pnpm-lock.yaml
git commit -m "feat: add category change confirmation dialog"
```

---

## Task 12: Wire Up Category Change in FileEditDialog

**Files:**
- Modify: `src/components/FileEditDialog.tsx`

- [ ] **Step 1: Add file_move_category import and handler with error handling**

Add imports:

```tsx
import { fileMoveCategory, translateError } from '@/lib/tauri';
import { useState } from 'react';
```

Add state for error:

```tsx
const [moveError, setMoveError] = useState<string | null>(null);
```

Add handler with error handling:

```tsx
const handleCategoryChange = async (newCategoryId: number | null) => {
  if (!file) return;
  setMoveError(null);
  try {
    await fileMoveCategory(file.id, newCategoryId);
  } catch (error) {
    const errorMsg = error instanceof Error ? error.message : String(error);
    setMoveError(translateError(errorMsg));
    throw error; // Re-throw to let the form know it failed
  }
};
```

- [ ] **Step 2: Pass props to DynamicMetadataForm and show errors**

Update the DynamicMetadataForm usage:

```tsx
<DynamicMetadataForm
  values={formValues}
  onChange={setFormValues}
  categories={categories}
  tags={tags}
  authors={authors}
  onCategoryCreated={handleCategoryCreated}
  onTagCreate={handleTagCreate}
  onAuthorCreate={handleAuthorCreate}
  fileId={file?.id}
  inStorage={file?.in_storage}
  onCategoryChange={handleCategoryChange}
/>
{moveError && (
  <p className="text-sm text-red-500 mt-2">{moveError}</p>
)}
```

- [ ] **Step 3: Commit**

```bash
git add src/components/FileEditDialog.tsx
git commit -m "feat: wire up category change with file move"
```

---

## Task 13: Update SQL Queries for New Fields

**Files:**
- Modify: `src-tauri/src/commands/file.rs`
- Modify: `src-tauri/src/commands/category.rs`

- [ ] **Step 1: Update file_list query**

Find `file_list` function and replace the query string:

```rust
let mut query = String::from(
    "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, created_at, updated_at FROM files WHERE 1=1"
);
```

- [ ] **Step 2: Update file_get query**

Find `file_get` function and replace the query:

```rust
let file: FileEntry = sqlx::query_as(
    "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, created_at, updated_at FROM files WHERE id = ?"
)
.bind(id)
.fetch_optional(&pool)
.await
.map_err(|e| e.to_string())?
.ok_or("File not found")?;
```

Also update the FileWithDetails return - the struct already has the fields from Task 2 Step 3.

- [ ] **Step 3: Update file_search queries**

Find `file_search` function and replace both branches:

```rust
let sql = if query.is_empty() {
    "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, created_at, updated_at FROM files WHERE 1=1"
} else {
    "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status, f.in_storage, f.original_path, f.created_at, f.updated_at \
     FROM files f \
     JOIN files_fts ON files_fts.rowid = f.id \
     WHERE files_fts MATCH ?"
};
```

- [ ] **Step 4: Update file_check_status queries**

Find `file_check_status` and update both queries:

```rust
// Inside the Some(ids) branch:
let query = format!(
    "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, created_at, updated_at FROM files WHERE id IN ({})",
    placeholders
);

// Inside the None branch:
sqlx::query_as("SELECT id, path, display_name, category_id, file_status, in_storage, original_path, created_at, updated_at FROM files")
```

- [ ] **Step 5: Update category_list query**

In `src-tauri/src/commands/category.rs`, find `category_list` and update:

```rust
sqlx::query_as("SELECT id, name, icon, is_default, folder_name, created_at FROM categories ORDER BY name")
```

**Note**: Existing categories will have `folder_name = NULL`. The code in Task 7 handles this by computing the folder name on first use.

- [ ] **Step 6: Update category_get query**

```rust
sqlx::query_as("SELECT id, name, icon, is_default, folder_name, created_at FROM categories WHERE id = ?")
```

- [ ] **Step 7: Test compilation and run**

Run: `cd src-tauri && cargo check`
Expected: No errors

Run: `pnpm tauri:dev`
Expected: App starts, can configure storage path and add files

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/commands/file.rs src-tauri/src/commands/category.rs
git commit -m "feat: update SQL queries for new storage fields"
```

---

## Task 14: Final Testing and Polish

- [ ] **Step 1: Test complete flow**

1. Start the app: `pnpm tauri:dev`
2. Open settings, configure storage path
3. Add a file with a category
4. Verify file moved to `{storage}/{category}/`
5. Change the file's category
6. Verify file moved to new folder
7. Delete the file
8. Verify file removed from filesystem

- [ ] **Step 2: Test error cases**

1. Try to add file without configuring storage path → verify disabled state
2. Try to add file from inside storage path → verify rejection error
3. Try to delete category with files → verify blocked with error message
4. Try to delete category with no files → verify successful deletion
5. Rename category with files → verify folder renamed and file paths updated

- [ ] **Step 3: Run typecheck and tests**

Run: `pnpm typecheck`
Expected: No errors

Run: `pnpm test:run`
Expected: All tests pass

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "feat: complete local media storage system"
```

---

## Summary

This implementation adds:
- Configurable storage path in settings
- Files moved to category folders on import
- Category change moves files between folders
- Category deletion blocked when files exist
- Category rename renames folders
- File deletion removes from filesystem when in storage
- Warning banner when storage not configured

**Known Limitation**: Full SQL transaction support with filesystem rollback is not implemented in this version. File operations proceed in sequence (filesystem first, then database). If a database operation fails after a successful filesystem operation, manual intervention may be required. This will be addressed in a future enhancement with proper transaction handling and recovery mechanisms.