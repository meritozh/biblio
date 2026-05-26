# Biblio

A desktop app for organizing a local library of novels and comics into
category-based folders with searchable metadata, tags, and authors. Built
with Tauri 2 (Rust backend, React frontend).

Biblio moves imported files into a `{storage_path}/{category}/` layout,
records metadata in a local SQLite database, and can optionally use an
LLM to extract titles, authors, and tags at import time. It also supports
syncing files to and from Baidu Netdisk.

## Features

- **Library** — grid view of files with cover thumbnails, full-text
  search (trigram FTS, works mid-word and on CJK), and a composable
  filter editor (by tag, author, progress, status, storage, name length).
- **Category-driven schemas** — `novel` (text) and `comic` (archives /
  image folders) each drive their own import pipeline, form layout, and
  card layout. Categories own their default view config.
- **Tags & authors** — paginated, searchable pickers; bulk and per-file
  assignment.
- **LLM-assisted import** (optional) — filename and content analysis via
  any OpenAI-compatible endpoint (e.g. LM Studio, Ollama). Prompts are
  editable in-app per schema and step.
- **Comic handling** — reads `.cbz` / `.zip` / `.cbr` / `.rar` and image
  folders, picks a cover from the archive, and can regenerate missing
  covers.
- **Remote storage** (optional) — upload, download, and delete against
  Baidu Netdisk, with files stored under paths relative to a configurable
  root.
- **Cleanup tools** — re-analyze novels missing tags, bulk-assign an
  author to authorless files, and regenerate missing comic covers.

## Tech stack

- **Frontend**: React 19, TypeScript 5, TanStack Router / Table / Form /
  Store / Virtual, shadcn/ui (Radix primitives), Tailwind CSS 4, Vite.
- **Backend**: Rust, Tauri 2, `tauri-plugin-sql` (sqlx + SQLite), `rig`
  for LLM calls.
- **Tests**: Vitest (unit), Playwright (e2e), `cargo test` (Rust).

## Project structure

```text
src/
  components/   React components (ui/ holds shadcn primitives)
  routes/       TanStack Router pages (library, categories, tags, authors,
                cleanup, prompts)
  lib/          utilities + Tauri API wrappers
  stores/       TanStack Store state
  types/        TypeScript type definitions
src-tauri/
  src/
    commands/   Tauri command handlers
    database/   schema.sql + migrations (mod.rs)
    pipeline/   import pipeline (sampling, decoding, LLM nodes)
    providers/  remote storage (baidu_netdisk)
    services/   background workers (import, upload, download, delete)
```

## Development

Prerequisites: Node.js with [pnpm](https://pnpm.io/), Rust 1.75+, and the
[Tauri 2 system dependencies](https://tauri.app/start/prerequisites/).

```bash
pnpm install
pnpm tauri:dev        # run the app in dev mode
```

Other useful commands:

```bash
pnpm typecheck        # tsc --noEmit
pnpm lint             # ESLint (zero-warning gate)
pnpm test:run         # Vitest unit tests
pnpm test:e2e         # Playwright e2e
pnpm tauri:build      # production bundle
```

Rust-side checks run from `src-tauri/`:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test  --manifest-path src-tauri/Cargo.toml
```

## Configuration

Both the LLM and remote storage are optional and configured inside the
app:

- **LLM** — point it at an OpenAI-compatible base URL and model. With LLM
  disabled, import still works; it just skips the metadata extraction
  steps.
- **Baidu Netdisk** — authorize the app to enable remote upload/download.

The dev database lives at
`~/Library/Application Support/io.augite.biblio/biblio.db`.

## Database

The schema is migration-driven through `tauri-plugin-sql`. `schema.sql` is
the consolidated v1 baseline; evolve the schema by adding a new numbered
entry in `src-tauri/src/database/mod.rs::get_migrations()` rather than
editing `schema.sql` (editing it changes v1's checksum and breaks existing
databases).
