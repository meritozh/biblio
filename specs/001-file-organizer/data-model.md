# Data Model: File Organizer

**Feature**: 001-file-organizer
**Date**: 2026-02-27

## Entity Relationship Diagram

```
┌─────────────┐       ┌─────────────────┐       ┌─────────────┐
│  Category   │       │     File        │       │     Tag     │
├─────────────┤       ├─────────────────┤       ├─────────────┤
│ id          │◄──────│ category_id     │       │ id          │
│ name        │       │ id              │       │ name        │
│ icon        │       │ path            │       │ color       │
│ created_at  │       │ display_name    │       │ created_at  │
└─────────────┘       │ created_at      │       └─────────────┘
                      │ updated_at      │              │
                      │ file_status     │              │
                      └─────────────────┘              │
                              │                       │
                              │       ┌───────────────┘
                              │       │
                              ▼       ▼
                      ┌─────────────────┐
                      │   FileTag       │
                      ├─────────────────┤
                      │ file_id         │
                      │ tag_id          │
                      └─────────────────┘
                              │
                              │
                              ▼
                      ┌─────────────────┐
                      │   Metadata      │
                      ├─────────────────┤
                      │ id              │
                      │ file_id         │
                      │ key             │
                      │ value           │
                      │ data_type       │
                      └─────────────────┘
```

## Entities

### Category

Classification type for grouping files. Predefined categories: novel, comic, game, anime, other.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | INTEGER | PRIMARY KEY, AUTOINCREMENT | Unique identifier |
| name | TEXT | NOT NULL, UNIQUE | Display name (e.g., "Novel", "Comic") |
| icon | TEXT | NULLABLE | Icon identifier or emoji |
| is_default | BOOLEAN | DEFAULT 0 | System default category (cannot be deleted) |
| created_at | DATETIME | DEFAULT CURRENT_TIMESTAMP | Creation timestamp |

**Validation Rules**:
- name: 1-50 characters, no leading/trailing whitespace
- icon: Optional, max 10 characters (emoji or icon name)

**State Transitions**: None (CRUD only)

### File

Represents a file reference in the library. Stores metadata without duplicating file content.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | INTEGER | PRIMARY KEY, AUTOINCREMENT | Unique identifier |
| path | TEXT | NOT NULL, UNIQUE | Absolute file path |
| display_name | TEXT | NOT NULL | User-visible name |
| category_id | INTEGER | FOREIGN KEY → Category(id), NULLABLE | Assigned category |
| file_status | TEXT | DEFAULT 'available' | File availability status |
| created_at | DATETIME | DEFAULT CURRENT_TIMESTAMP | When added to library |
| updated_at | DATETIME | DEFAULT CURRENT_TIMESTAMP | Last modification time |

**File Status Values**:
- `available`: File exists on disk
- `missing`: File not found at path
- `moved`: File path changed (detected)

**Validation Rules**:
- path: Must be absolute path, max 4096 characters
- display_name: 1-255 characters
- category_id: Must reference existing category or be NULL

**State Transitions**:
```
available ──(file deleted on disk)──► missing
missing ──(file restored)──► available
available ──(file moved)──► moved
moved ──(path updated)──► available
```

### Tag

User-defined label for flexible organization.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | INTEGER | PRIMARY KEY, AUTOINCREMENT | Unique identifier |
| name | TEXT | NOT NULL, UNIQUE | Tag name |
| color | TEXT | NULLABLE | Hex color code (e.g., "#FF5733") |
| created_at | DATETIME | DEFAULT CURRENT_TIMESTAMP | Creation timestamp |

**Validation Rules**:
- name: 1-50 characters, alphanumeric and spaces only
- color: Valid hex color format (#RRGGBB) or NULL

**State Transitions**: None (CRUD only)

### FileTag (Junction Table)

Many-to-many relationship between files and tags.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| file_id | INTEGER | NOT NULL, FOREIGN KEY → File(id) | File reference |
| tag_id | INTEGER | NOT NULL, FOREIGN KEY → Tag(id) | Tag reference |

**Constraints**: PRIMARY KEY (file_id, tag_id)

### Metadata

Custom key-value pairs attached to files.

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| id | INTEGER | PRIMARY KEY, AUTOINCREMENT | Unique identifier |
| file_id | INTEGER | NOT NULL, FOREIGN KEY → File(id), ON DELETE CASCADE | File reference |
| key | TEXT | NOT NULL | Metadata field name |
| value | TEXT | NOT NULL | Metadata value |
| data_type | TEXT | DEFAULT 'text' | Value type |

**Data Type Values**:
- `text`: String value
- `number`: Numeric value (stored as text, parsed on read)
- `date`: ISO 8601 date string
- `boolean`: "true" or "false"

**Validation Rules**:
- key: 1-100 characters, alphanumeric and underscores only
- value: Max 10,000 characters
- Unique constraint: (file_id, key)

## Indexes

```sql
-- Performance indexes
CREATE INDEX idx_files_category ON files(category_id);
CREATE INDEX idx_files_status ON files(file_status);
CREATE INDEX idx_files_name ON files(display_name);
CREATE INDEX idx_file_tags_tag ON file_tags(tag_id);
CREATE INDEX idx_metadata_file ON metadata(file_id);
CREATE INDEX idx_metadata_key ON metadata(key);

-- Full-text search
CREATE VIRTUAL TABLE files_fts USING fts5(
    display_name,
    path,
    content='files',
    content_rowid='id'
);
```

## Queries

### Common Query Patterns

**Get files by category with tags**:
```sql
SELECT f.*, GROUP_CONCAT(t.name) as tags
FROM files f
LEFT JOIN file_tags ft ON f.id = ft.file_id
LEFT JOIN tags t ON ft.tag_id = t.id
WHERE f.category_id = ?
GROUP BY f.id;
```

**Search files by name/tags/metadata**:
```sql
SELECT DISTINCT f.*
FROM files f
LEFT JOIN file_tags ft ON f.id = ft.file_id
LEFT JOIN tags t ON ft.tag_id = t.id
LEFT JOIN metadata m ON f.id = m.file_id
WHERE f.display_name LIKE ?
   OR t.name LIKE ?
   OR m.value LIKE ?;
```

**Get file with all metadata**:
```sql
SELECT f.*,
       c.name as category_name,
       GROUP_CONCAT(DISTINCT t.name) as tags,
       GROUP_CONCAT(DISTINCT m.key || ':' || m.value) as metadata
FROM files f
LEFT JOIN categories c ON f.category_id = c.id
LEFT JOIN file_tags ft ON f.id = ft.file_id
LEFT JOIN tags t ON ft.tag_id = t.id
LEFT JOIN metadata m ON f.id = m.file_id
WHERE f.id = ?
GROUP BY f.id;
```

## TypeScript Interfaces

```typescript
export type FileStatus = 'available' | 'missing' | 'moved';
export type MetadataType = 'text' | 'number' | 'date' | 'boolean';

export interface Category {
  id: number;
  name: string;
  icon: string | null;
  isDefault: boolean;
  createdAt: string;
}

export interface FileEntry {
  id: number;
  path: string;
  displayName: string;
  categoryId: number | null;
  fileStatus: FileStatus;
  createdAt: string;
  updatedAt: string;
  category?: Category;
  tags?: Tag[];
  metadata?: Metadata[];
}

export interface Tag {
  id: number;
  name: string;
  color: string | null;
  createdAt: string;
}

export interface Metadata {
  id: number;
  fileId: number;
  key: string;
  value: string;
  dataType: MetadataType;
}

export interface FileWithDetails extends FileEntry {
  category: Category | null;
  tags: Tag[];
  metadata: Metadata[];
}
```