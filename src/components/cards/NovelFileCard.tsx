import { memo } from 'react';
import { FileContextMenu } from '@/components/FileContextMenu';
import { NovelCover } from '@/components/NovelCover';
import { schemaForCategoryId } from '@/lib/categorySchema';
import { useAppState } from '@/stores/appStore';
import { useFile } from '@/stores/fileStore';
import { MiddleEllipsis } from '@/components/MiddleEllipsis';
import { CARD_HEIGHT, CARD_WIDTH } from './constants';
import { CardField } from './CardField';
import { CardStatus } from './CardStatus';
import { FavoriteToggleButton } from './FavoriteToggleButton';
import type { FileCardProps } from './types';

/** Novel / book card: uses procedural `NovelCover` (no real cover art).
 *  Per-row component subscribed to its own entry via `useFile(id)`,
 *  wrapped in `memo` so single-row patches in the store re-render only
 *  this card. */
export const NovelFileCard = memo(function NovelFileCard({
  id,
  isSelected,
  isUploading,
  blocked,
  selectionMode,
  onCardClick,
  onToggleSelect,
  onEdit,
  onDelete,
  remoteEnabled,
}: FileCardProps) {
  const file = useFile(id);
  const categories = useAppState((s) => s.categories);
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
          <NovelCover
            tags={file.tags}
            fileId={file.id}
            displayName={file.display_name}
            progress={file.progress}
          />
          <div className="absolute bottom-1.5 left-1.5">
            <CardStatus
              storageKind={file.storage_kind}
              isUploading={isUploading}
              hasLocalCache={!!file.local_cache_path}
            />
          </div>
        </div>
        <div className="space-y-0.5 min-w-0 px-0.5">
          <MiddleEllipsis
            text={file.display_name}
            className="text-sm font-medium leading-tight"
          />
          {schema.cardFields.map((field) => (
            <CardField key={field} field={field} file={file} />
          ))}
        </div>
      </button>
      {!selectionMode && (
        <div
          className={`absolute top-3.5 left-3.5 z-10 transition-opacity ${
            file.is_favorite
              ? 'opacity-100'
              : 'opacity-0 group-hover:opacity-100 focus-within:opacity-100'
          }`}
        >
          <FavoriteToggleButton file={file} />
        </div>
      )}
      {!selectionMode && onEdit && onDelete && (
        <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity z-10">
          <FileContextMenu
            file={file}
            onEdit={onEdit}
            onDelete={onDelete}
            remoteEnabled={remoteEnabled}
          />
        </div>
      )}
    </div>
  );
});
