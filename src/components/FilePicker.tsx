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

interface FilePickerProps {
  onFilesSelected: (paths: string[], folderRoot?: string) => void;
  multiple?: boolean;
  disabled?: boolean;
}

export function FilePicker({ onFilesSelected, multiple = true, disabled = false }: FilePickerProps) {
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
        multiple: false,
        directory: true,
        title: 'Choose a folder to import',
      });

      if (!selected) return;

      const folderPath =
        typeof selected === 'string'
          ? selected
          : selected !== null && typeof selected === 'object'
            ? String((selected as Record<string, unknown>).path ?? '')
            : '';

      if (!folderPath) return;

      setExpanding(true);
      try {
        const files = await listFilesInFolder(folderPath);
        if (files.length === 0) {
          alert('The selected folder is empty.');
          return;
        }
        onFilesSelected(files, folderPath);
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
            <span>Choose folder…</span>
            <span className="text-xs text-muted-foreground">
              All files within, recursively
            </span>
          </div>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
