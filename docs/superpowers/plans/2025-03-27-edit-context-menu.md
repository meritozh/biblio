# Edit Item with Context Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add right-click context menu and three-dot button to file rows for editing, deleting, opening in Finder, and copying file paths.

**Architecture:** Create a FileContextMenu component using shadcn's DropdownMenu, extract the existing add dialog into a reusable EditFileDialog, and integrate into FileList table rows.

**Tech Stack:** React, TypeScript, Tauri 2.x, shadcn/ui, Radix UI DropdownMenu

---

## File Structure

| File | Responsibility |
|------|----------------|
| `src/components/FileContextMenu.tsx` | Dropdown menu with Edit/Delete/Open in Finder/Copy Path actions |
| `src/components/ConfirmDeleteDialog.tsx` | Simple confirmation dialog before delete |
| `src/components/EditFileDialog.tsx` | Reusable dialog for both add and edit modes |
| `src/components/FileList.tsx` | Integrate context menu into table rows |
| `src/routes/index.tsx` | Wire up edit/delete handlers |
| `src/lib/tauri.ts` | Add `revealItemInDir` function |

---

### Task 1: Add revealItemInDir to Tauri Wrapper

**Files:**
- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Add revealItemInDir function**

Add the function to `src/lib/tauri.ts` after the existing opener-related imports are verified:

```typescript
// Add after storageCheckAccess function (around line 184)
export async function revealItemInDir(path: string): Promise<void> {
  return invoke('plugin:opener|reveal_item_in_dir', { path });
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `pnpm typecheck`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add src/lib/tauri.ts
git commit -m "feat: add revealItemInDir helper for opening files in Finder"
```

---

### Task 2: Create FileContextMenu Component

**Files:**
- Create: `src/components/FileContextMenu.tsx`

- [ ] **Step 1: Create FileContextMenu component**

Create `src/components/FileContextMenu.tsx`:

```typescript
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Button } from '@/components/ui/button';
import { MoreHorizontal, Pencil, Trash2, FolderOpen, Copy } from 'lucide-react';
import { revealItemInDir } from '@/lib/tauri';
import type { FileEntry } from '@/types';

interface FileContextMenuProps {
  file: FileEntry;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onEdit: (file: FileEntry) => void;
  onDelete: (file: FileEntry) => void;
}

export function FileContextMenu({
  file,
  open,
  onOpenChange,
  onEdit,
  onDelete,
}: FileContextMenuProps) {
  const handleOpenInFinder = async () => {
    try {
      await revealItemInDir(file.path);
    } catch (error) {
      // Silently fail - file may be missing
      console.error('Failed to reveal file:', error);
    }
    onOpenChange(false);
  };

  const handleCopyPath = async () => {
    try {
      await navigator.clipboard.writeText(file.path);
    } catch (error) {
      console.error('Failed to copy path:', error);
    }
    onOpenChange(false);
  };

  const handleEdit = () => {
    onEdit(file);
    onOpenChange(false);
  };

  const handleDelete = () => {
    onDelete(file);
    onOpenChange(false);
  };

  return (
    <DropdownMenu open={open} onOpenChange={onOpenChange}>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          aria-label={`Actions for ${file.display_name}`}
          onClick={(e) => {
            e.stopPropagation();
          }}
        >
          <MoreHorizontal className="h-4 w-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onClick={handleEdit}>
          <Pencil className="h-4 w-4 mr-2" />
          Edit
        </DropdownMenuItem>
        <DropdownMenuItem onClick={handleDelete} className="text-destructive focus:text-destructive">
          <Trash2 className="h-4 w-4 mr-2" />
          Delete
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem onClick={handleOpenInFinder}>
          <FolderOpen className="h-4 w-4 mr-2" />
          Open in Finder
        </DropdownMenuItem>
        <DropdownMenuItem onClick={handleCopyPath}>
          <Copy className="h-4 w-4 mr-2" />
          Copy Path
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `pnpm typecheck`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add src/components/FileContextMenu.tsx
git commit -m "feat: add FileContextMenu component with edit/delete/open/copy actions"
```

---

### Task 3: Create ConfirmDeleteDialog Component

**Files:**
- Create: `src/components/ConfirmDeleteDialog.tsx`

- [ ] **Step 1: Create ConfirmDeleteDialog component**

Create `src/components/ConfirmDeleteDialog.tsx`:

```typescript
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';

interface ConfirmDeleteDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  fileName: string;
  onConfirm: () => Promise<void>;
}

export function ConfirmDeleteDialog({
  open,
  onOpenChange,
  fileName,
  onConfirm,
}: ConfirmDeleteDialogProps) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Delete file?</AlertDialogTitle>
          <AlertDialogDescription>
            Remove "{fileName}" from your library? The original file will not be deleted from disk.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction asChild>
            <Button variant="destructive" onClick={onConfirm}>
              Delete
            </Button>
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `pnpm typecheck`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add src/components/ConfirmDeleteDialog.tsx
git commit -m "feat: add ConfirmDeleteDialog component"
```

---

### Task 4: Create EditFileDialog Component

**Files:**
- Create: `src/components/EditFileDialog.tsx`

- [ ] **Step 1: Create EditFileDialog component**

Create `src/components/EditFileDialog.tsx`:

```typescript
import { useEffect, useState } from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { DynamicMetadataForm, type DynamicMetadataFormValues } from '@/components/DynamicMetadataForm';
import type { FileEntry, Category, Tag, Author } from '@/types';

interface EditFileDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  file: FileEntry | null;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onCategoryCreated: (category: Category) => void;
  onTagCreate: (name: string) => Promise<Tag>;
  onAuthorCreate: (name: string) => Promise<Author>;
  onSave: (fileId: number, values: DynamicMetadataFormValues) => Promise<void>;
}

export function EditFileDialog({
  open,
  onOpenChange,
  file,
  categories,
  tags,
  authors,
  onCategoryCreated,
  onTagCreate,
  onAuthorCreate,
  onSave,
}: EditFileDialogProps) {
  const [saving, setSaving] = useState(false);
  const [formValues, setFormValues] = useState<DynamicMetadataFormValues>({
    display_name: '',
    category_id: null,
    tag_ids: [],
    author_ids: [],
    metadata: [],
  });

  // Populate form when file changes
  useEffect(() => {
    if (file) {
      setFormValues({
        display_name: file.display_name,
        category_id: file.category_id,
        tag_ids: file.tags?.map((t) => t.id) ?? [],
        author_ids: file.authors?.map((a) => a.id) ?? [],
        metadata: file.metadata?.map((m) => ({
          key: m.key,
          value: m.value,
          data_type: m.data_type,
        })) ?? [],
      });
    }
  }, [file]);

  const handleSave = async () => {
    if (!file || saving) return;
    setSaving(true);
    try {
      await onSave(file.id, formValues);
      onOpenChange(false);
    } catch (error) {
      console.error('Failed to save:', error);
      alert(`Failed to save: ${error}`);
    }
    setSaving(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Edit File</DialogTitle>
        </DialogHeader>
        <DynamicMetadataForm
          values={formValues}
          onChange={setFormValues}
          categories={categories}
          tags={tags}
          authors={authors}
          onCategoryCreated={onCategoryCreated}
          onTagCreate={onTagCreate}
          onAuthorCreate={onAuthorCreate}
          fileId={file?.id}
          inStorage={file?.in_storage}
        />
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={saving}>
            {saving ? 'Saving...' : 'Save'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `pnpm typecheck`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add src/components/EditFileDialog.tsx
git commit -m "feat: add EditFileDialog component for editing file metadata"
```

---

### Task 5: Integrate Context Menu into FileList

**Files:**
- Modify: `src/components/FileList.tsx`

- [ ] **Step 1: Add imports for context menu**

Add at the top of `src/components/FileList.tsx`:

```typescript
import { useState } from 'react';
import { FileContextMenu } from '@/components/FileContextMenu';
```

- [ ] **Step 2: Add props for context menu state**

Update the `FileListProps` interface to include:

```typescript
interface FileListProps {
  files: FileEntry[];
  onFileClick?: (file: FileEntry) => void;
  onFileEdit?: (file: FileEntry) => void;
  onFileDelete?: (file: FileEntry) => void;
}
```

(These props already exist - just verify they're present)

- [ ] **Step 3: Add context menu state and update table component**

Replace the `FileList` component function with the updated version that includes right-click support:

```typescript
export function FileList({ files, onFileClick, onFileEdit, onFileDelete }: FileListProps) {
  const [sorting, setSorting] = useState<{ id: string; desc: boolean }[]>([]);
  const [contextMenuFileId, setContextMenuFileId] = useState<number | null>(null);

  const columns = [
    {
      accessorKey: 'display_name',
      header: ({
        column,
      }: {
        column: { getIsSorted: () => string | false; toggleSorting: (desc?: boolean) => void };
      }) => (
        <Button
          variant="ghost"
          onClick={() => column.toggleSorting(column.getIsSorted() === 'asc')}
          aria-label={`Sort by name ${column.getIsSorted() === 'asc' ? 'descending' : 'ascending'}`}
        >
          Name
          <ArrowUpDown className="ml-2 h-4 w-4" aria-hidden="true" />
        </Button>
      ),
    },
    {
      accessorKey: 'path',
      header: 'Path',
      cell: ({ row }: { row: { original: FileEntry } }) => (
        <span className="text-muted-foreground truncate max-w-md block">{row.original.path}</span>
      ),
    },
    {
      accessorKey: 'file_status',
      header: 'Status',
      cell: ({ row }: { row: { original: FileEntry } }) => {
        const status = row.original.file_status;
        const colors = {
          available: 'text-success',
          missing: 'text-destructive',
          moved: 'text-warning',
        };
        return <span className={colors[status as keyof typeof colors] || ''}>{status}</span>;
      },
    },
    {
      accessorKey: 'created_at',
      header: 'Added',
      cell: ({ row }: { row: { original: FileEntry } }) =>
        new Date(row.original.created_at).toLocaleDateString(),
    },
    {
      id: 'actions',
      cell: ({ row }: { row: { original: FileEntry } }) => {
        const file = row.original;
        return (
          <div className="flex justify-end" role="group" aria-label="File actions">
            {onFileEdit && onFileDelete && (
              <FileContextMenu
                file={file}
                open={contextMenuFileId === file.id}
                onOpenChange={(open) => setContextMenuFileId(open ? file.id : null)}
                onEdit={onFileEdit}
                onDelete={onFileDelete}
              />
            )}
          </div>
        );
      },
    },
  ];

  const table = useReactTable({
    data: files,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    getSortedRowModel: getSortedRowModel(),
    onSortingChange: setSorting,
    state: { sorting },
  });

  return (
    <div className="space-y-4">
      <div className="rounded-md border">
        <Table>
          <TableHeader>
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow key={headerGroup.id}>
                {headerGroup.headers.map((header) => (
                  <TableHead key={header.id}>
                    {header.isPlaceholder
                      ? null
                      : flexRender(header.column.columnDef.header, header.getContext())}
                  </TableHead>
                ))}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {table.getRowModel().rows?.length ? (
              table.getRowModel().rows.map((row) => (
                <TableRow
                  key={row.id}
                  data-state={row.getIsSelected() && 'selected'}
                  className={onFileClick ? 'cursor-pointer hover:bg-muted/50' : ''}
                  onClick={() => onFileClick?.(row.original)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    if (onFileEdit && onFileDelete) {
                      setContextMenuFileId(row.original.id);
                    }
                  }}
                  tabIndex={onFileClick ? 0 : undefined}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && onFileClick) {
                      onFileClick(row.original);
                    }
                  }}
                  role={onFileClick ? 'button' : undefined}
                  aria-label={onFileClick ? `View ${row.original.display_name}` : undefined}
                >
                  {row.getVisibleCells().map((cell) => (
                    <TableCell key={cell.id}>
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </TableCell>
                  ))}
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell colSpan={columns.length} className="h-24 text-center">
                  No files in library.
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
      <div
        className="flex items-center justify-end space-x-2"
        role="navigation"
        aria-label="Table pagination"
      >
        <Button
          variant="outline"
          size="sm"
          onClick={() => table.previousPage()}
          disabled={!table.getCanPreviousPage()}
          aria-label="Previous page"
        >
          <ChevronLeft className="h-4 w-4" aria-hidden="true" />
          Previous
        </Button>
        <span aria-live="polite" className="text-sm text-muted-foreground">
          Page {table.getState().pagination.pageIndex + 1} of {table.getPageCount()}
        </span>
        <Button
          variant="outline"
          size="sm"
          onClick={() => table.nextPage()}
          disabled={!table.getCanNextPage()}
          aria-label="Next page"
        >
          Next
          <ChevronRight className="h-4 w-4" aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `pnpm typecheck`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add src/components/FileList.tsx
git commit -m "feat: integrate FileContextMenu into FileList table"
```

---

### Task 6: Wire Up Handlers in HomePage

**Files:**
- Modify: `src/routes/index.tsx`

- [ ] **Step 1: Add imports for new components**

Add to imports at the top of `src/routes/index.tsx`:

```typescript
import { EditFileDialog } from '@/components/EditFileDialog';
import { ConfirmDeleteDialog } from '@/components/ConfirmDeleteDialog';
import { fileUpdate, fileDelete, tagAssign, authorSet, metadataSet, metadataDelete } from '@/lib/tauri';
```

- [ ] **Step 2: Add state for edit and delete dialogs**

Add state variables after the existing state declarations (around line 48):

```typescript
  const [editDialogOpen, setEditDialogOpen] = useState(false);
  const [editingFile, setEditingFile] = useState<FileEntry | null>(null);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletingFile, setDeletingFile] = useState<FileEntry | null>(null);
```

- [ ] **Step 3: Add edit handler**

Add after `handleAuthorCreate` function (around line 200):

```typescript
  const handleFileEdit = useCallback((file: FileEntry) => {
    setEditingFile(file);
    setEditDialogOpen(true);
  }, []);

  const handleFileSave = useCallback(async (fileId: number, values: DynamicMetadataFormValues) => {
    // Update basic fields
    await fileUpdate(fileId, {
      display_name: values.display_name,
      category_id: values.category_id,
    });

    // Update tags
    await tagAssign(fileId, values.tag_ids);

    // Update authors
    await authorSet(fileId, values.author_ids);

    // Update metadata - first clear existing, then set new
    if (editingFile?.metadata) {
      for (const m of editingFile.metadata) {
        await metadataDelete(fileId, m.key);
      }
    }
    for (const m of values.metadata) {
      await metadataSet(fileId, m.key, m.value, m.data_type);
    }

    void loadFiles(selectedCategoryId);
  }, [editingFile, selectedCategoryId, loadFiles]);
```

- [ ] **Step 4: Add delete handler**

Add after `handleFileSave`:

```typescript
  const handleFileDeleteClick = useCallback((file: FileEntry) => {
    setDeletingFile(file);
    setDeleteDialogOpen(true);
  }, []);

  const handleFileDeleteConfirm = useCallback(async () => {
    if (!deletingFile) return;
    try {
      await fileDelete(deletingFile.id);
      setDeleteDialogOpen(false);
      setDeletingFile(null);
      void loadFiles(selectedCategoryId);
    } catch (error) {
      console.error('Failed to delete:', error);
      alert(`Failed to delete: ${error}`);
    }
  }, [deletingFile, selectedCategoryId, loadFiles]);
```

- [ ] **Step 5: Update FileList props to pass handlers**

Find the `<FileList>` component (around line 259) and update:

```typescript
            <FileList
              files={files}
              onFileClick={handleFileClick}
              onFileEdit={handleFileEdit}
              onFileDelete={handleFileDeleteClick}
            />
```

- [ ] **Step 6: Add EditFileDialog and ConfirmDeleteDialog to JSX**

Add before the closing `</div>` of the main container (after the existing Dialog for adding files):

```typescript
      <EditFileDialog
        open={editDialogOpen}
        onOpenChange={setEditDialogOpen}
        file={editingFile}
        categories={categories}
        tags={tags}
        authors={authors}
        onCategoryCreated={handleCategoryCreated}
        onTagCreate={handleTagCreate}
        onAuthorCreate={handleAuthorCreate}
        onSave={handleFileSave}
      />

      <ConfirmDeleteDialog
        open={deleteDialogOpen}
        onOpenChange={setDeleteDialogOpen}
        fileName={deletingFile?.display_name ?? ''}
        onConfirm={handleFileDeleteConfirm}
      />
```

- [ ] **Step 7: Verify TypeScript compiles**

Run: `pnpm typecheck`
Expected: No errors

- [ ] **Step 8: Commit**

```bash
git add src/routes/index.tsx
git commit -m "feat: wire up edit and delete handlers in HomePage"
```

---

### Task 7: Integration Testing

- [ ] **Step 1: Start the dev server**

Run: `pnpm tauri:dev`
Expected: App starts without errors

- [ ] **Step 2: Test context menu appears on click**

- Click the three-dot button on any file row
- Verify dropdown menu appears with Edit, Delete, separator, Open in Finder, Copy Path

- [ ] **Step 3: Test Edit functionality**

- Click "Edit" from context menu
- Verify dialog opens with pre-populated file data
- Modify display name or other fields
- Click Save
- Verify changes persist after page refresh

- [ ] **Step 4: Test Delete functionality**

- Click "Delete" from context menu
- Verify confirmation dialog appears
- Click Delete
- Verify file is removed from list

- [ ] **Step 5: Test Open in Finder**

- Click "Open in Finder" from context menu
- Verify Finder opens with file selected

- [ ] **Step 6: Test Copy Path**

- Click "Copy Path" from context menu
- Paste somewhere to verify path was copied

- [ ] **Step 7: Final commit if any fixes needed**

```bash
git add -A
git commit -m "fix: resolve issues found during testing"
```