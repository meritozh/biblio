import { schemaForCategoryId } from '@/lib/categorySchema';
import { useAppState } from '@/stores/appStore';
import type { FileEntry } from '@/types';
import { CARD_HEIGHT, CARD_WIDTH } from './cards/constants';
import { ComicFileCard } from './cards/ComicFileCard';
import { NovelFileCard } from './cards/NovelFileCard';

export const LUCKY_CARD_COUNT = 3;
export const LUCKY_CARD_GAP = 16;
export const LUCKY_DIALOG_HORIZONTAL_PADDING = 24 * 2;
export const LUCKY_DIALOG_WIDTH =
  CARD_WIDTH * LUCKY_CARD_COUNT +
  LUCKY_CARD_GAP * (LUCKY_CARD_COUNT - 1) +
  LUCKY_DIALOG_HORIZONTAL_PADDING;

interface LuckyFileCardsProps {
  files: ReadonlyArray<FileEntry>;
  onFileClick: (file: FileEntry) => void;
  onFileEdit?: (file: FileEntry) => void;
  onFileDelete?: (file: FileEntry) => void;
  remoteEnabled?: boolean;
}

export function LuckyFileCards({
  files,
  onFileClick,
  onFileEdit,
  onFileDelete,
  remoteEnabled,
}: LuckyFileCardsProps) {
  const categories = useAppState((s) => s.categories);

  return (
    <div
      data-testid="lucky-file-cards"
      className="flex flex-row gap-4 overflow-x-auto pb-1"
      style={{ minHeight: CARD_HEIGHT }}
    >
      {files.map((file) => {
        const schema = schemaForCategoryId(file.category_id, categories);
        const Card = schema.slug === 'novel' ? NovelFileCard : ComicFileCard;

        return (
          <div key={file.id} className="shrink-0">
            <Card
              id={file.id}
              isSelected={false}
              isUploading={false}
              blocked={false}
              selectionMode={false}
              onCardClick={onFileClick}
              onToggleSelect={() => {}}
              onEdit={onFileEdit}
              onDelete={onFileDelete}
              remoteEnabled={remoteEnabled}
            />
          </div>
        );
      })}
    </div>
  );
}
