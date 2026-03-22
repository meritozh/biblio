# Local Media Storage System

**Date**: 2026-03-22

## Overview

Transform biblio into a local media management desktop application that moves files into a configurable managed storage path, organized by category.

## Requirements

### Core Functionality
- Configurable storage path (global setting)
- Files moved to `{storage_path}/{category_name}/` on import
- Files automatically reorganized when category changes
- Support media types: text, epub, comic zip (external app integration only)

### Constraints
- Reject files already inside the managed storage path
- Storage path must be configured before importing files
- Files organized by category folders only (no sub-folders by author/date)

### Edge Cases & Scenarios

**Existing files in database**: Files added before this feature will have `in_storage = false` (default). They remain at their original paths and are not moved. Only newly imported files are moved to storage.

**Storage path change**: If user changes storage_path after files are already stored:
- Block the change with error: "Cannot change storage path while files are stored. Remove all files first."
- Alternative: Offer to migrate files (future enhancement)

**Category deletion**: When a category is deleted:
- If category has files: Block deletion with error "Cannot delete category with files. Move or delete files first."
- If category has no files: Allow deletion, folder remains (cleanup on app restart or manual deletion)

**Category rename**: When a category name changes:
- Rename the category folder from old name to new name
- Update all file paths in database
- If new folder name conflicts: Auto-rename with suffix

**Category name sanitization**: Folder names derived from category names must be filesystem-safe:
- Replace invalid characters (`/`, `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|`) with underscore `_`
- Trim leading/trailing whitespace and dots
- Empty name after sanitization → use "Untitled"
- Maximum length: 200 characters (leave room for filename)
- **Collision handling**: If sanitized name conflicts with existing category folder, append suffix: `Category_1`, `Category_2`, etc.
- Sanitized folder name stored in `categories.folder_name` column (new column) for tracking

**"Uncategorized" folder**:
- When a file has no category assigned (`category_id = null`), store in folder named `"_uncategorized"` (underscore prefix prevents collision with user-created "Uncategorized" category)
- This is NOT a special category in the database - it's just a default folder name
- Folder name is derived from a constant, not from category data

**Cross-platform path handling**:

### Database Schema

**Note**: During active development, schema changes are applied in-place. Delete the database file to apply fresh schema. For production release, proper migrations will be created.

New `app_settings` table:
```sql
CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

New columns on `files` table:
```sql
ALTER TABLE files ADD COLUMN in_storage BOOLEAN DEFAULT 0;
ALTER TABLE files ADD COLUMN original_path TEXT;
```

New column on `categories` table:
```sql
ALTER TABLE categories ADD COLUMN folder_name TEXT;
```
- Stores the sanitized, unique folder name for the category
- Set automatically on category creation/update
- Used for file path construction instead of computing from name

Default setting:
```sql
INSERT INTO app_settings (key, value) VALUES ('storage_path', '');
```

### Backend Commands

**New file: `src-tauri/src/commands/settings.rs`**
- `settings_get(key: string) -> Option<string>`
- `settings_set(key: string, value: string) -> Result<()>`
- `storage_get_path() -> Option<string>` - convenience wrapper

**Updated: `src-tauri/src/commands/file.rs`**
- `file_create` changes:
  1. Get storage_path from settings
  2. If not configured, return error
  3. Check if source path is inside storage_path (prefix check) - reject if so
  4. Determine category folder name (use "Uncategorized" if no category)
  5. Create `{storage_path}/{category_name}/` if not exists
  6. Move file to destination (auto-rename if exists)
  7. Save to database with `path = new_path`, `in_storage = true`, `original_path = source_path`

- New command: `file_move_category(file_id: number, new_category_id: number | null)`
  1. Get file and current category
  2. Get new category name
  3. Move file from `{storage_path}/{old_category}/file` to `{storage_path}/{new_category}/file`
  4. Update database with new path

- Update `file_delete` command (replace existing, not new command):
  - If `in_storage = true`: delete file from filesystem, then delete from database
  - If `in_storage = false`: delete only from database (file stays at original location)
  - Use SQL transaction to ensure atomicity

**Updated: `src-tauri/src/commands/category.rs`**
- `category_update` changes:
  1. Check if files exist in this category
  2. If files exist and name changed: rename category folder
  3. Sanitize new name for filesystem
  4. Update all file paths in database

- `category_delete` changes (**breaking change**):
  1. Check if any files have this category_id
  2. If yes: return error, block deletion
  3. If no: proceed with deletion
  - Previous behavior: Deleted category and unassigned files (files kept `category_id = null`)
  - New behavior: Must move/delete files before deleting category

### Frontend Components

**New: `src/components/StoragePathSetting.tsx`**
- Display current storage path
- Button to select folder via Tauri `open` dialog
- Save via `settings_set('storage_path', path)`
- Show warning if not configured
- Validate path is writable before saving
- **Placement**: In a settings dialog accessed from the main header (gear icon)

**New: `src/components/SettingsDialog.tsx`**
- Dialog wrapper for app settings
- Contains `StoragePathSetting` as the first setting
- Triggered by settings icon in main header

**Updated: `src/routes/index.tsx`**
- On mount, check if storage_path is configured
- Show warning banner if not configured
- Disable file picker button until configured

**Updated: `src/components/DynamicMetadataForm.tsx`**
- Category change detection: Check if `fileId` prop is provided (indicates existing file)
- If category changes on existing file:
  - Show confirmation dialog: "Move file to new category folder?"
  - If user confirms: call `file_move_category(fileId, newCategoryId)`
  - If user declines: update database only (file stays in old folder, `path` unchanged)
- Only prompt for files with `in_storage = true` (tracked via file data prop)

**Updated: `src-tauri/src/commands/file.rs`**
- `file_update` changes: Remove `category_id` from allowed updates
- Category changes must go through `file_move_category` to ensure file movement

### Error Handling

**All file operations use SQL transactions** to ensure atomicity. If filesystem operation fails, database changes are rolled back.

| Scenario | Behavior |
|----------|----------|
| Storage path not configured | Disable import, show setup prompt |
| Storage path not writable | Validation error when setting path |
| Storage path is system directory | Validation error (block common paths: `/`, `/System`, `/Windows`, etc.) |
| Storage path becomes inaccessible | **Detection**: Check on app startup and before each import. Show error banner, disable import, show "Reconfigure" button |
| Source file doesn't exist | Error message, don't register |
| File already in storage_path | Reject with error message |
| Destination folder creation fails | Error message, don't register |
| File exists at destination | Auto-rename: `filename (1).ext`, `filename (2).ext`, etc. For files without extension: `filename (1)`, `filename (2)` |
| Move operation fails (any reason) | Rollback transaction, leave file in place, show error |
| Permission denied during move | Error message with suggestion to check permissions |
| File locked by another process | Error message: "File is in use by another application" |
| Disk full during move | Error message: "Not enough disk space" |
| Cross-drive move (Windows) | Use copy+delete instead of rename (Rust `fs::copy` + `fs::remove_file`) |
| Category folder missing | Auto-create when moving files |

## Implementation Order

1. Update `src-tauri/src/database/schema.sql` with new tables/columns
2. Update `FileEntry` struct in `mod.rs` to include `in_storage`, `original_path` fields
3. Update `Category` struct in `mod.rs` to include `folder_name` field
4. Update TypeScript types in `src/types/index.ts` to match
5. Create `src-tauri/src/commands/settings.rs` with get/set
6. Export settings module in `mod.rs`, register in `lib.rs`
7. Add settings functions to `src/lib/tauri.ts`
8. Update `file_create` in `file.rs` to move files with transaction support
9. Update `file_delete` to handle `in_storage` flag
10. Add `file_move_category` command
11. Remove `category_id` from `file_update` allowed fields
12. Update `category_update` to rename folders and track `folder_name`
13. Update `category_delete` to block when files exist
14. Create `StoragePathSetting.tsx` and `SettingsDialog.tsx` components
15. Add storage path check and warning to `routes/index.tsx`
16. Update `DynamicMetadataForm.tsx` for category change handling with confirmation dialog
17. Add comprehensive error handling throughout

## Testing

- Import file → verify moved to correct category folder
- Change category → verify file moved to new folder
- Try importing file from storage_path → verify rejection
- Delete file → verify removed from filesystem and database
- Configure storage path → verify setting persisted