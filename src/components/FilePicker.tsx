import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Plus, Files, Folder, Loader2 } from 'lucide-react';
import { listFilesInFolder } from '@/lib/tauri';
import type { SchemaSlug } from '@/types';

interface FilePickerProps {
  /** `pathFolderRoots` maps every scanned path back to the folder the user
   *  picked for it. Empty / omitted for plain file picks and drag-drop. */
  onFilesSelected: (
    paths: string[],
    pathFolderRoots?: Record<string, string>
  ) => void;
  /** Schema of the category being imported into. Drives folder-scan
   *  semantics: galgame folders collapse to one unit, comic/novel keep the
   *  image-leaf walk. Defaults to novel on the backend when omitted. */
  schemaSlug?: SchemaSlug;
  multiple?: boolean;
  disabled?: boolean;
}

export function FilePicker({
  onFilesSelected,
  schemaSlug,
  multiple = true,
  disabled = false,
}: FilePickerProps) {
  const [expanding, setExpanding] = useState(false);

  const handlePickFiles = async () => {
    try {
      const selected = await open({
        multiple,
        directory: false,
        title: 'Select files to add to library',
      });

      if (!selected) return;

      const paths: string[] = [];
      const items = Array.isArray(selected) ? selected : [selected];
      for (const item of items) {
        if (typeof item === 'string') {
          paths.push(item);
        } else if (item !== null && typeof item === 'object') {
          const obj = item as Record<string, unknown>;
          if (typeof obj.path === 'string') {
            paths.push(obj.path);
          }
        }
      }
      if (paths.length > 0) {
        onFilesSelected(paths);
      }
    } catch (error) {
      console.error('Failed to open file dialog:', error);
    }
  };

  const handlePickFolder = async () => {
    try {
      const selected = await open({
        multiple: true,
        directory: true,
        title: 'Choose folders to import',
      });

      if (!selected) return;

      const folderPaths: string[] = [];
      const items = Array.isArray(selected) ? selected : [selected];
      for (const item of items) {
        if (typeof item === 'string') {
          folderPaths.push(item);
        } else if (item !== null && typeof item === 'object') {
          const obj = item as Record<string, unknown>;
          if (typeof obj.path === 'string') {
            folderPaths.push(obj.path);
          }
        }
      }

      if (folderPaths.length === 0) return;

      setExpanding(true);
      try {
        // Scan in parallel. Each path returned by `listFilesInFolder`
        // gets attributed back to the folder it came from so the
        // backend can build a per-comic author hint.
        const scans = await Promise.all(
          folderPaths.map(async (root) => ({
            root,
            files: await listFilesInFolder(root, schemaSlug),
          }))
        );

        const allFiles: string[] = [];
        const pathFolderRoots: Record<string, string> = {};
        const emptyFolders: string[] = [];
        for (const { root, files } of scans) {
          if (files.length === 0) {
            emptyFolders.push(root);
            continue;
          }
          for (const file of files) {
            allFiles.push(file);
            pathFolderRoots[file] = root;
          }
        }

        if (allFiles.length === 0) {
          alert(
            emptyFolders.length === 1
              ? 'The selected folder is empty.'
              : `All ${emptyFolders.length} selected folders are empty.`
          );
          return;
        }
        if (emptyFolders.length > 0) {
          console.warn('Skipped empty folders:', emptyFolders);
        }
        onFilesSelected(allFiles, pathFolderRoots);
      } finally {
        setExpanding(false);
      }
    } catch (error) {
      console.error('Failed to open folder dialog:', error);
      alert(`Failed to scan folder: ${String(error)}`);
      setExpanding(false);
    }
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          className="gap-2"
          disabled={disabled || expanding}
          aria-label="Add to library"
        >
          {expanding ? (
            <Loader2 className="h-4 w-4 animate-spin" aria-hidden="true" />
          ) : (
            <Plus className="h-4 w-4" aria-hidden="true" />
          )}
          {expanding ? 'Scanning…' : 'Add'}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56">
        <DropdownMenuItem onSelect={handlePickFiles} className="gap-2">
          <Files className="h-4 w-4" aria-hidden="true" />
          <div className="flex flex-col">
            <span>Choose files…</span>
            <span className="text-xs text-muted-foreground">
              One or more individual files
            </span>
          </div>
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={handlePickFolder} className="gap-2">
          <Folder className="h-4 w-4" aria-hidden="true" />
          <div className="flex flex-col">
            <span>Choose folders…</span>
            <span className="text-xs text-muted-foreground">
              One or more folders, scanned recursively
            </span>
          </div>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
