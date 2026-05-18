import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Button } from '@/components/ui/button';
import {
  Copy,
  FolderOpen,
  MoreHorizontal,
  Pencil,
  Play,
  Trash2,
  XCircle,
} from 'lucide-react';
import { cacheClear, cacheOpen, revealItemInDir } from '@/lib/tauri';
import { patchFile } from '@/stores/fileStore';
import type { FileEntry } from '@/types';

interface FileContextMenuProps {
  file: FileEntry;
  onEdit: (file: FileEntry) => void;
  onDelete: (file: FileEntry) => void;
}

export function FileContextMenu({
  file,
  onEdit,
  onDelete,
}: FileContextMenuProps) {
  const isRemote = file.storage_kind === 'remote';
  const hasCache = !!file.local_cache_path;
  // A file has a local copy when it's a local-storage row OR a remote
  // row whose download worker has populated the cache. The Open and
  // Show-in-Finder items key off this.
  const hasLocalCopy = !isRemote || hasCache;

  const handleOpen = async () => {
    try {
      await cacheOpen(file.id);
    } catch (error) {
      console.error('Failed to open file:', error);
      alert(`Failed to open: ${error}`);
    }
  };

  const handleRevealInFinder = async () => {
    // Local rows use file.path directly; remote-with-cache routes to
    // the cache path. Remote-without-cache hides this item, so the
    // branch below is exhaustive.
    const target = isRemote ? file.local_cache_path : file.path;
    if (!target) return;
    try {
      await revealItemInDir(target);
    } catch (error) {
      console.error('Failed to reveal file:', error);
    }
  };

  const handleCopyPath = async () => {
    try {
      await navigator.clipboard.writeText(file.path);
    } catch (error) {
      console.error('Failed to copy path:', error);
    }
  };

  const handleClearCache = async () => {
    const ok = window.confirm(
      'Delete the local cache copy of this file?\n\n' +
        'The remote copy stays on Baidu Pan; you can re-download anytime.'
    );
    if (!ok) return;
    try {
      await cacheClear(file.id);
      // Patch the row locally so the cache badge updates without a refetch.
      patchFile(file.id, { local_cache_path: null });
    } catch (error) {
      console.error('Failed to clear cache:', error);
      alert(`Failed to clear cache: ${error}`);
    }
  };

  return (
    <DropdownMenu>
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
        {hasLocalCopy && (
          <DropdownMenuItem onClick={handleOpen}>
            <Play className="h-4 w-4 mr-2" />
            Open
          </DropdownMenuItem>
        )}
        <DropdownMenuItem onClick={() => onEdit(file)}>
          <Pencil className="h-4 w-4 mr-2" />
          Edit
        </DropdownMenuItem>
        <DropdownMenuItem
          onClick={() => onDelete(file)}
          className="text-destructive focus:text-destructive"
        >
          <Trash2 className="h-4 w-4 mr-2" />
          Delete
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        {hasLocalCopy && (
          <DropdownMenuItem onClick={handleRevealInFinder}>
            <FolderOpen className="h-4 w-4 mr-2" />
            Show in Finder
          </DropdownMenuItem>
        )}
        <DropdownMenuItem onClick={handleCopyPath}>
          <Copy className="h-4 w-4 mr-2" />
          Copy Path
        </DropdownMenuItem>
        {isRemote && hasCache && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={handleClearCache}>
              <XCircle className="h-4 w-4 mr-2" />
              Clear cache
            </DropdownMenuItem>
          </>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
