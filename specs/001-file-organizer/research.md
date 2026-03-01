# Research: File Organizer

**Feature**: 001-file-organizer
**Date**: 2026-02-27

## Technology Stack Decisions

### Frontend Framework: React 19

**Decision**: React 19 with TypeScript
**Rationale**: 
- Latest React version with improved concurrent features
- Excellent TypeScript support
- Large ecosystem and community
- User-specified requirement
**Alternatives Considered**: Vue 3, Svelte, SolidJS

### Desktop Framework: Tauri 2.x

**Decision**: Tauri 2.x
**Rationale**:
- Cross-platform (macOS, Windows, Linux)
- Smaller bundle size than Electron (uses system WebView)
- Rust backend for performance and safety
- Native SQLite support via tauri-plugin-sql
- User-specified requirement
**Alternatives Considered**: Electron, Neutralino, Flutter

### UI Component Library: shadcn/ui

**Decision**: shadcn/ui
**Rationale**:
- Copy-paste components, no npm dependency bloat
- Built on Radix UI primitives (accessible by default)
- Tailwind CSS styling
- Fully customizable
- User-specified requirement
**Alternatives Considered**: MUI, Chakra UI, Ant Design

### Routing: TanStack Router

**Decision**: TanStack Router v1
**Rationale**:
- Type-safe routing with TypeScript
- Built-in data loading and caching
- Search param state management
- Code splitting support
- User-specified requirement
**Alternatives Considered**: React Router, Wouter

### Table/Data Grid: TanStack Table

**Decision**: TanStack Table v8
**Rationale**:
- Headless UI, maximum flexibility
- Virtualization support for large datasets
- Sorting, filtering, pagination built-in
- TypeScript-first
- User-specified requirement
**Alternatives Considered**: AG Grid, React Data Grid, MUI DataGrid

### Form Management: TanStack Form

**Decision**: TanStack Form
**Rationale**:
- Type-safe form handling
- Integrates with TanStack ecosystem
- Validation agnostic (works with Zod, Yup, etc.)
- User-specified requirement
**Alternatives Considered**: React Hook Form, Formik

### State Management: TanStack Store

**Decision**: TanStack Store
**Rationale**:
- Atomic state management
- TypeScript-first
- Integrates with TanStack ecosystem
- User-specified requirement
**Alternatives Considered**: Zustand, Jotai, Redux

### Styling: Tailwind CSS 4.x

**Decision**: Tailwind CSS 4.x
**Rationale**:
- Utility-first CSS
- Excellent DX with JIT compilation
- Works seamlessly with shadcn/ui
- User-specified requirement
**Alternatives Considered**: CSS Modules, Styled Components

### Database: SQLite (embedded)

**Decision**: SQLite via tauri-plugin-sql
**Rationale**:
- Embedded, no external process
- ACID compliant
- Excellent performance for local data
- User-specified requirement
- Full-text search via FTS5 extension
**Alternatives Considered**: IndexedDB, PouchDB, custom file format

## Best Practices

### Tauri Security

- Use Tauri's allowlist to restrict IPC commands
- Validate all inputs in Rust backend
- Use content security policy for WebView
- Store sensitive data in OS keychain if needed

### React Performance

- Use React.memo for expensive components
- Implement virtualization for large file lists
- Lazy load routes and heavy components
- Use TanStack Table's virtualization for 10k+ rows

### SQLite Optimization

- Create indexes on frequently queried columns (category_id, tags)
- Use FTS5 for full-text search
- Implement connection pooling
- Use prepared statements for repeated queries
- Vacuum database periodically

### TypeScript Configuration

```json
{
  "compilerOptions": {
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "exactOptionalPropertyTypes": true
  }
}
```

### Testing Strategy

- **Unit**: Vitest for isolated component/function tests
- **Integration**: Test Tauri IPC commands with mocked backend
- **E2E**: Playwright for full user flows

## Dependency Versions (Latest as of 2026-02-27)

| Package | Version |
|---------|---------|
| react | ^19.0.0 |
| react-dom | ^19.0.0 |
| @tauri-apps/api | ^2.0.0 |
| @tauri-apps/cli | ^2.0.0 |
| @tanstack/react-router | ^1.x |
| @tanstack/react-table | ^8.x |
| @tanstack/react-form | ^0.x |
| @tanstack/react-store | ^0.x |
| tailwindcss | ^4.0.0 |
| typescript | ^5.x |
| vite | ^6.x |
| vitest | ^3.x |
| @playwright/test | ^1.x |

## Integration Patterns

### Tauri IPC Pattern

```typescript
// Frontend
import { invoke } from '@tauri-apps/api/core';
const files = await invoke<FileEntry[]>('get_files', { categoryId: 1 });

// Backend (Rust)
#[tauri::command]
fn get_files(category_id: Option<i64>) -> Result<Vec<FileEntry>, String> {
    // Database query and return
}
```

### SQLite Schema Pattern

```sql
CREATE TABLE files (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    category_id INTEGER REFERENCES categories(id),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_files_category ON files(category_id);
CREATE INDEX idx_files_name ON files(display_name);
```

### Search Implementation

Use SQLite FTS5 for full-text search across file names, tags, and metadata:

```sql
CREATE VIRTUAL TABLE files_fts USING fts5(
    display_name,
    metadata_content,
    content='files',
    content_rowid='id'
);
```