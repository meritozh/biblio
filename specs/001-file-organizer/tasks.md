---
description: 'Task list for File Organizer feature implementation'
---

# Tasks: File Organizer

**Input**: Design documents from `/specs/001-file-organizer/`
**Prerequisites**: plan.md (required), spec.md (required for user stories), research.md, data-model.md, contracts/

**Tests**: Tests ARE included following TDD (Test-Driven Development) as required by constitution. Write tests FIRST, ensure they FAIL, then implement.

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3, US4)
- Include exact file paths in descriptions

## Path Conventions

- **Frontend**: `src/` at repository root
- **Backend**: `src-tauri/src/` at repository root
- **Tests**: `tests/` at repository root

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialization and basic structure

- [x] T001 Initialize Tauri 2.x project with React TypeScript template
- [x] T002 [P] Configure TypeScript with strict mode in tsconfig.json
- [x] T003 [P] Configure Tailwind CSS 4.x in tailwind.config.ts
- [x] T004 [P] Configure ESLint and Prettier in .eslintrc.js and .prettierrc
- [x] T005 [P] Configure Vitest in vitest.config.ts
- [x] T006 [P] Configure Playwright in playwright.config.ts
- [x] T006a [P] Configure Vitest coverage reporting in vitest.config.ts
- [x] T006b [P] Create GitHub Actions CI workflow in .github/workflows/ci.yml
- [x] T007 Install shadcn/ui and initialize in src/components/ui/
- [x] T008 [P] Add TanStack dependencies (router, table, form, store) to package.json
- [x] T009 [P] Add tauri-plugin-sql to src-tauri/Cargo.toml with sqlite feature

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core infrastructure that MUST be complete before ANY user story can be implemented

**CRITICAL**: No user story work can begin until this phase is complete

### Database Schema

- [x] T010 Create SQLite schema migration in src-tauri/src/database/schema.sql
- [x] T011 Implement database initialization in src-tauri/src/database/mod.rs
- [x] T012 Seed default categories (novel, comic, game, anime, other) in src-tauri/src/database/seed.rs

### TypeScript Types

- [x] T013 [P] Create Category type in src/types/category.ts
- [x] T014 [P] Create FileEntry type in src/types/file.ts
- [x] T015 [P] Create Tag type in src/types/tag.ts
- [x] T016 [P] Create Metadata type in src/types/metadata.ts
- [x] T017 Create index exports in src/types/index.ts

### Tauri Setup

- [x] T018 Configure tauri-plugin-sql in src-tauri/src/lib.rs
- [x] T019 Configure security allowlist in src-tauri/tauri.conf.json

### Router Setup

- [x] T020 Configure TanStack Router with route tree in src/main.tsx
- [x] T021 Create root layout route in src/routes/\_\_root.tsx

### Store Setup

- [x] T022 [P] Create fileStore atom in src/stores/fileStore.ts
- [x] T023 [P] Create categoryStore atom in src/stores/categoryStore.ts
- [x] T024 [P] Create tagStore atom in src/stores/tagStore.ts
- [x] T025 Create store index in src/stores/index.ts

### Utility Functions

- [x] T026 [P] Create Tauri IPC wrapper in src/lib/tauri.ts
- [x] T027 [P] Create utility functions in src/lib/utils.ts

**Checkpoint**: Foundation ready - user story implementation can now begin in parallel

---

## Phase 3: User Story 1 - Add and Categorize Files (Priority: P1)

**Goal**: Users can add files to the library and assign them to categories

**Independent Test**: Add files through file picker or drag-drop, assign categories, view files grouped by category

### Tests for User Story 1

> **NOTE: TDD required - write these tests FIRST, ensure they FAIL before implementation**

- [x] T028a [P] [US1] Unit test for file_list command in tests/unit/commands/file.test.ts
- [x] T029a [P] [US1] Unit test for file_get command in tests/unit/commands/file.test.ts
- [x] T030a [US1] Unit test for file_create command with duplicate detection in tests/unit/commands/file.test.ts
- [x] T031a [P] [US1] Unit test for category_list command in tests/unit/commands/category.test.ts
- [x] T033a [US1] Integration test for file add and categorize workflow in tests/integration/file.test.ts

### Backend Commands (Rust)

- [x] T028 [P] [US1] Implement file_list command in src-tauri/src/commands/file.rs
- [x] T029 [P] [US1] Implement file_get command in src-tauri/src/commands/file.rs
- [x] T030 [US1] Implement file_create command with duplicate detection in src-tauri/src/commands/file.rs
- [x] T031 [P] [US1] Implement category_list command in src-tauri/src/commands/category.rs
- [x] T032 [P] [US1] Implement category_get command in src-tauri/src/commands/category.rs
- [x] T033 [US1] Register file and category commands in src-tauri/src/lib.rs

### Frontend Components

- [x] T034 [P] [US1] Create FilePicker component using Tauri dialog API in src/components/FilePicker.tsx
- [x] T035 [P] [US1] Create FileCard component in src/components/FileCard.tsx
- [x] T036 [US1] Create FileList component with TanStack Table in src/components/FileList.tsx
- [x] T037 [US1] Create CategorySelect component in src/components/CategorySelect.tsx
- [x] T038 [US1] Create CategorySidebar component in src/components/CategorySidebar.tsx

### Frontend Pages

- [x] T039 [US1] Create library home page in src/routes/index.tsx
- [x] T040 [US1] Create category detail page in src/routes/category/$id.tsx

### Frontend Integration

- [x] T041 [US1] Implement drag-drop file import in src/components/DropZone.tsx
- [x] T042 [US1] Connect fileStore to file commands in src/stores/fileStore.ts
- [x] T043 [US1] Connect categoryStore to category commands in src/stores/categoryStore.ts

**Checkpoint**: At this point, User Story 1 should be fully functional and testable independently

---

## Phase 4: User Story 2 - Tag and Add Metadata (Priority: P2)

**Goal**: Users can add custom tags and searchable metadata to files

**Independent Test**: Add tags to files, set custom metadata fields, view files with specific tag/metadata combinations

### Tests for User Story 2

> **NOTE: TDD required - write these tests FIRST, ensure they FAIL before implementation**

- [x] T044a [P] [US2] Unit test for tag_list command in tests/unit/commands/tag.test.ts
- [x] T046a [US2] Unit test for tag_assign command in tests/unit/commands/tag.test.ts
- [x] T049a [US2] Unit test for metadata_set command in tests/unit/commands/metadata.test.ts
- [x] T050a [US2] Integration test for tag and metadata workflow in tests/integration/tag.test.ts

### Backend Commands (Rust)

- [x] T044 [P] [US2] Implement tag_list command in src-tauri/src/commands/tag.rs
- [x] T045 [P] [US2] Implement tag_create command in src-tauri/src/commands/tag.rs
- [x] T046 [US2] Implement tag_assign command in src-tauri/src/commands/tag.rs
- [x] T047 [US2] Implement tag_unassign command in src-tauri/src/commands/tag.rs
- [x] T048 [P] [US2] Implement metadata_get command in src-tauri/src/commands/metadata.rs
- [x] T049 [US2] Implement metadata_set command in src-tauri/src/commands/metadata.rs
- [x] T050 [US2] Register tag and metadata commands in src-tauri/src/lib.rs

### Frontend Components

- [x] T051 [P] [US2] Create TagBadge component in src/components/TagBadge.tsx
- [x] T052 [US2] Create TagInput component with autocomplete in src/components/TagInput.tsx
- [x] T053 [US2] Create TagManager component in src/components/TagManager.tsx
- [x] T054 [US2] Create MetadataEditor component in src/components/MetadataEditor.tsx
- [x] T055 [US2] Create MetadataField component in src/components/MetadataField.tsx

### Frontend Integration

- [x] T056 [US2] Connect tagStore to tag commands in src/stores/tagStore.ts
- [x] T057 [US2] Add tag support to FileCard component in src/components/FileCard.tsx
- [x] T058 [US2] Add metadata support to FileList component in src/components/FileList.tsx

**Checkpoint**: At this point, User Stories 1 AND 2 should both work independently

---

## Phase 5: User Story 3 - Search and Filter Files (Priority: P3)

**Goal**: Users can search across all files by name, category, tags, and metadata

**Independent Test**: Search files by name, apply multiple filters, verify results match criteria

### Tests for User Story 3

> **NOTE: TDD required - write these tests FIRST, ensure they FAIL before implementation**

- [x] T059a [US3] Unit test for file_search command in tests/unit/commands/file.test.ts
- [x] T059b [US3] Performance benchmark for search (<500ms for 10k files) in tests/benchmarks/search.bench.ts
- [x] T061a [US3] Unit test for file_check_status command in tests/unit/commands/file.test.ts
- [x] T063a [US3] Integration test for filter workflow in tests/integration/search.test.ts

### Backend Commands (Rust)

- [x] T059 [P] [US3] Implement file_search command with FTS5 in src-tauri/src/commands/file.rs
- [x] T060 [US3] Create FTS5 virtual table in database schema in src-tauri/src/database/schema.sql
- [x] T061 [US3] Implement file_check_status command in src-tauri/src/commands/file.rs

### Frontend Components

- [x] T062 [P] [US3] Create SearchBar component in src/components/SearchBar.tsx
- [x] T063 [US3] Create FilterPanel component in src/components/FilterPanel.tsx
- [x] T064 [US3] Create SearchResults component in src/components/SearchResults.tsx

### Frontend Pages

- [x] T065 [US3] Create search page in src/routes/search.tsx

### Frontend Integration

- [x] T066 [US3] Add search functionality to fileStore in src/stores/fileStore.ts
- [x] T067 [US3] Add filter state management to stores in src/stores/filterStore.ts

**Checkpoint**: At this point, User Stories 1, 2, AND 3 should all work independently

---

## Phase 6: User Story 4 - Manage Library and Files (Priority: P4)

**Goal**: Users can edit file information, remove files, and manage categories and tags

**Independent Test**: Edit file details, delete files, create/renamed/delete categories and tags

### Tests for User Story 4

> **NOTE: TDD required - write these tests FIRST, ensure they FAIL before implementation**

- [x] T068a [P] [US4] Unit test for file_update command in tests/unit/commands/file.test.ts
- [x] T069a [P] [US4] Unit test for file_delete command in tests/unit/commands/file.test.ts
- [x] T072a [US4] Unit test for category_delete command with protection in tests/unit/commands/category.test.ts
- [x] T074a [US4] Unit test for tag_delete command in tests/unit/commands/tag.test.ts
- [x] T075a [US4] Integration test for CRUD operations in tests/integration/manage.test.ts

### Backend Commands (Rust)

- [x] T068 [P] [US4] Implement file_update command in src-tauri/src/commands/file.rs
- [x] T069 [P] [US4] Implement file_delete command in src-tauri/src/commands/file.rs
- [x] T070 [P] [US4] Implement category_create command in src-tauri/src/commands/category.rs
- [x] T071 [P] [US4] Implement category_update command in src-tauri/src/commands/category.rs
- [x] T072 [US4] Implement category_delete command with protection in src-tauri/src/commands/category.rs
- [x] T073 [P] [US4] Implement tag_update command in src-tauri/src/commands/tag.rs
- [x] T074 [US4] Implement tag_delete command in src-tauri/src/commands/tag.rs
- [x] T075 [US4] Implement metadata_delete command in src-tauri/src/commands/metadata.rs

### Frontend Components

- [x] T076 [P] [US4] Create FileEditDialog component in src/components/FileEditDialog.tsx
- [x] T077 [P] [US4] Create DeleteConfirmDialog component in src/components/DeleteConfirmDialog.tsx
- [x] T078 [US4] Create CategoryManager component with CRUD in src/components/CategoryManager.tsx
- [x] T079 [US4] Create TagManager component with CRUD in src/components/TagManager.tsx (enhance existing)

### Frontend Integration

- [x] T080 [US4] Add edit/delete actions to FileCard in src/components/FileCard.tsx
- [x] T081 [US4] Add category management to sidebar in src/components/CategorySidebar.tsx

**Checkpoint**: All user stories should now be independently functional

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Improvements that affect multiple user stories

- [x] T082 [P] Implement error boundaries in src/components/ErrorBoundary.tsx
- [x] T083 [P] Add loading states to all async operations in src/components/LoadingState.tsx
- [x] T084 [P] Add empty states for zero-data scenarios in src/components/EmptyState.tsx
- [x] T084a [P] Audit UI components for WCAG 2.1 AA compliance in src/components/
- [x] T084b [P] Add ARIA labels and keyboard navigation to all interactive components
- [x] T084c Configure accessibility testing with axe-core in vitest.config.ts
- [x] T085 Implement missing file detection on startup in src-tauri/src/commands/file.rs
- [x] T085a Implement large collection import with progress indication in src/components/ImportProgress.tsx
- [x] T085b Add Unicode validation for tags/metadata in src-tauri/src/commands/validation.rs
- [x] T085c Implement database corruption recovery in src-tauri/src/database/recovery.rs
- [x] T086 [P] Add keyboard shortcuts for common actions in src/hooks/useKeyboardShortcuts.ts
- [x] T087 Configure app icon and metadata in src-tauri/tauri.conf.json
- [x] T088 [P] Add tooltips and help text throughout UI in src/components/
- [x] T089a Add performance benchmark for startup time (<3s) in tests/benchmarks/startup.bench.ts
- [x] T089 Run quickstart.md validation and fix issues

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user stories
- **User Stories (Phase 3+)**: All depend on Foundational phase completion
  - User stories can then proceed in parallel (if staffed)
  - Or sequentially in priority order (P1 → P2 → P3 → P4)
- **Polish (Phase 7)**: Depends on all desired user stories being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 2 (P2)**: Can start after Foundational (Phase 2) - Uses File entities from US1 but independently testable
- **User Story 3 (P3)**: Can start after Foundational (Phase 2) - Uses File/Tag/Metadata from US1/US2 but independently testable
- **User Story 4 (P4)**: Can start after Foundational (Phase 2) - Enhances existing components but independently testable

### Within Each User Story

- Backend commands before frontend components
- Components before pages
- Integration last
- Story complete before moving to next priority

### Parallel Opportunities

- All Setup tasks marked [P] can run in parallel
- All Foundational tasks marked [P] can run in parallel (within Phase 2)
- Once Foundational phase completes, all user stories can start in parallel (if team capacity allows)
- Within each story, tasks marked [P] can run in parallel
- Different user stories can be worked on in parallel by different team members

---

## Parallel Example: User Story 1

```bash
# Launch all backend commands for User Story 1 together:
Task: "Implement file_list command in src-tauri/src/commands/file.rs"
Task: "Implement file_get command in src-tauri/src/commands/file.rs"
Task: "Implement category_list command in src-tauri/src/commands/category.rs"
Task: "Implement category_get command in src-tauri/src/commands/category.rs"

# Launch all frontend components for User Story 1 together:
Task: "Create FilePicker component using Tauri dialog API in src/components/FilePicker.tsx"
Task: "Create FileCard component in src/components/FileCard.tsx"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1
4. **STOP and VALIDATE**: Test User Story 1 independently
5. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Foundation ready
2. Add User Story 1 → Test independently → Deploy/Demo (MVP!)
3. Add User Story 2 → Test independently → Deploy/Demo
4. Add User Story 3 → Test independently → Deploy/Demo
5. Add User Story 4 → Test independently → Deploy/Demo
6. Add Polish → Final release
7. Each story adds value without breaking previous stories

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 (backend commands)
   - Developer B: User Story 1 (frontend components)
   - Developer C: User Story 2 (can start in parallel)
3. Stories complete and integrate independently

---

## Notes

- [P] tasks = different files, no dependencies
- [Story] label maps task to specific user story for traceability
- Each user story should be independently completable and testable
- Commit after each task or logical group
- Stop at any checkpoint to validate story independently
- Avoid: vague tasks, same file conflicts, cross-story dependencies that break independence
