# Implementation Plan: File Organizer

**Branch**: `001-file-organizer` | **Date**: 2026-02-27 | **Spec**: [spec.md](./spec.md)
**Input**: Feature specification from `/specs/001-file-organizer/spec.md`

## Summary

A cross-platform desktop application for organizing files by categories, tags, and searchable metadata. Built with Tauri (Rust backend) and React (TypeScript frontend), using embedded SQLite for persistence. Users can add files, assign categories (novel, comic, game, anime, etc.), add custom tags and metadata, and search/filter their collection.

## Technical Context

**Language/Version**: TypeScript 5.x (frontend), Rust 1.75+ (Tauri backend)
**Primary Dependencies**: React 19, Tauri 2.x, shadcn/ui, TanStack Router, TanStack Table, TanStack Form, TanStack Store, Tailwind CSS 4.x
**Storage**: SQLite (embedded via Tauri sql plugin)
**Testing**: Vitest (unit/integration), Playwright (E2E)
**Target Platform**: macOS 10.15+, Windows 10+
**Project Type**: desktop-app
**Performance Goals**: <500ms search for 10k files, <3s cold start, 60fps UI interactions
**Constraints**: <512MB memory, offline-capable, local filesystem only, single user
**Scale/Scope**: 10,000+ files, 50+ categories, 100+ tags, single-user desktop

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Code Quality | ✅ Pass | TypeScript + ESLint + Prettier configured; strict mode enabled |
| II. Test Standards | ✅ Pass | Vitest + Playwright planned; TDD workflow enforced; test tasks included per phase |
| III. UX Consistency | ✅ Pass | shadcn/ui provides unified design system; error boundaries for user-friendly errors |
| IV. Performance | ✅ Pass | SQLite with indexed fields; TanStack Table virtualization for large datasets |

**Quality Gates**:
- Pre-commit: ESLint, Prettier, secret scanning
- Pre-merge: Vitest unit tests pass, type check passes, coverage ≥80%
- Pre-deploy: E2E tests pass, bundle size check

## Project Structure

### Documentation (this feature)

```text
specs/001-file-organizer/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 output (via /speckit.tasks)
```

### Source Code (repository root)

```text
src-tauri/
├── src/
│   ├── commands/        # Tauri command handlers
│   ├── database/        # SQLite operations
│   └── lib.rs           # Tauri app setup
├── Cargo.toml
└── tauri.conf.json

src/
├── components/          # React components (shadcn/ui)
│   ├── ui/              # shadcn primitives
│   ├── FileList.tsx
│   ├── FileDetail.tsx
│   ├── CategoryManager.tsx
│   └── TagManager.tsx
├── pages/               # TanStack Router routes
│   ├── index.tsx        # Home/library view
│   ├── category/$id.tsx
│   └── search.tsx
├── stores/              # TanStack Store atoms
│   ├── fileStore.ts
│   ├── categoryStore.ts
│   └── tagStore.ts
├── hooks/               # Custom React hooks
├── lib/                 # Utilities
│   ├── tauri.ts         # Tauri IPC wrappers
│   └── utils.ts
├── types/               # TypeScript type definitions
├── main.tsx             # App entry point
└── routeTree.gen.ts     # Generated routes

tests/
├── unit/                # Vitest unit tests
├── integration/         # Vitest integration tests
└── e2e/                 # Playwright E2E tests

package.json
vite.config.ts
tailwind.config.ts
tsconfig.json
```

**Structure Decision**: Tauri desktop application with React frontend. Frontend code in `src/`, Rust backend in `src-tauri/`. Tests mirror source structure. This follows Tauri best practices and enables clear separation of concerns.

## Complexity Tracking

> No violations detected. All architecture decisions align with constitution principles.

| Decision | Justification |
|----------|---------------|
| TanStack Suite | Single vendor for routing, state, forms, and tables reduces integration complexity |
| shadcn/ui | Copy-paste components with Tailwind, fully customizable, no runtime dependency |
| SQLite via Tauri | Embedded database, no external process, meets offline-first requirement |