# Quickstart: File Organizer Development

**Feature**: 001-file-organizer
**Date**: 2026-02-27

## Prerequisites

- **Node.js**: 20.x or later
- **pnpm**: 9.x or later (recommended) or npm/yarn
- **Rust**: 1.75 or later
- **Platform**: macOS 10.15+ or Windows 10+

## Initial Setup

### 1. Install Rust

```bash
# macOS/Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows: Download from https://rustup.rs
```

### 2. Install pnpm

```bash
npm install -g pnpm
```

### 3. Create Tauri Project

```bash
pnpm create tauri-app@latest biblio --template react-ts
cd biblio
```

### 4. Install Dependencies

```bash
# Core dependencies
pnpm add react react-dom
pnpm add @tanstack/react-router @tanstack/react-table @tanstack/react-form @tanstack/react-store

# UI dependencies
pnpm add tailwindcss @tailwindcss/vite
pnpm add class-variance-authority clsx tailwind-merge
pnpm add lucide-react

# Tauri plugins
pnpm add @tauri-apps/plugin-sql

# Development dependencies
pnpm add -D typescript @types/react @types/react-dom
pnpm add -D vitest @testing-library/react @testing-library/jest-dom
pnpm add -D @playwright/test
pnpm add -D eslint prettier eslint-config-prettier
```

### 5. Setup shadcn/ui

```bash
npx shadcn@latest init
npx shadcn@latest add button input dialog table tabs
```

### 6. Configure Tailwind CSS 4.x

```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
});
```

### 7. Add Tauri SQL Plugin

```bash
cd src-tauri
cargo add tauri-plugin-sql --features sqlite
```

```rust
// src-tauri/src/lib.rs
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_sql::Builder::default().build())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## Project Commands

### Development

```bash
# Start dev server with hot reload
pnpm tauri dev

# Run type checking
pnpm tsc --noEmit

# Run linting
pnpm lint

# Run formatting
pnpm format
```

### Testing

```bash
# Run unit tests
pnpm test

# Run unit tests in watch mode
pnpm test:watch

# Run E2E tests
pnpm test:e2e

# Run all tests
pnpm test:all
```

### Build

```bash
# Build for production
pnpm tauri build

# Build for specific platform
pnpm tauri build --target universal-apple-darwin  # macOS universal
pnpm tauri build --target x86_64-pc-windows-msvc   # Windows
```

## Key Files

| File | Purpose |
|------|---------|
| `src/main.tsx` | Application entry point |
| `src/routeTree.gen.ts` | Generated routes (TanStack Router) |
| `src/stores/*.ts` | State atoms (TanStack Store) |
| `src/components/*.tsx` | React components |
| `src/lib/tauri.ts` | Tauri IPC wrappers |
| `src-tauri/src/commands/*.rs` | Rust command handlers |
| `src-tauri/src/database/*.rs` | SQLite operations |

## Development Workflow

### 1. Create a New Feature

```bash
# Create feature branch
git checkout -b 001-file-organizer
```

### 2. Write Tests First (TDD)

```typescript
// tests/unit/FileStore.test.ts
import { describe, it, expect } from 'vitest';
import { useFileStore } from '@/stores/fileStore';

describe('FileStore', () => {
  it('should add a file', () => {
    const { addFile } = useFileStore.getState();
    addFile({ path: '/test/file.pdf', displayName: 'Test' });
    // Assert...
  });
});
```

### 3. Implement Feature

```typescript
// src/stores/fileStore.ts
import { atom, useStore } from '@tanstack/react-store';

const fileStore = atom({
  files: [],
});

export const useFileStore = () => useStore(fileStore);
```

### 4. Run Tests

```bash
pnpm test
```

### 5. Commit

```bash
git add .
git commit -m "feat: implement file store with TDD"
```

## Database Migrations

```bash
# Create migration
cd src-tauri
# Migrations are embedded in the app
```

## Debugging

### Frontend DevTools

- React DevTools: Install browser extension
- TanStack DevTools: Built-in for Router and Table

### Tauri Debugging

```bash
# Enable dev tools in release build
pnpm tauri build --debug
```

### Database Inspection

```bash
# Open SQLite database
sqlite3 ~/.local/share/com.biblio/app/database.sqlite
```

## Common Issues

### Rust Compilation Errors

```bash
# Update Rust
rustup update

# Clean build
cd src-tauri && cargo clean
```

### Tailwind Not Applying

```bash
# Ensure content paths are correct
# tailwind.config.ts
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
};
```

### Tauri Commands Not Found

1. Ensure command is registered in `src-tauri/src/lib.rs`
2. Check `tauri.conf.json` security allowlist
3. Rebuild with `pnpm tauri dev`