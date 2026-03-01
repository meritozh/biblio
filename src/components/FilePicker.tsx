import { open } from '@tauri-apps/plugin-dialog';
import { Button } from '@/components/ui/button';
import { FolderOpen } from 'lucide-react';

interface FilePickerProps {
  onFilesSelected: (paths: string[]) => void;
  multiple?: boolean;
}

export function FilePicker({ onFilesSelected, multiple = true }: FilePickerProps) {
  const handleOpenDialog = async () => {
    try {
      const selected = await open({
        multiple,
        directory: false,
        title: 'Select files to add to library',
      });

      if (!selected) {
        return;
      }

      if (typeof selected === 'string') {
        onFilesSelected([selected]);
        return;
      }

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

  return (
    <Button onClick={handleOpenDialog} className="gap-2" aria-label="Add files to library">
      <FolderOpen className="h-4 w-4" aria-hidden="true" />
      Add Files
    </Button>
  );
}
