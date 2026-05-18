import type { CardFieldKey } from '@/lib/categorySchema';
import type { FileEntry } from '@/types';

interface CardFieldProps {
  field: CardFieldKey;
  file: FileEntry;
}

/** Render one card body field. Returns null when the row has no value
 *  for the field, so the card doesn't sprout empty rows. */
export function CardField({ field, file }: CardFieldProps) {
  switch (field) {
    case 'authors':
      if (!file.authors || file.authors.length === 0) return null;
      return (
        <p className="text-xs text-muted-foreground line-clamp-1">
          {file.authors.map((a) => a.name).join(', ')}
        </p>
      );
    case 'progress':
      if (!file.progress) return null;
      return (
        <p className="text-[11px] text-muted-foreground/80 line-clamp-1 font-serif-italic">
          {file.progress}
        </p>
      );
    case 'tags':
      if (!file.tags || file.tags.length === 0) return null;
      // Compact inline tag chips. Cap at 3 to keep the card height stable.
      return (
        <p className="text-[11px] text-muted-foreground line-clamp-1">
          {file.tags.slice(0, 3).map((t) => `#${t.name}`).join(' ')}
        </p>
      );
  }
}
