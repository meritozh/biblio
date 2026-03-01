# Tauri IPC Command Contracts

**Feature**: 001-file-organizer
**Date**: 2026-02-27

## Overview

This document defines the Tauri IPC command contracts between the Rust backend and TypeScript frontend. All commands follow the invoke pattern with typed request/response objects.

## Command Naming Convention

- Prefix: domain entity (file, category, tag, metadata)
- Action: get, create, update, delete, search, list
- Format: `{entity}_{action}`

---

## File Commands

### `file_list`

List files with optional filtering.

**Request**:
```typescript
interface FileListRequest {
  categoryId?: number | null;
  tagIds?: number[];
  status?: FileStatus;
  limit?: number;
  offset?: number;
}
```

**Response**:
```typescript
interface FileListResponse {
  files: FileEntry[];
  total: number;
}
```

---

### `file_get`

Get a single file with full details (category, tags, metadata).

**Request**:
```typescript
interface FileGetRequest {
  id: number;
}
```

**Response**:
```typescript
interface FileWithDetails {
  id: number;
  path: string;
  displayName: string;
  categoryId: number | null;
  fileStatus: FileStatus;
  createdAt: string;
  updatedAt: string;
  category: Category | null;
  tags: Tag[];
  metadata: Metadata[];
}
```

---

### `file_create`

Add a new file to the library.

**Request**:
```typescript
interface FileCreateRequest {
  path: string;
  displayName: string;
  categoryId?: number | null;
  tagIds?: number[];
  metadata?: Array<{ key: string; value: string; dataType: MetadataType }>;
}
```

**Response**:
```typescript
interface FileCreateResponse {
  id: number;
}
```

**Errors**:
- `FILE_ALREADY_EXISTS`: Path already in library
- `INVALID_PATH`: Path does not exist or is not accessible

---

### `file_update`

Update file properties.

**Request**:
```typescript
interface FileUpdateRequest {
  id: number;
  displayName?: string;
  categoryId?: number | null;
}
```

**Response**:
```typescript
interface FileUpdateResponse {
  success: boolean;
}
```

---

### `file_delete`

Remove a file from the library (does not delete actual file).

**Request**:
```typescript
interface FileDeleteRequest {
  id: number;
}
```

**Response**:
```typescript
interface FileDeleteResponse {
  success: boolean;
}
```

---

### `file_search`

Full-text search across files.

**Request**:
```typescript
interface FileSearchRequest {
  query: string;
  categoryId?: number | null;
  tagIds?: number[];
  metadataFilters?: Array<{ key: string; value: string }>;
  limit?: number;
  offset?: number;
}
```

**Response**:
```typescript
interface FileSearchResponse {
  files: FileEntry[];
  total: number;
}
```

---

### `file_check_status`

Check availability status of files.

**Request**:
```typescript
interface FileCheckStatusRequest {
  fileIds?: number[]; // If omitted, check all files
}
```

**Response**:
```typescript
interface FileCheckStatusResponse {
  updated: Array<{ id: number; status: FileStatus }>;
}
```

---

## Category Commands

### `category_list`

List all categories.

**Request**: `{}` (no parameters)

**Response**:
```typescript
interface CategoryListResponse {
  categories: Category[];
}
```

---

### `category_get`

Get a single category.

**Request**:
```typescript
interface CategoryGetRequest {
  id: number;
}
```

**Response**: `Category`

---

### `category_create`

Create a new category.

**Request**:
```typescript
interface CategoryCreateRequest {
  name: string;
  icon?: string | null;
}
```

**Response**:
```typescript
interface CategoryCreateResponse {
  id: number;
}
```

**Errors**:
- `CATEGORY_EXISTS`: Category name already exists

---

### `category_update`

Update a category.

**Request**:
```typescript
interface CategoryUpdateRequest {
  id: number;
  name?: string;
  icon?: string | null;
}
```

**Response**:
```typescript
interface CategoryUpdateResponse {
  success: boolean;
}
```

---

### `category_delete`

Delete a category (files become uncategorized).

**Request**:
```typescript
interface CategoryDeleteRequest {
  id: number;
}
```

**Response**:
```typescript
interface CategoryDeleteResponse {
  success: boolean;
  affectedFiles: number;
}
```

**Errors**:
- `CANNOT_DELETE_DEFAULT`: Cannot delete default categories

---

## Tag Commands

### `tag_list`

List all tags with optional usage counts.

**Request**:
```typescript
interface TagListRequest {
  includeUsage?: boolean;
}
```

**Response**:
```typescript
interface TagWithUsage extends Tag {
  usageCount: number;
}

interface TagListResponse {
  tags: TagWithUsage[];
}
```

---

### `tag_create`

Create a new tag.

**Request**:
```typescript
interface TagCreateRequest {
  name: string;
  color?: string | null;
}
```

**Response**:
```typescript
interface TagCreateResponse {
  id: number;
}
```

**Errors**:
- `TAG_EXISTS`: Tag name already exists

---

### `tag_update`

Update a tag.

**Request**:
```typescript
interface TagUpdateRequest {
  id: number;
  name?: string;
  color?: string | null;
}
```

**Response**:
```typescript
interface TagUpdateResponse {
  success: boolean;
}
```

---

### `tag_delete`

Delete a tag (removes from all files).

**Request**:
```typescript
interface TagDeleteRequest {
  id: number;
}
```

**Response**:
```typescript
interface TagDeleteResponse {
  success: boolean;
  affectedFiles: number;
}
```

---

### `tag_assign`

Assign tags to a file.

**Request**:
```typescript
interface TagAssignRequest {
  fileId: number;
  tagIds: number[];
}
```

**Response**:
```typescript
interface TagAssignResponse {
  success: boolean;
}
```

---

### `tag_unassign`

Remove tags from a file.

**Request**:
```typescript
interface TagUnassignRequest {
  fileId: number;
  tagIds: number[];
}
```

**Response**:
```typescript
interface TagUnassignResponse {
  success: boolean;
}
```

---

## Metadata Commands

### `metadata_get`

Get all metadata for a file.

**Request**:
```typescript
interface MetadataGetRequest {
  fileId: number;
}
```

**Response**:
```typescript
interface MetadataGetResponse {
  metadata: Metadata[];
}
```

---

### `metadata_set`

Set a metadata field for a file (creates or updates).

**Request**:
```typescript
interface MetadataSetRequest {
  fileId: number;
  key: string;
  value: string;
  dataType?: MetadataType;
}
```

**Response**:
```typescript
interface MetadataSetResponse {
  id: number;
}
```

---

### `metadata_delete`

Delete a metadata field.

**Request**:
```typescript
interface MetadataDeleteRequest {
  fileId: number;
  key: string;
}
```

**Response**:
```typescript
interface MetadataDeleteResponse {
  success: boolean;
}
```

---

## Error Types

All commands may return errors in this format:

```typescript
interface CommandError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
}
```

### Error Codes

| Code | Description |
|------|-------------|
| `FILE_NOT_FOUND` | File does not exist in library |
| `FILE_ALREADY_EXISTS` | File path already in library |
| `INVALID_PATH` | File path is invalid or inaccessible |
| `CATEGORY_NOT_FOUND` | Category does not exist |
| `CANNOT_DELETE_DEFAULT` | Cannot delete default category |
| `CATEGORY_EXISTS` | Category name already exists |
| `TAG_NOT_FOUND` | Tag does not exist |
| `TAG_EXISTS` | Tag name already exists |
| `DATABASE_ERROR` | SQLite operation failed |
| `VALIDATION_ERROR` | Input validation failed |

---

## Frontend Usage

```typescript
import { invoke } from '@tauri-apps/api/core';

// Example: List files
const { files, total } = await invoke<FileListResponse>('file_list', {
  categoryId: 1,
  limit: 50,
  offset: 0
});

// Example: Create file
const { id } = await invoke<FileCreateResponse>('file_create', {
  path: '/path/to/file.pdf',
  displayName: 'My Novel',
  categoryId: 1,
  tagIds: [1, 2]
});
```