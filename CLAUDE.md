# biblio Development Guidelines

Auto-generated from all feature plans. Last updated: 2026-02-27

## Active Technologies

- TypeScript 5.x (frontend), Rust 1.75+ (Tauri backend) + React 19, Tauri 2.x, shadcn/ui, TanStack Router, TanStack Table, TanStack Form, TanStack Store, Tailwind CSS 4.x (001-file-organizer)

## Project Structure

```text
src/
tests/
```

## Commands

pnpm typecheck         # TypeScript check
pnpm test:run          # Run unit tests
cd src-tauri && cargo clippy  # Rust linting
pnpm tauri:dev         # Start dev server

## Code Style

TypeScript 5.x (frontend), Rust 1.75+ (Tauri backend): Follow standard conventions

## Gotchas

- **Database Migrations**: Never modify existing migration files after they've been applied. Create new migration files (e.g., `migration_2.sql`) and add them to `get_migrations()` in `src-tauri/src/database/mod.rs`.

- **React useEffect**: Avoid circular dependencies between effects. Don't sync internal state with props in one effect and call `onChange` in another - this causes infinite loops. Derive values from props directly.

- **New Tauri Commands**: Must be (1) created in `src-tauri/src/commands/`, (2) exported in `mod.rs`, (3) registered in `lib.rs` invoke_handler.

- **New UI Components**: Install radix package with `pnpm add @radix-ui/react-xxx`, then create component in `src/components/ui/`.

## Recent Changes

- 001-file-organizer: Added TypeScript 5.x (frontend), Rust 1.75+ (Tauri backend) + React 19, Tauri 2.x, shadcn/ui, TanStack Router, TanStack Table, TanStack Form, TanStack Store, Tailwind CSS 4.x

<!-- MANUAL ADDITIONS START -->

# Practice

The role of this file is to describe project principle, main goal, common mistakes and confusion points that agents might encounter as they work in this project. If you ever encounter something in the project that surprises you, please alert the developer working with you and indicate that this is the case in AgentMD file to help prevent future agents from having same issue.

Each commit you should pass ci test jobs.

Use pnpm instead of npm, bun.

<!-- MANUAL ADDITIONS END -->
