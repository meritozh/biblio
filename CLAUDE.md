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

**Design System**: Colors defined in `src/index.css` using Tailwind 4.x `@theme` blocks. Primary palette: ivory background (#FFFDF5), espresso brown primary (#3D3629), bronze accent (#8B7355). Typography: Cormorant Garamond (headings), Nunito Sans (body) self-hosted via `@fontsource/*` packages and imported at the top of `src/index.css`. Production CSP keeps `font-src 'self'` only — to add a new weight, add an `@import '@fontsource/<family>/<weight>.css'` line alongside the existing ones.

**UI Components**: shadcn/ui primitives in `src/components/ui/`. Use rounded-xl borders, subtle shadows, smooth transitions (duration-200). Focus states use ring utilities.

## Gotchas

- **Database Schema**: Do not edit `schema.sql` directly. Add a new migration entry in `src-tauri/src/database/mod.rs::get_migrations()` with the next `version` number and the SQL in the `sql` field. `tauri_plugin_sql` runs pending migrations on startup.

- **Database Location**: Dev database is at `~/Library/Application Support/io.augite.biblio/biblio.db`. Delete only as a last-resort dev reset — normally rely on migrations to evolve the schema.

- **Storage System**: Files are moved to `{storage_path}/{category}/` folders. Check storage path state when dialogs close.

- **Remote Encryption**: Every file uploaded to remote storage is encrypted client-side via `services::container` — go through `upload_worker::wrap_and_upload`, never call `upload_to_remote` with a raw file. Remote objects use opaque, extension-less names; the real filename lives only in the DB. The `files.remote_container` column is the authority for the download path — `'bbx1'` (single encrypted object), `'bbx1-split'` (multi-part, below), or `NULL` (legacy raw). Back-filling legacy raw files goes through `reencrypt_worker`. The encryption key lives in `app_settings`; losing it makes remote files unrecoverable.

- **Remote Multi-Part (`'bbx1-split'`)**: a file larger than `upload_worker::PART_SIZE` (**10 GiB** plaintext, to stay under Baidu's 20 GB per-object cap) is uploaded as N independent encrypted parts (`container::wrap_range_with_progress`), each its own remote object, mapped one-logical-file → ordered parts in the `remote_parts` table. The file row's `remote_fs_id`/`remote_md5` are `NULL` and `path` is a placeholder token (the real objects are in `remote_parts`). Download (`download_worker::download_split`) `unwrap_append`s parts in order and asserts the reassembled length == `remote_size`. **Invariant: the download path is chosen from `remote_container` ONLY, never from file size** (`download_worker::is_multipart`) — an existing single `'bbx1'` object larger than `PART_SIZE` must keep the single-object path. Deleting a file must go through `remote::delete_on_remote_for_file` (not `delete_on_remote`) so all part objects are removed; `remote_parts` rows are FK `ON DELETE CASCADE`.

- **Baidu Upload Slice Size**: `providers::baidu_netdisk::upload::SLICE_SIZE` is **32 MB** (超级会员/SVIP max). Baidu caps a single upload's `block_list` around 2048 slices, so the old fixed 4 MB died at partseq 2048 on files > 8 GB (errno 31299 "Invalid param part_id"). 32 MB **requires an SVIP account** — 普通会员 caps at 16 MB, 普通用户 at 4 MB. Slice POSTs use the larger `SLICE_TIMEOUT` (600 s), not `REQUEST_TIMEOUT`, because 32 MB over a slow uplink exceeds the 120 s control-call timeout. Slice uploads stay sequential (`SLICE_CONCURRENCY = 1`) regardless of tier — `superfile2` drops bodies under concurrent POSTs.

- **React useEffect**: Avoid circular dependencies between effects. Don't sync internal state with props in one effect and call `onChange` in another - this causes infinite loops. Derive values from props directly.

- **New Tauri Commands**: Must be (1) created in `src-tauri/src/commands/`, (2) exported in `mod.rs`, (3) registered in `lib.rs` invoke_handler.

- **Tailwind 4.x**: Uses `@plugin 'tailwindcss-animate'` syntax in CSS, not postcss.config.js.

## Agent Principles

- **Think before acting**: Don't mechanically execute user input. Follow the loop: think → explore → plan → execute → validate. Understand the problem space before writing code.
- **Ask when uncertain**: If anything is unclear or ambiguous, ask the user for clarification rather than guessing.
