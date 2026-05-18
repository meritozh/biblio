import { useEffect, useState, type RefObject } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Loader2 } from 'lucide-react';
import { schemaForCategoryId } from '@/lib/categorySchema';
import { useAppState } from '@/stores/appStore';
import type { ComicCollection, ComicViewMode, FileEntry } from '@/types';
import { CARD_HEIGHT, CARD_WIDTH } from './cards/constants';
import { CollectionCard } from './cards/CollectionCard';
import { ComicFileCard } from './cards/ComicFileCard';
import { NovelFileCard } from './cards/NovelFileCard';

const GRID_GAP = 16;
const GRID_PAD = 4;
const OVERSCAN = 4;
const LOAD_MORE_THRESHOLD = 5;
const DEBOUNCE_MS = 150;

interface FileListContentProps {
  /** Owned by the orchestrator so it can scroll-reset on filter changes
   *  and stash/restore on collection drill-in. */
  scrollContainerRef: RefObject<HTMLDivElement | null>;
  visibleEntries: FileEntry[];
  /** Drives empty-state copy: distinguishes "library is empty" from
   *  "filters hid everything". */
  hasImportableEntries: boolean;
  showCollections: boolean;
  collections?: ComicCollection[];
  viewMode: ComicViewMode;
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
  onOpenCollection?: (c: ComicCollection) => void;
}

/** Virtualized grid container. Renders either file cards (ComicFileCard /
 *  NovelFileCard, dispatched by category schema) or collection cards,
 *  depending on `showCollections`. Owns its own container-width
 *  ResizeObserver and load-more trigger; defers scroll-reset to the
 *  orchestrator via `scrollContainerRef`. */
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
}: FileListContentProps) {
  const [containerWidth, setContainerWidth] = useState(0);
  const categories = useAppState((s) => s.categories);

  const colCount = Math.max(
    1,
    Math.floor((containerWidth - GRID_PAD * 2 + GRID_GAP) / (CARD_WIDTH + GRID_GAP))
  );
  // The virtualizer renders whichever dataset is currently active. Switching
  // between file rows and collection rows reuses the same scroll container
  // and overscan; only the row-count input changes.
  const activeCount = showCollections
    ? (collections?.length ?? 0)
    : visibleEntries.length;
  const gridRowCount = Math.ceil(activeCount / colCount);

  const virtualizer = useVirtualizer({
    count: gridRowCount,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: () => CARD_HEIGHT + GRID_GAP,
    overscan: OVERSCAN,
  });
  const virtualItems = virtualizer.getVirtualItems();
  const totalVirtualSize = virtualizer.getTotalSize();

  // Trigger backend load-more when the last virtual row is near the end.
  // Compare against the *unfiltered* loaded count so an active filter pill
  // shrinking the view doesn't fake a "we need more rows" signal.
  // Suppressed while showing collection cards — the backend returns the
  // full collection set in one call, so there's no more-to-load signal.
  useEffect(() => {
    if (showCollections) return;
    const lastItem = virtualItems[virtualItems.length - 1];
    if (!lastItem || loadingMore || loadedCount >= (total ?? 0)) return;
    if (lastItem.index >= gridRowCount - 1 - LOAD_MORE_THRESHOLD) {
      onLoadMore?.();
    }
  }, [virtualItems, gridRowCount, loadedCount, total, loadingMore, onLoadMore, showCollections]);

  // ResizeObserver feeds containerWidth, which drives column-count math.
  useEffect(() => {
    const el = scrollContainerRef.current;
    if (!el) return;
    let timeoutId: number | null = null;
    const recompute = () => setContainerWidth(el.clientWidth);
    recompute();
    const observer = new ResizeObserver(() => {
      if (timeoutId !== null) window.clearTimeout(timeoutId);
      timeoutId = window.setTimeout(recompute, DEBOUNCE_MS);
    });
    observer.observe(el);
    return () => {
      if (timeoutId !== null) window.clearTimeout(timeoutId);
      observer.disconnect();
    };
  }, [scrollContainerRef]);

  return (
    <div ref={scrollContainerRef} className="flex-1 min-h-0 overflow-auto">
      <div
        style={{ height: totalVirtualSize, position: 'relative', padding: GRID_PAD }}
      >
        {virtualItems.map((virtualRow) => {
          const startIdx = virtualRow.index * colCount;
          const endIdx = Math.min(startIdx + colCount, activeCount);
          return (
            <div
              key={virtualRow.index}
              style={{
                position: 'absolute',
                // Offset by GRID_PAD so the first row's selection ring
                // has breathing room before the scroll clip boundary.
                // Parent's `padding: GRID_PAD` doesn't affect absolute
                // children, so we apply the top inset here.
                top: GRID_PAD,
                left: 0,
                transform: `translateY(${virtualRow.start}px)`,
                width: '100%',
                paddingLeft: GRID_PAD,
                paddingRight: GRID_PAD,
                display: 'grid',
                gridTemplateColumns: `repeat(${colCount}, ${CARD_WIDTH}px)`,
                gap: GRID_GAP,
                justifyContent: 'start',
              }}
            >
              {showCollections
                ? (collections ?? []).slice(startIdx, endIdx).map((c) => (
                  <CollectionCard
                    key={`${c.mode}:${c.key}`}
                    collection={c}
                    onOpen={(col) => onOpenCollection?.(col)}
                  />
                ))
                : visibleEntries.slice(startIdx, endIdx).map((file) => {
                  // Dispatch by category schema: novels get the
                  // procedural cover, everything else gets the real
                  // artwork via `coverGet`. Falls back to comic for
                  // unknown categories so legacy files keep working.
                  const schema = schemaForCategoryId(file.category_id, categories);
                  const Card = schema.slug === 'novel' ? NovelFileCard : ComicFileCard;
                  const blocked = inFlightAnyIds.has(file.id);
                  const isSelected = selectedIds.has(file.id);
                  return (
                    <Card
                      key={file.id}
                      id={file.id}
                      isSelected={isSelected}
                      isUploading={inFlightUploadIds.has(file.id)}
                      blocked={blocked}
                      selectionMode={selectionMode}
                      onCardClick={onCardClick}
                      onToggleSelect={onToggleSelect}
                      onEdit={onFileEdit}
                      onDelete={onFileDelete}
                    />
                  );
                })}
            </div>
          );
        })}
      </div>

      {activeCount === 0 && (
        <div className="flex items-center justify-center h-24 text-sm text-muted-foreground">
          {showCollections
            ? viewMode === 'author'
              ? 'No multi-volume authors in this category yet.'
              : 'No series detected — file names look too unique to group.'
            : !hasImportableEntries
              ? 'No files in library.'
              : 'No files match the active filters.'}
        </div>
      )}

      {loadingMore && !showCollections && (
        <div
          className="flex items-center justify-center py-3 gap-2 text-xs text-muted-foreground font-serif-italic"
          aria-live="polite"
        >
          <Loader2 className="h-3 w-3 animate-spin" aria-hidden="true" />
          {total != null
            ? `loading ${total - visibleEntries.length} more…`
            : 'loading more…'}
        </div>
      )}
    </div>
  );
}
