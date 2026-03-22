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

**Cross-platform path handling**:
- Use `std::path::canonicalize()` for path comparisons
- Normalize paths before prefix check (storage_path containment)
- Use `std::path::PathBuf` for all path operations
- Handle Windows drive letters and UNC paths explicitly

## Architecture

### Database Schema

**Note**: Schema updates are applied in-place to `schema.sql`. No migration files needed during development.

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

- New command: `file_delete_with_storage(file_id: number)`
  1. Delete file from filesystem
  2. Delete from database (cascades to metadata, tags, authors, cover)

**Updated: `src-tauri/src/commands/category.rs`**
- `category_update` changes:
  1. Check if files exist in this category
  2. If files exist and name changed: rename category folder
  3. Sanitize new name for filesystem
  4. Update all file paths in database

- `category_delete` changes:
  1. Check if any files have this category_id
  2. If yes: return error, block deletion
  3. If no: proceed with deletion

### Frontend Components

**New: `src/components/StoragePathSetting.tsx`**
- Display current storage path
- Button to select folder via Tauri `open` dialog
- Save via `settings_set('storage_path', path)`
- Show warning if not configured

**Updated: `src/routes/index.tsx`**
- On mount, check if storage_path is configured
- Show warning banner if not configured
- Disable file picker button until configured

**Updated: `src/components/DynamicMetadataForm.tsx`**
- On category change for existing file, prompt to move file
- Call `file_move_category` if user confirms

### Error Handling

| Scenario | Behavior |
|----------|----------|
| Storage path not configured | Disable import, show setup prompt |
| Source file doesn't exist | Error message, don't register |
| File already in storage_path | Reject with error message |
| Destination folder creation fails | Error message, don't register |
| File exists at destination | Auto-rename: `filename (1).ext`, `filename (2).ext`, etc. |
| Move operation fails | Rollback database, leave file in place |
| Storage path inaccessible | Error on import, show in UI |
| Category folder missing | Auto-create when moving files |

## Implementation Order

1. Update `src-tauri/src/database/schema.sql` with new tables/columns
2. Create `src-tauri/src/commands/settings.rs` with get/set
3. Export settings module in `mod.rs`, register in `lib.rs`
4. Add settings functions to `src/lib/tauri.ts`
5. Update `file_create` in `file.rs` to move files
6. Add `file_move_category` and `file_delete_with_storage` commands
7. Create `StoragePathSetting.tsx` component
8. Add storage path check and warning to `routes/index.tsx`
9. Update `DynamicMetadataForm.tsx` for category change handling
10. Add error handling and validation throughout

## Testing

- Import file → verify moved to correct category folder
- Change category → verify file moved to new folder
- Try importing file from storage_path → verify rejection
- Delete file → verify removed from filesystem and database
- Configure storage path → verify setting persisted