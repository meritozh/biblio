# Biblio Architecture Research - Learnings

## Overview

Biblio is a Tauri 2.x desktop application for organizing local files into category-based folder structures with metadata support. It uses a modern React 19 frontend with TanStack ecosystem and a Rust backend with SQLite.

---

## Frontend Architecture

### Entry Point & Routing

- **Entry**: `src/main.tsx` - Minimal setup using TanStack Router's `RouterProvider`
- **Router**: File-based routing via `@tanstack/react-router` with auto-generated `routeTree.gen.ts`
- **Root Route**: `src/routes/__root.tsx` - Simple wrapper with background styling
- **Main Route**: `src/routes/index.tsx` - Contains the entire application UI (single-page app pattern)

### State Management

Uses **TanStack Store** (`@tanstack/react-store`) with a store-per-domain pattern:

1. **fileStore.ts** - Files list, loading state, selected category, import progress
2. **categoryStore.ts** - Categories list with loading/error state
3. **tagStore.ts** - Tags list with usage counts

**Pattern**: Each store exports:

- A `Store` instance with typed state interface
- Async action functions that update the store (e.g., `fetchFiles`, `fetchCategories`)
- Optional hook wrappers (e.g., `useTagStore`)

### Tauri API Wrapper

`src/lib/tauri.ts` provides a clean TypeScript API layer:

- Wraps all Tauri `invoke()` calls with proper parameter mapping (camelCase → snake_case)
- Exports typed functions for each backend command
- Includes `translateError()` for user-friendly error messages
- **Key insight**: Parameter names are converted (e.g., `category_id` → `categoryId`)

### Types System

`src/types/index.ts` defines all shared types:

- Core entities: `Category`, `FileEntry`, `Tag`, `Author`, `Metadata`
- Request/Response types: `FileListRequest`, `FileCreateRequest`, etc.
- Enums: `FileStatus`, `MetadataType`, `FieldType`

### Component Organization

```
src/components/
├── ui/                    # shadcn/ui primitives (button, dialog, input, etc.)
├── CategorySidebar.tsx    # Resizable sidebar with category navigation
├── FileList.tsx           # TanStack Table-based file listing
├── FileCard.tsx           # Individual file card with status indicator
├── DynamicMetadataForm.tsx # Form with category-specific fields
├── FilePicker.tsx         # Tauri dialog integration for file selection
├── SettingsDialog.tsx     # Storage path configuration
└── [Manager components]   # TagManager, AuthorManager, CategorySelect
```

### Configuration

`src/config/categoryFields.ts` - Category-specific metadata field definitions:

- `DEFAULT_FIELDS` - Shown for all files (authors)
- `CATEGORY_FIELDS` - Category-specific fields (e.g., Novels have progress, Comics have volume)
- `getFieldsForCategory()` - Merges default + category fields

---

## Backend Architecture (Rust/Tauri)

### Entry Point

`src-tauri/src/lib.rs`:

- Registers Tauri plugins: `tauri-plugin-sql`, `tauri-plugin-dialog`, `tauri-plugin-opener`, `tauri-plugin-mcp-bridge`
- Configures SQLite migrations
- Registers all command handlers in `invoke_handler`

### Command Structure

`src-tauri/src/commands/` - One file per domain:

- `file.rs` - File CRUD, move, search, status check
- `category.rs` - Category CRUD with folder management
- `tag.rs` - Tag CRUD and assignment
- `author.rs` - Author CRUD and assignment
- `metadata.rs` - Key-value metadata storage
- `cover.rs` - Cover image BLOB storage
- `settings.rs` - App settings (storage_path)
- `validation.rs` - Input validation helpers

**Pattern**: Each command file:

- Uses `get_sqlite_pool()` helper to access database
- Returns typed structs with `#[derive(Serialize)]`
- Uses `sqlx::query_as` for type-safe queries

### Database Layer

`src-tauri/src/database/`:

- `mod.rs` - Migration registration
- `schema.sql` - Initial schema (v1)
- `migration_2.sql` - Authors and covers tables (v2)
- `seed.rs` - Default data seeding
- `recovery.rs` - Database maintenance commands

### Database Schema

**Tables**:

- `categories` - id, name, icon, is_default, folder_name
- `files` - id, path, display_name, category_id, file_status, in_storage, original_path
- `tags` - id, name, color
- `file_tags` - Junction table (file_id, tag_id)
- `authors` - id, name
- `file_authors` - Junction table (file_id, author_id)
- `metadata` - id, file_id, key, value, data_type
- `covers` - file_id, data (BLOB), mime_type
- `app_settings` - key, value (stores storage_path)

**Full-Text Search**: `files_fts` virtual table using FTS5 with triggers for sync

---

## Data Flow Patterns

### File Import Flow

1. User selects files via `FilePicker` (Tauri dialog)
2. `DynamicMetadataForm` collects metadata
3. `handleAddFile()` in index.tsx calls `fileCreate()`
4. Backend:
   - Validates storage path exists
   - Creates category folder if needed
   - Moves file to `{storage_path}/{category_folder}/`
   - Inserts database record with `in_storage=true`
   - Associates tags, authors, metadata

### Category Change Flow

1. User changes category in `DynamicMetadataForm`
2. If file is in storage, shows confirmation dialog
3. Backend `file_move_category`:
   - Moves file between category folders
   - Updates database path and category_id

### State Synchronization

- Stores are NOT automatically synced with backend
- Components call fetch functions explicitly after mutations
- `useEffect` in index.tsx loads initial data on mount

---

## Design System

### Tailwind 4.x Configuration

`src/index.css` uses `@theme` blocks for design tokens:

- **Colors**: Notion-inspired palette (warm grays, soft accents)
- **Radius**: 0.75rem base with derived sizes
- **Dark mode**: Via `prefers-color-scheme: dark` media query

### UI Patterns

- shadcn/ui components with Radix primitives
- Lucide icons
- Custom scrollbar styling
- Notion-style hover effects and gradients

---

## Key Architectural Decisions

1. **Single Route App**: All UI in `routes/index.tsx` - no multi-page navigation
2. **Store-per-Domain**: Separate stores for files, categories, tags rather than a single global store
3. **Tauri API Wrapper**: Centralized in `lib/tauri.ts` for type safety and error handling
4. **Category-based Folder Structure**: Files physically moved to `{storage}/{category}/`
5. **Migration-based Schema**: Never modify existing migrations, add new ones
6. **FTS5 for Search**: Full-text search on file names and paths

---

## File Naming Conventions

- **Frontend**: camelCase for files, PascalCase for components
- **Backend**: snake_case for Rust files
- **Database**: snake_case for tables and columns
- **API**: snake_case in Rust, camelCase wrapper in TypeScript
