import { useState, type MouseEvent } from 'react';
import { Loader2, Star } from 'lucide-react';
import { fileSetFavorite } from '@/lib/tauri';
import { patchFile } from '@/stores/fileStore';
import { cn } from '@/lib/utils';
import type { FileEntry } from '@/types';

interface FavoriteToggleButtonProps {
  file: FileEntry;
}

export function FavoriteToggleButton({ file }: FavoriteToggleButtonProps) {
  const [saving, setSaving] = useState(false);
  const next = !file.is_favorite;
  const label = file.is_favorite ? 'Remove from favorites' : 'Add to favorites';

  const handleClick = async (event: MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    if (saving) return;
    setSaving(true);
    try {
      await fileSetFavorite(file.id, next);
      patchFile(file.id, { is_favorite: next });
    } catch (error) {
      console.error('Failed to update favorite:', error);
      alert(`Failed to update favorite: ${error}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <button
      type="button"
      onClick={handleClick}
      disabled={saving}
      title={label}
      aria-label={`${label}: ${file.display_name}`}
      aria-pressed={file.is_favorite}
      className={cn(
        'flex h-6 w-6 items-center justify-center rounded-md border border-border/40 bg-background/90 shadow-sm backdrop-blur-sm transition-colors',
        'hover:bg-background focus:outline-none focus:ring-2 focus:ring-ring disabled:cursor-wait disabled:opacity-70',
        file.is_favorite ? 'text-amber-600 dark:text-amber-400' : 'text-muted-foreground'
      )}
    >
      {saving ? (
        <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden="true" />
      ) : (
        <Star
          className="h-3.5 w-3.5"
          fill={file.is_favorite ? 'currentColor' : 'none'}
          aria-hidden="true"
        />
      )}
    </button>
  );
}
