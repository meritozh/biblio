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

## Architecture

### Database Schema

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
| File exists at destination | Auto-rename with `(1)`, `(2)` suffix |
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