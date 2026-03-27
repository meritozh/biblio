# File Context Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add right-click context menu to library file items with Edit, Delete, Open in Finder, and Copy Path actions.

**Architecture:** Create reusable `FileContextMenu` component using Radix ContextMenu, integrate into FileList (table rows) and FileCard (card view). Use existing `tauri-plugin-opener` API for "Open in Finder" functionality.

**Tech Stack:** React, TypeScript, Radix UI Context Menu, Tauri, tauri-plugin-opener

---

## File Structure

| File                                 | Purpose                                                    |
| ------------------------------------ | ---------------------------------------------------------- |
| `src/components/ui/context-menu.tsx` | Radix ContextMenu primitives styled to match design system |
| `src/components/FileContextMenu.tsx` | Shared context menu component with file actions            |
| `src/components/FileList.tsx`        | Wrap table rows with FileContextMenu                       |
| `src/components/FileCard.tsx`        | Wrap card with FileContextMenu                             |
| `src/lib/tauri.ts`                   | Add wrapper for revealItemInDir from tauri-plugin-opener   |

---

### Task 1: Install Radix Context Menu dependency

**Files:**

- Modify: `package.json`

- [ ] **Step 1: Install the package**

Run: `pnpm add @radix-ui/react-context-menu`

Expected: Package added successfully

- [ ] **Step 2: Commit**

```bash
git add package.json pnpm-lock.yaml
git commit -m "chore: add @radix-ui/react-context-menu dependency"
```

---

### Task 2: Create context-menu.tsx UI primitive

**Files:**

- Create: `src/components/ui/context-menu.tsx`

- [ ] **Step 1: Create the context menu UI component**

```tsx
import * as React from 'react';
import * as ContextMenuPrimitive from '@radix-ui/react-context-menu';
import { Check, ChevronRight, Circle } from 'lucide-react';

import { cn } from '@/lib/utils';

const ContextMenu = ContextMenuPrimitive.Root;

const ContextMenuTrigger = ContextMenuPrimitive.Trigger;

const ContextMenuGroup = ContextMenuPrimitive.Group;

const ContextMenuPortal = ContextMenuPrimitive.Portal;

const ContextMenuSub = ContextMenuPrimitive.Sub;

const ContextMenuRadioGroup = ContextMenuPrimitive.RadioGroup;

const ContextMenuSubTrigger = React.forwardRef<
  React.ElementRef<typeof ContextMenuPrimitive.SubTrigger>,
  React.ComponentPropsWithoutRef<typeof ContextMenuPrimitive.SubTrigger> & {
    inset?: boolean;
  }
>(({ className, inset, children, ...props }, ref) => (
  <ContextMenuPrimitive.SubTrigger
    ref={ref}
    className={cn(
      'flex cursor-default select-none items-center gap-2 rounded-xl px-2 py-1.5 text-sm outline-none focus:bg-accent data-[state=open]:bg-accent [&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0',
      inset && 'pl-8',
      className
    )}
    {...props}
  >
    {children}
    <ChevronRight className="ml-auto" />
  </ContextMenuPrimitive.SubTrigger>
));
ContextMenuSubTrigger.displayName = ContextMenuPrimitive.SubTrigger.displayName;

const ContextMenuSubContent = React.forwardRef<
  React.ElementRef<typeof ContextMenuPrimitive.SubContent>,
  React.ComponentPropsWithoutRef<typeof ContextMenuPrimitive.SubContent>
>(({ className, ...props }, ref) => (
  <ContextMenuPrimitive.SubContent
    ref={ref}
    className={cn(
      'z-50 min-w-[8rem] overflow-hidden rounded-xl border bg-popover p-1 text-popover-foreground shadow-lg data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 origin-[--radix-context-menu-content-transform-origin]',
      className
    )}
    {...props}
  />
));
ContextMenuSubContent.displayName = ContextMenuPrimitive.SubContent.displayName;

const ContextMenuContent = React.forwardRef<
  React.ElementRef<typeof ContextMenuPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof ContextMenuPrimitive.Content>
>(({ className, ...props }, ref) => (
  <ContextMenuPrimitive.Portal>
    <ContextMenuPrimitive.Content
      ref={ref}
      className={cn(
        'z-50 min-w-[8rem] overflow-hidden rounded-xl border bg-popover p-1 text-popover-foreground shadow-lg data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 origin-[--radix-context-menu-content-transform-origin]',
        className
      )}
      {...props}
    />
  </ContextMenuPrimitive.Portal>
));
ContextMenuContent.displayName = ContextMenuPrimitive.Content.displayName;

const ContextMenuItem = React.forwardRef<
  React.ElementRef<typeof ContextMenuPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof ContextMenuPrimitive.Item> & {
    inset?: boolean;
  }
>(({ className, inset, ...props }, ref) => (
  <ContextMenuPrimitive.Item
    ref={ref}
    className={cn(
      'relative flex cursor-default select-none items-center gap-2 rounded-xl px-2 py-1.5 text-sm outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&>svg]:size-4 [&>svg]:shrink-0',
      inset && 'pl-8',
      className
    )}
    {...props}
  />
));
ContextMenuItem.displayName = ContextMenuPrimitive.Item.displayName;

const ContextMenuCheckboxItem = React.forwardRef<
  React.ElementRef<typeof ContextMenuPrimitive.CheckboxItem>,
  React.ComponentPropsWithoutRef<typeof ContextMenuPrimitive.CheckboxItem>
>(({ className, children, checked, ...props }, ref) => (
  <ContextMenuPrimitive.CheckboxItem
    ref={ref}
    className={cn(
      'relative flex cursor-default select-none items-center rounded-xl py-1.5 pl-8 pr-2 text-sm outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50',
      className
    )}
    checked={checked}
    {...props}
  >
    <span className="absolute left-2 flex h-3.5 w-3.5 items-center justify-center">
      <ContextMenuPrimitive.ItemIndicator>
        <Check className="h-4 w-4" />
      </ContextMenuPrimitive.ItemIndicator>
    </span>
    {children}
  </ContextMenuPrimitive.CheckboxItem>
));
ContextMenuCheckboxItem.displayName = ContextMenuPrimitive.CheckboxItem.displayName;

const ContextMenuRadioItem = React.forwardRef<
  React.ElementRef<typeof ContextMenuPrimitive.RadioItem>,
  React.ComponentPropsWithoutRef<typeof ContextMenuPrimitive.RadioItem>
>(({ className, children, ...props }, ref) => (
  <ContextMenuPrimitive.RadioItem
    ref={ref}
    className={cn(
      'relative flex cursor-default select-none items-center rounded-xl py-1.5 pl-8 pr-2 text-sm outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50',
      className
    )}
    {...props}
  >
    <span className="absolute left-2 flex h-3.5 w-3.5 items-center justify-center">
      <ContextMenuPrimitive.ItemIndicator>
        <Circle className="h-2 w-2 fill-current" />
      </ContextMenuPrimitive.ItemIndicator>
    </span>
    {children}
  </ContextMenuPrimitive.RadioItem>
));
ContextMenuRadioItem.displayName = ContextMenuPrimitive.RadioItem.displayName;

const ContextMenuLabel = React.forwardRef<
  React.ElementRef<typeof ContextMenuPrimitive.Label>,
  React.ComponentPropsWithoutRef<typeof ContextMenuPrimitive.Label> & {
    inset?: boolean;
  }
>(({ className, inset, ...props }, ref) => (
  <ContextMenuPrimitive.Label
    ref={ref}
    className={cn('px-2 py-1.5 text-sm font-semibold', inset && 'pl-8', className)}
    {...props}
  />
));
ContextMenuLabel.displayName = ContextMenuPrimitive.Label.displayName;

const ContextMenuSeparator = React.forwardRef<
  React.ElementRef<typeof ContextMenuPrimitive.Separator>,
  React.ComponentPropsWithoutRef<typeof ContextMenuPrimitive.Separator>
>(({ className, ...props }, ref) => (
  <ContextMenuPrimitive.Separator
    ref={ref}
    className={cn('-mx-1 my-1 h-px bg-muted', className)}
    {...props}
  />
));
ContextMenuSeparator.displayName = ContextMenuPrimitive.Separator.displayName;

const ContextMenuShortcut = ({ className, ...props }: React.HTMLAttributes<HTMLSpanElement>) => {
  return (
    <span className={cn('ml-auto text-xs tracking-widest opacity-60', className)} {...props} />
  );
};
ContextMenuShortcut.displayName = 'ContextMenuShortcut';

export {
  ContextMenu,
  ContextMenuTrigger,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuCheckboxItem,
  ContextMenuRadioItem,
  ContextMenuLabel,
  ContextMenuSeparator,
  ContextMenuShortcut,
  ContextMenuGroup,
  ContextMenuPortal,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuRadioGroup,
};
```

- [ ] **Step 2: Commit**

```bash
git add src/components/ui/context-menu.tsx
git commit -m "feat: add context-menu UI component"
```

---

### Task 3: Add revealItemInDir to tauri.ts

**Files:**

- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Add revealItemInDir function**

Add to end of file before `translateError`:

```typescript
// Reveal file in file manager
export async function revealItemInDir(path: string): Promise<void> {
  const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
  return revealItemInDir(path);
}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tauri.ts
git commit -m "feat: add revealItemInDir wrapper for tauri-plugin-opener"
```

---

### Task 4: Create FileContextMenu component

**Files:**

- Create: `src/components/FileContextMenu.tsx`

- [ ] **Step 1: Create the FileContextMenu component**

```tsx
import { Pencil, Trash2, FolderOpen, Copy } from 'lucide-react';
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu';
import { revealItemInDir } from '@/lib/tauri';
import type { FileEntry } from '@/types';

interface FileContextMenuProps {
  file: FileEntry;
  onEdit?: (file: FileEntry) => void;
  onDelete?: (file: FileEntry) => void;
  children: React.ReactNode;
}

export function FileContextMenu({ file, onEdit, onDelete, children }: FileContextMenuProps) {
  const handleCopyPath = async () => {
    try {
      await navigator.clipboard.writeText(file.path);
    } catch (error) {
      console.error('Failed to copy path:', error);
      alert('Could not copy to clipboard');
    }
  };

  const handleRevealInFinder = async () => {
    try {
      await revealItemInDir(file.path);
    } catch (error) {
      console.error('Failed to reveal in finder:', error);
      alert('File not found at this location');
    }
  };

  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent className="w-48">
        {onEdit && (
          <ContextMenuItem onClick={() => onEdit(file)}>
            <Pencil className="h-4 w-4 mr-2" />
            Edit
          </ContextMenuItem>
        )}
        {onDelete && (
          <ContextMenuItem
            onClick={() => onDelete(file)}
            className="text-destructive focus:text-destructive"
          >
            <Trash2 className="h-4 w-4 mr-2" />
            Delete
          </ContextMenuItem>
        )}
        <ContextMenuSeparator />
        <ContextMenuItem onClick={handleRevealInFinder}>
          <FolderOpen className="h-4 w-4 mr-2" />
          Open in Finder
        </ContextMenuItem>
        <ContextMenuItem onClick={handleCopyPath}>
          <Copy className="h-4 w-4 mr-2" />
          Copy Path
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/components/FileContextMenu.tsx
git commit -m "feat: add FileContextMenu component with edit/delete/reveal/copy actions"
```

---

### Task 5: Integrate FileContextMenu into FileList

**Files:**

- Modify: `src/components/FileList.tsx`

- [ ] **Step 1: Add import for FileContextMenu**

Add to imports at top:

```tsx
import { FileContextMenu } from '@/components/FileContextMenu';
```

- [ ] **Step 2: Wrap TableRow with FileContextMenu**

Replace the `TableRow` rendering (lines 134-153) with:

```tsx
{table.getRowModel().rows?.length ? (
  table.getRowModel().rows.map((row) => (
    <FileContextMenu
      key={row.id}
      file={row.original}
      onEdit={onFileEdit}
      onDelete={onFileDelete}
    >
      <TableRow
        data-state={row.getIsSelected() && 'selected'}
        className={onFileClick ? 'cursor-pointer hover:bg-muted/50' : ''}
        onClick={() => onFileClick?.(row.original)}
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
    </FileContextMenu>
  ))
) : (
```

- [ ] **Step 3: Commit**

```bash
git add src/components/FileList.tsx
git commit -m "feat: integrate FileContextMenu into FileList table rows"
```

---

### Task 6: Integrate FileContextMenu into FileCard

**Files:**

- Modify: `src/components/FileCard.tsx`

- [ ] **Step 1: Add import for FileContextMenu**

Add to imports at top:

```tsx
import { FileContextMenu } from '@/components/FileContextMenu';
```

- [ ] **Step 2: Wrap Card with FileContextMenu**

Replace the return statement (lines 27-121) with:

```tsx
return (
  <FileContextMenu file={file} onEdit={onEdit} onDelete={onDelete}>
    <Card className="group">
      <CardContent className="p-0">
        <div className="flex items-start justify-between gap-4 p-4">
          <div className="flex items-start gap-3 min-w-0 flex-1">
            <div className="mt-0.5">
              <FileText className="h-5 w-5 text-muted-foreground shrink-0" aria-hidden="true" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <h3 className="font-medium text-sm truncate" id={`file-name-${file.id}`}>
                  {file.display_name}
                </h3>
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <div
                        className={`w-1.5 h-1.5 rounded-full ${statusColor[file.file_status as keyof typeof statusColor]}`}
                        role="status"
                        aria-label={statusLabel[file.file_status as keyof typeof statusLabel]}
                      />
                    </TooltipTrigger>
                    <TooltipContent side="top">
                      <p>{statusLabel[file.file_status as keyof typeof statusLabel]}</p>
                    </TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              </div>
              <TooltipProvider>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <p className="text-xs text-muted-foreground truncate mt-0.5">{file.path}</p>
                  </TooltipTrigger>
                  <TooltipContent side="top">
                    <p className="max-w-xs break-all">{file.path}</p>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
              {file.tags && file.tags.length > 0 && (
                <div className="flex flex-wrap gap-1 mt-2">
                  {file.tags.map((tag: Tag) => (
                    <Badge key={tag.id} variant="gray" className="text-xs font-normal">
                      {tag.name}
                    </Badge>
                  ))}
                </div>
              )}
            </div>
          </div>
          <div
            className="flex gap-0.5 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity"
            role="group"
            aria-label="File actions"
          >
            <TooltipProvider>
              {onEdit && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={(e) => {
                        e.stopPropagation();
                        onEdit(file);
                      }}
                      aria-label={`Edit ${file.display_name}`}
                    >
                      <Edit className="h-4 w-4" aria-hidden="true" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent side="top">
                    <p>Edit</p>
                  </TooltipContent>
                </Tooltip>
              )}
              {onDelete && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={(e) => {
                        e.stopPropagation();
                        onDelete(file);
                      }}
                      aria-label={`Delete ${file.display_name}`}
                    >
                      <Trash2 className="h-4 w-4" aria-hidden="true" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent side="top">
                    <p>Delete</p>
                  </TooltipContent>
                </Tooltip>
              )}
            </TooltipProvider>
          </div>
        </div>
      </CardContent>
    </Card>
  </FileContextMenu>
);
```

- [ ] **Step 3: Commit**

```bash
git add src/components/FileCard.tsx
git commit -m "feat: integrate FileContextMenu into FileCard"
```

---

### Task 7: Run typecheck and fix any issues

**Files:**

- None (verification)

- [ ] **Step 1: Run typecheck**

Run: `pnpm typecheck`

Expected: No errors

- [ ] **Step 2: Run lint**

Run: `pnpm lint`

Expected: No errors

- [ ] **Step 3: If errors, fix them and commit**

---

### Task 8: Final commit for feature

- [ ] **Step 1: Create final feature commit if not already done**

```bash
git add -A
git commit -m "feat: add file context menu with edit/delete/reveal/copy actions"
```
