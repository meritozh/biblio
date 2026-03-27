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
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  onEdit: (file: FileEntry) => void;
  onDelete: (file: FileEntry) => void;
}

export function FileContextMenu({
  file,
  open: externalOpen,
  onOpenChange: externalOnOpenChange,
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
    externalOnOpenChange?.(false);
  };

  const handleCopyPath = async () => {
    try {
      await navigator.clipboard.writeText(file.path);
    } catch (error) {
      console.error('Failed to copy path:', error);
    }
    externalOnOpenChange?.(false);
  };

  const handleEdit = () => {
    onEdit(file);
    externalOnOpenChange?.(false);
  };

  const handleDelete = () => {
    onDelete(file);
    externalOnOpenChange?.(false);
  };

  return (
    <DropdownMenu open={externalOpen} onOpenChange={externalOnOpenChange}>
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