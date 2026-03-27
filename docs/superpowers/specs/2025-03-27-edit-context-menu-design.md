# Edit Item with Context Menu

**Date:** 2025-03-27

## Overview

Add context menu support to the library file list, allowing users to edit file metadata, delete files, open files in Finder, and copy file paths.

## Requirements

- Right-click context menu on file rows
- Three-dot "⋯" button in each row as an additional trigger
- Edit action opens full metadata editing dialog (same as add dialog)
- Delete action with confirmation
- Open in Finder action
- Copy Path action

## Components

### FileContextMenu

New component wrapping file list items with context menu functionality.

**Props:**
```typescript
interface FileContextMenuProps {
  file: FileEntry;
  onEdit: (file: FileEntry) => void;
  onDelete: (file: FileEntry) => void;
  children: React.ReactNode;
}
```

**Behavior:**
- Uses shadcn `DropdownMenu` for the three-dot button (more control over trigger)
- Right-click on row also opens the same `DropdownMenu` via `onContextMenu` + state control
- Menu items: Edit, Delete, separator, Open in Finder, Copy Path

### EditFileDialog

Refactored from existing add dialog logic for reusability.

**Props:**
```typescript
interface EditFileDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  file: FileEntry | null; // null for add mode
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onCategoryCreated: (category: Category) => void;
  onTagCreate: (name: string) => Promise<Tag>;
  onAuthorCreate: (name: string) => Promise<Author>;
  onSave: (fileId: number | null, values: DynamicMetadataFormValues) => Promise<void>;
}
```

**Behavior:**
- Uses existing `DynamicMetadataForm` component
- Pre-populates form with existing file data when editing
- Title changes: "Add Files" vs "Edit File"
- Save button text: "Add" vs "Save"

### ConfirmDeleteDialog

Simple confirmation dialog before delete.

**Props:**
```typescript
interface ConfirmDeleteDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  fileName: string;
  onConfirm: () => Promise<void>;
}
```

## Data Flow

```
FileList row
    ├── onContextMenu → opens context menu
    ├── three-dot button onClick → opens context menu
    └── menu items:
        ├── Edit → opens EditFileDialog with file data
        ├── Delete → opens ConfirmDeleteDialog
        ├── separator
        ├── Open in Finder → calls revealItemInDir
        └── Copy Path → navigator.clipboard.writeText()
```

## Backend Integration

| Action | Backend Call | Notes |
|--------|-------------|-------|
| Edit | `fileUpdate(id, updates)` | Already exists |
| Delete | `fileDelete(id)` | Already exists |
| Open in Finder | `revealItemInDir(path)` | From tauri-plugin-opener |
| Copy Path | Browser clipboard API | No backend needed |

## File Changes

### New Files
- `src/components/FileContextMenu.tsx` — Context menu wrapper component
- `src/components/EditFileDialog.tsx` — Reusable edit/add dialog
- `src/components/ConfirmDeleteDialog.tsx` — Delete confirmation dialog

### Modified Files
- `src/components/FileList.tsx` — Integrate context menu into table rows
- `src/routes/index.tsx` — Wire up edit/delete handlers, use EditFileDialog
- `src/lib/tauri.ts` — Add `revealItemInDir` wrapper if not present

## Error Handling

| Scenario | Handling |
|----------|----------|
| Edit save fails | Show alert with error, keep dialog open |
| Delete fails | Show alert with error, keep confirmation open |
| Open in Finder fails | Silently fail (file may be missing) |
| Copy Path fails | Show toast notification |

## Implementation Order

1. Create `FileContextMenu` component with menu items
2. Create `ConfirmDeleteDialog` component
3. Extract `EditFileDialog` from existing add dialog logic
4. Integrate context menu into `FileList` table rows
5. Wire up handlers in `routes/index.tsx`
6. Test all actions