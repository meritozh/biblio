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
