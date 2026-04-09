# biblio Development Guidelines

## What is Biblio

A Tauri 2.x desktop app for organizing local files into category-based folder structures with metadata support.

## Active Technologies

- TypeScript 5.x (frontend), Rust 1.75+ (Tauri backend) + React 19, Tauri 2.x, shadcn/ui, TanStack Router, TanStack Table, TanStack Form, TanStack Store, Tailwind CSS 4.x (001-file-organizer)

## Project Structure

```text
src/
  components/     # React components (ui/ for shadcn primitives)
  routes/         # TanStack Router pages
  lib/            # Utilities, Tauri API wrappers
  types/          # TypeScript type definitions
src-tauri/
  src/
    commands/     # Rust Tauri command handlers
    database/     # SQLite schema
tests/
```

## Commands

pnpm typecheck # TypeScript check
pnpm test:run # Run unit tests
pnpm lint # ESLint
pnpm tauri:dev # Start dev server
pnpm tauri:build # Build production app

## Code Style

TypeScript 5.x (frontend), Rust 1.75+ (Tauri backend): Follow standard conventions

**Design System**: Colors defined in `src/index.css` using Tailwind 4.x `@theme` blocks. Primary palette: ivory background (#FFFDF5), espresso brown primary (#3D3629), bronze accent (#8B7355). Typography: Cormorant Garamond (headings), Nunito Sans (body) loaded via Google Fonts in `index.html`.

**UI Components**: shadcn/ui primitives in `src/components/ui/`. Use rounded-xl borders, subtle shadows, smooth transitions (duration-200). Focus states use ring utilities.

## Gotchas

- **Database Schema**: Project is in active development. Update `schema.sql` directly when schema changes. Do NOT create migration files. After modifying schema, delete the dev database to reset (`~/Library/Application Support/com.biblio.app/biblio.db`).

- **Database Location**: Dev database is at `~/Library/Application Support/io.augite.biblio/biblio.db`. Delete to reset.

- **Storage System**: Files are moved to `{storage_path}/{category}/` folders. Check storage path state when dialogs close.

- **React useEffect**: Avoid circular dependencies between effects. Don't sync internal state with props in one effect and call `onChange` in another - this causes infinite loops. Derive values from props directly.

- **New Tauri Commands**: Must be (1) created in `src-tauri/src/commands/`, (2) exported in `mod.rs`, (3) registered in `lib.rs` invoke_handler.

- **Tailwind 4.x**: Uses `@plugin 'tailwindcss-animate'` syntax in CSS, not postcss.config.js.

## Agent Principles

- **Think before acting**: Don't mechanically execute user input. Follow the loop: think → explore → plan → execute → validate. Understand the problem space before writing code.
- **Ask when uncertain**: If anything is unclear or ambiguous, ask the user for clarification rather than guessing.
