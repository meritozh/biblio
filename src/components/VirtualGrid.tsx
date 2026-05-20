import {
  useCallback,
  useEffect,
  useState,
  type ReactNode,
  type RefObject,
} from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useDiffAnimation } from './useDiffAnimation';

interface VirtualGridProps<T> {
  items: ReadonlyArray<T>;
  getKey: (item: T) => string | number;
  renderItem: (item: T) => ReactNode;
  /** Fixed card dimensions. Column count is computed from
   *  containerWidth so the grid reflows on resize. */
  cardWidth: number;
  cardHeight: number;
  gap?: number;
  padding?: number;
  overscan?: number;
  onLoadMore?: () => void;
  hasMore?: boolean;
  loadMoreThreshold?: number;
  animationDiffThreshold?: number;
  scrollContainerRef?: RefObject<HTMLDivElement | null>;
  /** Bottom-of-container slot for "loading X more…" / footer copy. */
  loadingMoreSlot?: ReactNode;
  /** Rendered inside the scroll container when `items.length === 0`.
   *  Letting the caller surface "no matches" copy here (instead of
   *  short-circuiting around the wrapper) keeps the scroll container
   *  DOM identity stable across empty ↔ non-empty transitions —
   *  scrollTop is preserved without an explicit save/restore. */
  emptyState?: ReactNode;
  className?: string;
  /** Debounce window for ResizeObserver-driven column-count recompute. */
  resizeDebounceMs?: number;
}

const DEFAULT_GAP = 16;
const DEFAULT_PADDING = 4;
const DEFAULT_OVERSCAN = 4;
const DEFAULT_LOAD_MORE_THRESHOLD = 5;
const DEFAULT_ANIM_THRESHOLD = 20;
const DEFAULT_RESIZE_DEBOUNCE_MS = 150;

/** 2D card-grid virtualization. Each virtual row contains a CSS-grid of
 *  N cards where N is derived from the container width. Cards within a
 *  row don't carry per-card transforms (they're laid out by the inner
 *  grid), so individual card movement isn't animated; what is animated
 *  is the row-level position when total items change. The diff hook
 *  suppresses transitions on bulk renders. */
export function VirtualGrid<T>({
  items,
  getKey,
  renderItem,
  cardWidth,
  cardHeight,
  gap = DEFAULT_GAP,
  padding = DEFAULT_PADDING,
  overscan = DEFAULT_OVERSCAN,
  onLoadMore,
  hasMore = false,
  loadMoreThreshold = DEFAULT_LOAD_MORE_THRESHOLD,
  animationDiffThreshold = DEFAULT_ANIM_THRESHOLD,
  scrollContainerRef,
  loadingMoreSlot,
  emptyState,
  className = 'flex-1 min-h-0 overflow-auto',
  resizeDebounceMs = DEFAULT_RESIZE_DEBOUNCE_MS,
}: VirtualGridProps<T>) {
  const [scrollEl, setScrollEl] = useState<HTMLDivElement | null>(null);
  const [containerWidth, setContainerWidth] = useState(0);

  const refCallback = useCallback(
    (el: HTMLDivElement | null) => {
      setScrollEl(el);
      if (scrollContainerRef) {
        scrollContainerRef.current = el;
      }
    },
    [scrollContainerRef]
  );

  // ResizeObserver → containerWidth. Debounced so resize drags don't
  // recompute column count on every frame.
  useEffect(() => {
    if (!scrollEl) return;
    let timeoutId: number | null = null;
    const recompute = () => setContainerWidth(scrollEl.clientWidth);
    recompute();
    const observer = new ResizeObserver(() => {
      if (timeoutId !== null) window.clearTimeout(timeoutId);
      timeoutId = window.setTimeout(recompute, resizeDebounceMs);
    });
    observer.observe(scrollEl);
    return () => {
      if (timeoutId !== null) window.clearTimeout(timeoutId);
      observer.disconnect();
    };
  }, [scrollEl, resizeDebounceMs]);

  const colCount = Math.max(
    1,
    Math.floor((containerWidth - padding * 2 + gap) / (cardWidth + gap))
  );
  const rowCount = Math.ceil(items.length / colCount);

  const skipAnim = useDiffAnimation(items, getKey, animationDiffThreshold);

  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => scrollEl,
    estimateSize: () => cardHeight + gap,
    overscan,
  });
  const virtualItems = virtualizer.getVirtualItems();

  useEffect(() => {
    if (!onLoadMore || !hasMore) return;
    const last = virtualItems[virtualItems.length - 1];
    if (!last) return;
    if (last.index >= rowCount - 1 - loadMoreThreshold) {
      onLoadMore();
    }
  }, [virtualItems, rowCount, hasMore, onLoadMore, loadMoreThreshold]);

  return (
    <div
      ref={refCallback}
      className={className}
      data-skip-anim={skipAnim ? '' : undefined}
    >
      <div
        style={{
          height: virtualizer.getTotalSize(),
          position: 'relative',
          padding,
        }}
      >
        {virtualItems.map((vRow) => {
          const startIdx = vRow.index * colCount;
          const endIdx = Math.min(startIdx + colCount, items.length);
          return (
            <div
              key={vRow.index}
              data-virtual-row=""
              style={{
                position: 'absolute',
                // Top offset matches the outer padding so the first
                // row's selection ring etc. has breathing room.
                top: padding,
                left: 0,
                transform: `translateY(${vRow.start}px)`,
                width: '100%',
                paddingLeft: padding,
                paddingRight: padding,
                display: 'grid',
                gridTemplateColumns: `repeat(${colCount}, ${cardWidth}px)`,
                gap,
                justifyContent: 'start',
              }}
            >
              {items.slice(startIdx, endIdx).map((item) => (
                <div key={getKey(item)} data-virtual-card="">
                  {renderItem(item)}
                </div>
              ))}
            </div>
          );
        })}
      </div>
      {items.length === 0 && emptyState}
      {loadingMoreSlot}
    </div>
  );
}
