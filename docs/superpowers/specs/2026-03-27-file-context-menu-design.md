# File Context Menu Design

**Date:** 2026-03-27
**Status:** Approved

## Overview

Add a right-click context menu to library file items (in FileList table view) with common file management actions. The context menu coexists with existing action buttons as an additional interaction pattern.

## Actions

| Action         | Icon         | Behavior                                         |
| -------------- | ------------ | ------------------------------------------------ |
| Edit           | `Pencil`     | Opens the edit dialog for the file               |
| Delete         | `Trash2`     | Shows confirmation dialog, then deletes the file |
| Open in Finder | `FolderOpen` | Reveals file location in system file manager     |
| Copy Path      | `Copy`       | Copies full file path to clipboard               |

**Menu Layout:**

```
┌─────────────────────┐
│ ✏️  Edit            │
│ 🗑️  Delete          │
├─────────────────────┤
│ 📂  Open in Finder  │
│ 📋  Copy Path       │
└─────────────────────┘
```

- Separator between destructive action (Delete) and system actions
- No keyboard shortcuts in menu items (per user decision)

## Component Architecture

### FileContextMenu

**Location:** `src/components/FileContextMenu.tsx`

```typescript
interface FileContextMenuProps {
  file: FileEntry;
  onEdit?: (file: FileEntry) => void;
  onDelete?: (file: FileEntry) => void;
  children: React.ReactNode;
}
```

**Implementation:**

- Uses `@radix-ui/react-context-menu` package (install if needed)
- Wraps `children` as the trigger element via `ContextMenuTrigger`
- `ContextMenuContent` with `rounded-xl` styling matches design system
- If ContextMenu package unavailable, fallback to custom implementation using `onContextMenu` + absolute positioned menu

### Integration

**FileList (`src/components/FileList.tsx`):**

- Wrap each `TableRow` with `FileContextMenu`
- Pass `onFileEdit` and `onFileDelete` props through
- Keep existing action column buttons unchanged

**FileCard (`src/components/FileCard.tsx`):**

- Wrap the `Card` component with `FileContextMenu`
- Pass `onEdit` and `onDelete` props through
- Keep hover buttons unchanged (future-proofs for grid view)

## Backend Changes

### New Tauri Command: `file_reveal_in_finder`

**Location:** `src-tauri/src/commands/file.rs`

```rust
#[tauri::command]
pub async fn file_reveal_in_finder(path: String) -> Result<(), String> {
    // Use tauri-plugin-opener to reveal file in file manager
    // On macOS: uses NSWorkspace.activateFileViewerSelectingURLs
    // On Windows: uses explorer.exe /select
    // On Linux: uses dbus xdg-open or similar
}
```

**Registration:** Add to `invoke_handler` in `src-tauri/src/lib.rs`

**Alternative:** Check if `tauri-plugin-opener` already provides this via `reveal_item_in_dir` API.

## Clipboard Handling

For "Copy Path" action:

- Use Web API `navigator.clipboard.writeText(path)` in frontend
- No backend command needed

## Error Handling

| Action         | Error Scenario      | User Feedback                                 |
| -------------- | ------------------- | --------------------------------------------- |
| Edit           | Dialog already open | No-op (Radix handles)                         |
| Delete         | Delete fails        | Show alert with error message                 |
| Open in Finder | File not found      | Show toast: "File not found at this location" |
| Copy Path      | Clipboard denied    | Show toast: "Could not copy to clipboard"     |

## Styling

Follow existing design system:

- Menu: `rounded-xl`, `shadow-lg`, `bg-popover`
- Items: `rounded-xl` on hover, `focus:bg-accent`
- Destructive item (Delete): `text-destructive`
- Icons: `h-4 w-4`, `mr-2`

## Files Changed

| File                                 | Change                                                 |
| ------------------------------------ | ------------------------------------------------------ |
| `src/components/ui/context-menu.tsx` | New shadcn-style UI component (from Radix ContextMenu) |
| `src/components/FileContextMenu.tsx` | New component using the UI primitive                   |
| `src/components/FileList.tsx`        | Integrate context menu wrapper                         |
| `src/components/FileCard.tsx`        | Integrate context menu wrapper                         |
| `src-tauri/src/commands/file.rs`     | Add `file_reveal_in_finder` command                    |
| `src-tauri/src/commands/mod.rs`      | Export new command                                     |
| `src-tauri/src/lib.rs`               | Register new command in invoke_handler                 |
| `src/lib/tauri.ts`                   | Add `fileRevealInFinder` wrapper function              |
| `package.json`                       | Add `@radix-ui/react-context-menu` dependency          |

## Out of Scope

- Keyboard shortcuts for actions
- Batch operations (multi-select context menu)
- Custom menu items per file type
- Category/tag management in context menu
