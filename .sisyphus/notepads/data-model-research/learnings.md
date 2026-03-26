# Biblio Data Model Research

## Entity Relationship Diagram (Conceptual)

```
┌─────────────────┐       ┌─────────────────┐
│   categories    │       │     files       │
├─────────────────┤       ├─────────────────┤
│ id (PK)         │◄──────│ category_id(FK) │
│ name (UNIQUE)   │       │ id (PK)         │
│ icon            │       │ path (UNIQUE)   │
│ is_default      │       │ display_name    │
│ folder_name     │       │ file_status     │
│ created_at      │       │ in_storage      │
└─────────────────┘       │ original_path   │
                          │ created_at      │
                          │ updated_at      │
                          └────────┬────────┘
                                   │
          ┌────────────────────────┼────────────────────────┐
          │                        │                        │
          ▼                        ▼                        ▼
┌─────────────────┐       ┌─────────────────┐       ┌─────────────────┐
│   file_tags     │       │  file_authors   │       │    metadata     │
├─────────────────┤       ├─────────────────┤       ├─────────────────┤
│ file_id (FK)    │       │ file_id (FK)    │       │ id (PK)         │
│ tag_id (FK)     │       │ author_id (FK)  │       │ file_id (FK)    │
│ (PK: file+tag)  │       │ (PK: file+auth) │       │ key             │
└────────┬────────┘       └────────┬────────┘       │ value           │
         │                         │                │ data_type       │
         ▼                         ▼                │ (UNIQUE:file+key)│
┌─────────────────┐       ┌─────────────────┐       └─────────────────┘
│      tags       │       │    authors      │
├─────────────────┤       ├─────────────────┤
│ id (PK)         │       │ id (PK)         │
│ name (UNIQUE)   │       │ name (UNIQUE)   │
│ color           │       │ created_at      │
│ created_at      │       └─────────────────┘
└─────────────────┘

                          ┌─────────────────┐
                          │     covers      │
                          ├─────────────────┤
                          │ file_id (PK,FK) │
                          │ data (BLOB)     │
                          │ mime_type       │
                          │ created_at      │
                          └─────────────────┘

┌─────────────────┐
│   app_settings  │
├─────────────────┤
│ key (PK)        │
│ value           │
└─────────────────┘
```

## Database Schema

### Core Tables

#### categories

| Column      | Type     | Constraints               |
| ----------- | -------- | ------------------------- |
| id          | INTEGER  | PRIMARY KEY AUTOINCREMENT |
| name        | TEXT     | NOT NULL, UNIQUE          |
| icon        | TEXT     | NULLABLE                  |
| is_default  | BOOLEAN  | DEFAULT 0                 |
| folder_name | TEXT     | NULLABLE                  |
| created_at  | DATETIME | DEFAULT CURRENT_TIMESTAMP |

#### files

| Column        | Type     | Constraints                                |
| ------------- | -------- | ------------------------------------------ |
| id            | INTEGER  | PRIMARY KEY AUTOINCREMENT                  |
| path          | TEXT     | NOT NULL, UNIQUE                           |
| display_name  | TEXT     | NOT NULL                                   |
| category_id   | INTEGER  | FK → categories(id) ON DELETE SET NULL     |
| file_status   | TEXT     | CHECK IN ('available', 'missing', 'moved') |
| in_storage    | BOOLEAN  | DEFAULT 0                                  |
| original_path | TEXT     | NULLABLE                                   |
| created_at    | DATETIME | DEFAULT CURRENT_TIMESTAMP                  |
| updated_at    | DATETIME | DEFAULT CURRENT_TIMESTAMP                  |

#### tags

| Column     | Type     | Constraints                  |
| ---------- | -------- | ---------------------------- |
| id         | INTEGER  | PRIMARY KEY AUTOINCREMENT    |
| name       | TEXT     | NOT NULL, UNIQUE             |
| color      | TEXT     | NULLABLE (hex color #RRGGBB) |
| created_at | DATETIME | DEFAULT CURRENT_TIMESTAMP    |

#### authors

| Column     | Type     | Constraints               |
| ---------- | -------- | ------------------------- |
| id         | INTEGER  | PRIMARY KEY AUTOINCREMENT |
| name       | TEXT     | NOT NULL, UNIQUE          |
| created_at | DATETIME | DEFAULT CURRENT_TIMESTAMP |

#### metadata

| Column    | Type    | Constraints                                    |
| --------- | ------- | ---------------------------------------------- |
| id        | INTEGER | PRIMARY KEY AUTOINCREMENT                      |
| file_id   | INTEGER | FK → files(id) ON DELETE CASCADE               |
| key       | TEXT    | NOT NULL                                       |
| value     | TEXT    | NOT NULL                                       |
| data_type | TEXT    | CHECK IN ('text', 'number', 'date', 'boolean') |
|           |         | UNIQUE(file_id, key)                           |

### Junction Tables

#### file_tags (Many-to-Many: files ↔ tags)

| Column  | Type    | Constraints                      |
| ------- | ------- | -------------------------------- |
| file_id | INTEGER | FK → files(id) ON DELETE CASCADE |
| tag_id  | INTEGER | FK → tags(id) ON DELETE CASCADE  |
|         |         | PRIMARY KEY (file_id, tag_id)    |

#### file_authors (Many-to-Many: files ↔ authors)

| Column    | Type    | Constraints                        |
| --------- | ------- | ---------------------------------- |
| file_id   | INTEGER | FK → files(id) ON DELETE CASCADE   |
| author_id | INTEGER | FK → authors(id) ON DELETE CASCADE |
|           |         | PRIMARY KEY (file_id, author_id)   |

### Special Tables

#### covers (1:1 with files)

| Column     | Type     | Constraints                          |
| ---------- | -------- | ------------------------------------ |
| file_id    | INTEGER  | PK, FK → files(id) ON DELETE CASCADE |
| data       | BLOB     | NOT NULL                             |
| mime_type  | TEXT     | DEFAULT 'image/png'                  |
| created_at | DATETIME | DEFAULT CURRENT_TIMESTAMP            |

#### app_settings (Key-Value Store)

| Column | Type | Constraints |
| ------ | ---- | ----------- |
| key    | TEXT | PRIMARY KEY |
| value  | TEXT | NOT NULL    |

**Known Settings:**

- `storage_path`: Base directory for organized files

### Full-Text Search

#### files_fts (Virtual Table - FTS5)

```sql
CREATE VIRTUAL TABLE files_fts USING fts5(
    display_name,
    path,
    content='files',
    content_rowid='id'
);
```

Automatically synced via triggers on INSERT/UPDATE/DELETE of files.

### Indexes

- `idx_files_category` ON files(category_id)
- `idx_files_status` ON files(file_status)
- `idx_files_name` ON files(display_name)
- `idx_file_tags_tag` ON file_tags(tag_id)
- `idx_metadata_file` ON metadata(file_id)
- `idx_metadata_key` ON metadata(key)
- `idx_file_authors_author` ON file_authors(author_id)

---

## TypeScript Type Definitions

### Core Types (src/types/index.ts)

```typescript
// Enums
type FileStatus = 'available' | 'missing' | 'moved';
type MetadataType = 'text' | 'number' | 'date' | 'boolean';
type FieldType = 'text' | 'authors' | 'number' | 'date' | 'boolean' | 'select' | 'tags' | 'image';

// Entities
interface Category {
  id: number;
  name: string;
  icon: string | null;
  is_default: boolean;
  folder_name: string | null;
  created_at: string;
}

interface FileEntry {
  id: number;
  path: string;
  display_name: string;
  category_id: number | null;
  file_status: FileStatus;
  in_storage: boolean;
  original_path: string | null;
  created_at: string;
  updated_at: string;
  // Optional relations
  category?: Category | null;
  tags?: Tag[];
  authors?: Author[];
  metadata?: Metadata[];
}

interface Tag {
  id: number;
  name: string;
  color: string | null;
  created_at: string;
}

interface Author {
  id: number;
  name: string;
  created_at: string;
}

interface Metadata {
  id: number;
  file_id: number;
  key: string;
  value: string;
  data_type: MetadataType;
}

// Composite Types
interface FileWithDetails extends FileEntry {
  category: Category | null;
  tags: Tag[];
  authors: Author[];
  metadata: Metadata[];
}

// Form Configuration
interface FieldConfig {
  key: string;
  label: string;
  type: FieldType;
  required?: boolean;
  placeholder?: string;
  options?: string[];
  defaultValue?: string;
}
```

### Request/Response Types

```typescript
// File Operations
interface FileListRequest {
  category_id?: number | null;
  tag_ids?: number[];
  status?: FileStatus;
  limit?: number;
  offset?: number;
}

interface FileListResponse {
  files: FileEntry[];
  total: number;
}

interface FileCreateRequest {
  path: string;
  display_name: string;
  category_id?: number | null;
  tag_ids?: number[];
  author_ids?: number[];
  metadata?: Array<{ key: string; value: string; data_type: MetadataType }>;
}

interface FileSearchRequest {
  query: string;
  category_id?: number | null;
  tag_ids?: number[];
  metadata_filters?: Array<{ key: string; value: string }>;
  limit?: number;
  offset?: number;
}

// Category Operations
interface CategoryCreateRequest {
  name: string;
  icon?: string | null;
}

// Tag Operations
interface TagCreateRequest {
  name: string;
  color?: string | null;
}

// Author Operations
interface AuthorCreateRequest {
  name: string;
}
```

---

## API Contracts (Tauri Commands)

### File Commands

| Command              | Parameters                                                         | Returns                             | Description                                      |
| -------------------- | ------------------------------------------------------------------ | ----------------------------------- | ------------------------------------------------ |
| `file_list`          | category_id?, tag_ids?, status?, limit?, offset?                   | FileListResponse                    | List files with pagination                       |
| `file_get`           | id: i64                                                            | FileWithDetails                     | Get single file with all relations               |
| `file_create`        | path, display_name, category_id?, tag_ids?, author_ids?, metadata? | { id: i64 }                         | Create file (moves to storage)                   |
| `file_update`        | id, display_name?                                                  | { success: bool }                   | Update file display name                         |
| `file_delete`        | id                                                                 | { success: bool }                   | Delete file (removes from storage if in_storage) |
| `file_move_category` | id, category_id?                                                   | { success: bool, new_path: string } | Move file to different category folder           |
| `file_search`        | query, category_id?, tag_ids?, metadata_filters?, limit?, offset?  | FileListResponse                    | Full-text search                                 |
| `file_check_status`  | file_ids?                                                          | { updated: [{ id, status }] }       | Check/update file existence status               |

### Category Commands

| Command           | Parameters       | Returns           | Description                                        |
| ----------------- | ---------------- | ----------------- | -------------------------------------------------- |
| `category_list`   | -                | Vec<Category>     | List all categories                                |
| `category_get`    | id: i64          | Category          | Get single category                                |
| `category_create` | name, icon?      | { id: i64 }       | Create category                                    |
| `category_update` | id, name?, icon? | { success: bool } | Update category (renames folder)                   |
| `category_delete` | id               | { success: bool } | Delete category (fails if has files or is_default) |

### Tag Commands

| Command        | Parameters         | Returns                                | Description                         |
| -------------- | ------------------ | -------------------------------------- | ----------------------------------- |
| `tag_list`     | include_usage?     | { tags: TagWithUsage[] }               | List tags with optional usage count |
| `tag_create`   | name, color?       | { id: i64 }                            | Create tag                          |
| `tag_update`   | id, name?, color?  | { success: bool }                      | Update tag                          |
| `tag_delete`   | id                 | { success: bool, affected_files: i64 } | Delete tag                          |
| `tag_assign`   | file_id, tag_ids[] | { success: bool }                      | Assign tags to file                 |
| `tag_unassign` | file_id, tag_ids[] | { success: bool }                      | Remove tags from file               |

### Author Commands

| Command           | Parameters            | Returns                                | Description                            |
| ----------------- | --------------------- | -------------------------------------- | -------------------------------------- |
| `author_list`     | include_usage?        | { authors: AuthorWithUsage[] }         | List authors with optional usage count |
| `author_create`   | name                  | { id: i64 }                            | Create author                          |
| `author_update`   | id, name              | { success: bool }                      | Update author                          |
| `author_delete`   | id                    | { success: bool, affected_files: i64 } | Delete author                          |
| `author_assign`   | file_id, author_ids[] | { success: bool }                      | Add authors to file                    |
| `author_unassign` | file_id, author_ids[] | { success: bool }                      | Remove authors from file               |
| `author_set`      | file_id, author_ids[] | { success: bool }                      | Replace all authors for file           |

### Metadata Commands

| Command           | Parameters                      | Returns                  | Description                  |
| ----------------- | ------------------------------- | ------------------------ | ---------------------------- |
| `metadata_get`    | file_id                         | { metadata: Metadata[] } | Get all metadata for file    |
| `metadata_set`    | file_id, key, value, data_type? | { id: i64 }              | Set/update metadata (upsert) |
| `metadata_delete` | file_id, key                    | { success: bool }        | Delete metadata entry        |

### Cover Commands

| Command        | Parameters                         | Returns                              | Description     |
| -------------- | ---------------------------------- | ------------------------------------ | --------------- |
| `cover_set`    | file_id, data: Vec<u8>, mime_type? | { success: bool }                    | Set cover image |
| `cover_get`    | file_id                            | { data: string (base64), mime_type } | Get cover image |
| `cover_delete` | file_id                            | { success: bool }                    | Delete cover    |

### Settings Commands

| Command                | Parameters | Returns        | Description                          |
| ---------------------- | ---------- | -------------- | ------------------------------------ |
| `settings_get`         | key        | Option<String> | Get setting value                    |
| `settings_set`         | key, value | ()             | Set setting (validates storage_path) |
| `storage_get_path`     | -          | Option<String> | Get storage path                     |
| `storage_check_access` | -          | bool           | Check if storage path is accessible  |

---

## Validation Rules

### Names

- **Category name**: 1-50 chars, no control chars
- **Tag name**: 1-50 chars, letters/numbers/spaces/hyphens/underscores only
- **Author name**: 1-100 chars, no control chars
- **Display name**: 1-255 chars, no control chars

### Metadata

- **Key**: 1-100 chars, letters/numbers/underscores only
- **Value**: Max 10,000 chars

### Colors

- Must be hex format: `#RRGGBB`

### Folder Names

- Invalid chars replaced with `_`: `/ \ : * ? " < > |`
- Trimmed of whitespace and dots
- Max 200 chars
- Lowercased for cross-platform consistency

---

## Error Codes

| Code                            | Description                                |
| ------------------------------- | ------------------------------------------ |
| `FILE_ALREADY_EXISTS`           | File with same path already exists         |
| `STORAGE_PATH_NOT_CONFIGURED`   | Storage path not set                       |
| `STORAGE_PATH_NOT_FOUND`        | Storage path doesn't exist                 |
| `SOURCE_FILE_NOT_FOUND`         | Source file doesn't exist                  |
| `SOURCE_ALREADY_IN_STORAGE`     | Source is inside storage path              |
| `CATEGORY_NOT_FOUND`            | Category doesn't exist                     |
| `FILE_NOT_FOUND`                | File doesn't exist                         |
| `FILE_NOT_IN_STORAGE`           | File not in storage (can't move)           |
| `CATEGORY_EXISTS`               | Category name already exists               |
| `CANNOT_DELETE_DEFAULT`         | Can't delete default category              |
| `CATEGORY_HAS_FILES`            | Category has files (can't delete)          |
| `TAG_EXISTS`                    | Tag name already exists                    |
| `AUTHOR_EXISTS`                 | Author name already exists                 |
| `COVER_NOT_FOUND`               | No cover for file                          |
| `STORAGE_PATH_CHANGE_BLOCKED`   | Can't change storage with files in storage |
| `STORAGE_PATH_SYSTEM_DIRECTORY` | Can't use system directory                 |
| `STORAGE_PATH_NOT_WRITABLE`     | No write permission                        |
| `PERMISSION_DENIED`             | Filesystem permission error                |
| `DISK_FULL`                     | No disk space                              |
| `FILE_LOCKED`                   | File in use                                |

---

## Default Seed Data

Categories (all with `is_default: true`):

1. Novel (icon: "book")
2. Comic (icon: "panel-top")
3. Game (icon: "gamepad-2")
4. Anime (icon: "tv")
5. Other (icon: "folder")

---

## Key Architectural Notes

1. **File Storage Model**: Files are physically moved to `{storage_path}/{category_folder_name}/` when added. The `original_path` preserves the source location.

2. **Uncategorized Files**: Go to `{storage_path}/_uncategorized/`

3. **Category Folders**: Auto-generated from category name (sanitized), stored in `folder_name` column.

4. **Cascade Deletion**:
   - Deleting a file cascades to: file_tags, file_authors, metadata, covers
   - Deleting a category SET NULL on files.category_id
   - Deleting a tag/author cascades through junction tables

5. **Full-Text Search**: Uses SQLite FTS5 on display_name and path columns.

6. **Cover Images**: Stored as BLOB in database, returned as base64.

7. **Settings Protection**: Storage path can't be changed while files exist in storage.
