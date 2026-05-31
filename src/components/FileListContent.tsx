import { Loader2 } from 'lucide-react';
import { type RefObject } from 'react';
import { schemaForCategoryId } from '@/lib/categorySchema';
import { useAppState } from '@/stores/appStore';
import type { Collection, ViewMode, FileEntry } from '@/types';
import { CARD_HEIGHT, CARD_WIDTH } from './cards/constants';
import { CollectionCard } from './cards/CollectionCard';
import { ComicFileCard } from './cards/ComicFileCard';
import { NovelFileCard } from './cards/NovelFileCard';
import { VirtualGrid } from './VirtualGrid';

const GRID_GAP = 16;
const GRID_PAD = 4;
const OVERSCAN = 4;
const LOAD_MORE_THRESHOLD = 5;

interface FileListContentProps {
  /** Owned by the orchestrator so it can scroll-reset on filter changes
   *  and stash/restore on collection drill-in. */
  scrollContainerRef: RefObject<HTMLDivElement | null>;
  visibleEntries: FileEntry[];
  /** Drives empty-state copy: distinguishes "library is empty" from
   *  "filters hid everything". */
  hasImportableEntries: boolean;
  showCollections: boolean;
  collections?: Collection[];
  viewMode: ViewMode;
  // Pagination
  total?: number;
  loadingMore: boolean;
  onLoadMore?: () => void;
  /** Raw `ids` length — used by the load-more cutoff so client-side
   *  filters shrinking the view don't fake a "need more rows" signal. */
  loadedCount: number;
  // Selection / card props
  selectionMode: boolean;
  selectedIds: Set<number>;
  inFlightAnyIds: Set<number>;
  inFlightUploadIds: Set<number>;
  onCardClick: (file: FileEntry) => void;
  onToggleSelect: (id: number) => void;
  onFileEdit?: (file: FileEntry) => void;
  onFileDelete?: (file: FileEntry) => void;
  onOpenCollection?: (c: Collection) => void;
  /** Whether cloud storage is configured. Forwarded to each card's
   *  context menu to gate Upload / Download. */
  remoteEnabled?: boolean;
}

/** Virtualized grid container. Renders either file cards (ComicFileCard /
 *  NovelFileCard, dispatched by category schema) or collection cards,
 *  depending on `showCollections`. Layout is delegated to the shared
 *  `<VirtualGrid>` so column-count math, ResizeObserver, load-more, and
 *  the diff-animation hook live in one place. */
export function FileListContent({
  scrollContainerRef,
  visibleEntries,
  hasImportableEntries,
  showCollections,
  collections,
  viewMode,
  total,
  loadingMore,
  onLoadMore,
  loadedCount,
  selectionMode,
  selectedIds,
  inFlightAnyIds,
  inFlightUploadIds,
  onCardClick,
  onToggleSelect,
  onFileEdit,
  onFileDelete,
  onOpenCollection,
  remoteEnabled,
}: FileListContentProps) {
  const categories = useAppState((s) => s.categories);

  const emptyState = (
    <div className="flex items-center justify-center h-24 text-sm text-muted-foreground">
      {showCollections
        ? viewMode === 'author'
          ? 'No multi-volume authors in this category yet.'
          : 'No series detected — file names look too unique to group.'
        : !hasImportableEntries
          ? 'No files in library.'
          : 'No files match the active filters.'}
    </div>
  );

  const loadingMoreSlot =
    loadingMore && !showCollections ? (
      <div
        className="flex items-center justify-center py-3 gap-2 text-xs text-muted-foreground font-serif-italic"
        aria-live="polite"
      >
        <Loader2 className="h-3 w-3 animate-spin" aria-hidden="true" />
        {total != null
          ? `loading ${total - visibleEntries.length} more…`
          : 'loading more…'}
      </div>
    ) : null;

  // Two `<VirtualGrid>` instantiations selected by showCollections — keeps
  // the renderItem callback strongly typed per mode without an
  // any-cast on the items array. Both branches use VirtualGrid (no
  // bail-out for the empty case) so the scroll container's DOM identity
  // stays stable across empty ↔ non-empty transitions, which preserves
  // scrollTop when the user filters into and back out of an empty view.
  if (showCollections) {
    return (
      <VirtualGrid<Collection>
        items={collections ?? []}
        getKey={(c) => `${c.mode}:${c.key}`}
        cardWidth={CARD_WIDTH}
        cardHeight={CARD_HEIGHT}
        gap={GRID_GAP}
        padding={GRID_PAD}
        overscan={OVERSCAN}
        scrollContainerRef={scrollContainerRef}
        emptyState={emptyState}
        // No load-more for collections — backend returns the full set
        // in one call.
        renderItem={(c) => (
          <CollectionCard collection={c} onOpen={(col) => onOpenCollection?.(col)} />
        )}
      />
    );
  }

  return (
    <VirtualGrid<FileEntry>
      items={visibleEntries}
      getKey={(f) => f.id}
      cardWidth={CARD_WIDTH}
      cardHeight={CARD_HEIGHT}
      gap={GRID_GAP}
      padding={GRID_PAD}
      overscan={OVERSCAN}
      scrollContainerRef={scrollContainerRef}
      onLoadMore={onLoadMore}
      hasMore={loadedCount < (total ?? 0)}
      loadMoreThreshold={LOAD_MORE_THRESHOLD}
      loadingMoreSlot={loadingMoreSlot}
      emptyState={emptyState}
      renderItem={(file) => {
        // Dispatch by category schema: novels get the procedural cover,
        // everything else gets the real artwork via coverGet. Falls back
        // to comic for unknown categories so legacy files keep working.
        const schema = schemaForCategoryId(file.category_id, categories);
        const Card = schema.slug === 'novel' ? NovelFileCard : ComicFileCard;
        return (
          <Card
            id={file.id}
            isSelected={selectedIds.has(file.id)}
            isUploading={inFlightUploadIds.has(file.id)}
            blocked={inFlightAnyIds.has(file.id)}
            selectionMode={selectionMode}
            onCardClick={onCardClick}
            onToggleSelect={onToggleSelect}
            onEdit={onFileEdit}
            onDelete={onFileDelete}
            remoteEnabled={remoteEnabled}
          />
        );
      }}
    />
  );
}
