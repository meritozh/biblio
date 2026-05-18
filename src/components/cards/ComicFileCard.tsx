import { memo, useEffect, useState } from 'react';
import { BookOpen } from 'lucide-react';
import { FileContextMenu } from '@/components/FileContextMenu';
import { coverGet } from '@/lib/tauri';
import { schemaForCategoryId } from '@/lib/categorySchema';
import { useAppState } from '@/stores/appStore';
import { useFile } from '@/stores/fileStore';
import { CARD_HEIGHT, CARD_WIDTH } from './constants';
import { CardField } from './CardField';
import { CardStatus } from './CardStatus';
import type { FileCardProps } from './types';

/** Lazy-fetches stored cover art; falls back to a book icon while
 *  loading or when no cover exists. */
function ComicCover({ fileId }: { fileId: number }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    coverGet(fileId)
      .then(({ data, mime_type }) => setSrc(`data:${mime_type};base64,${data}`))
      .catch(() => { });
  }, [fileId]);
  return src ? (
    <img src={src} alt="Cover" className="h-full w-full object-cover" />
  ) : (
    <BookOpen className="h-8 w-8 text-muted-foreground/40" />
  );
}

/** Comic / manga card: uses real cover artwork via `coverGet`. Per-row
 *  component subscribed to its own entry via `useFile(id)`, wrapped in
 *  `memo` so single-row patches in the store re-render only this card. */
export const ComicFileCard = memo(function ComicFileCard({
  id,
  isSelected,
  isUploading,
  blocked,
  selectionMode,
  onCardClick,
  onToggleSelect,
  onEdit,
  onDelete,
}: FileCardProps) {
  const file = useFile(id);
  const categories = useAppState((s) => s.categories);
  // Brief absence is possible right after `removeFile(id)`: byId loses the
  // row in the same setState that drops it from the view's ids, but a stale
  // render frame can still ask for it. Render nothing instead of crashing.
  if (!file) return null;
  const schema = schemaForCategoryId(file.category_id, categories);

  return (
    <div
      className="relative group"
      style={{ width: CARD_WIDTH, height: CARD_HEIGHT }}
    >
      {selectionMode && (
        <div className="absolute top-2 left-2 z-10">
          <label className="flex items-center justify-center h-6 w-6 rounded-md bg-background/90 backdrop-blur-sm border border-border/40 shadow-sm cursor-pointer hover:bg-background transition-colors">
            <input
              type="checkbox"
              checked={blocked ? false : isSelected}
              onChange={() => !blocked && onToggleSelect(id)}
              onClick={(e) => e.stopPropagation()}
              disabled={blocked}
              className="h-3 w-3 rounded border-border accent-primary disabled:opacity-30 cursor-pointer disabled:cursor-not-allowed"
              aria-label={`Select ${file.display_name}`}
            />
          </label>
        </div>
      )}
      <button
        type="button"
        onClick={() => onCardClick(file)}
        className={`w-full h-full flex flex-col gap-2 text-left rounded-lg p-2 transition-colors focus:outline-none focus:ring-2 focus:ring-ring ${isSelected
          ? 'bg-primary/10 ring-1 ring-primary/40'
          : 'hover:bg-muted/50'
          }`}
        aria-label={
          selectionMode
            ? `Toggle selection of ${file.display_name}`
            : `View ${file.display_name}`
        }
        aria-pressed={selectionMode ? isSelected : undefined}
      >
        <div className="relative aspect-2/3 w-full rounded-md overflow-hidden bg-secondary/40 border flex items-center justify-center">
          <ComicCover fileId={file.id} />
          <div className="absolute bottom-1.5 left-1.5">
            <CardStatus
              storageKind={file.storage_kind}
              isUploading={isUploading}
              hasLocalCache={!!file.local_cache_path}
            />
          </div>
        </div>
        <div className="space-y-0.5 min-w-0 px-0.5">
          <p
            className="text-sm font-medium leading-tight truncate"
            title={file.display_name}
          >
            {file.display_name}
          </p>
          {schema.cardFields.map((field) => (
            <CardField key={field} field={field} file={file} />
          ))}
        </div>
      </button>
      {!selectionMode && onEdit && onDelete && (
        <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity z-10">
          <FileContextMenu file={file} onEdit={onEdit} onDelete={onDelete} />
        </div>
      )}
    </div>
  );
});
