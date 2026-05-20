import {
  useCallback,
  useEffect,
  useState,
  type ReactNode,
  type RefObject,
} from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useDiffAnimation } from './useDiffAnimation';

interface VirtualListProps<T> {
  /** Stable data. Mutations are applied as local diffs by the caller; the
   *  wrapper compares against the previous render's keys to decide
   *  whether to animate. */
  items: ReadonlyArray<T>;
  getKey: (item: T) => string | number;
  renderItem: (item: T, index: number) => ReactNode;
  /** Number for fixed-row layouts; function for variable-row when paired
   *  with `measureElement`. */
  estimateSize: number | ((index: number) => number);
  /** Set true to enable dynamic-height measurement. The wrapper attaches
   *  `virtualizer.measureElement` to each rendered row so real DOM
   *  heights propagate (used by cleanup's similar-names cards). */
  measureElement?: boolean;
  overscan?: number;
  /** Called when the last virtual row is within `loadMoreThreshold` of
   *  the end of the loaded slice. Caller appends rows; wrapper just
   *  fires the trigger. */
  onLoadMore?: () => void;
  hasMore?: boolean;
  loadMoreThreshold?: number;
  /** Skip animations on renders where the prev-vs-current keys diff
   *  exceeds this count. Default 20 — single mutations stay under,
   *  bulk operations skip. */
  animationDiffThreshold?: number;
  /** Optional parent-held ref for the scroll container, used by callers
   *  with their own scroll-reset effects. The wrapper writes to it on
   *  mount/unmount and still maintains its own internal state copy. */
  scrollContainerRef?: RefObject<HTMLDivElement | null>;
  /** Bottom-of-container slot, rendered after the virtual list — used
   *  for "loading more…" indicators or footer copy. */
  loadingMoreSlot?: ReactNode;
  /** Rendered inside the scroll container when `items.length === 0`.
   *  Use this instead of conditionally rendering the wrapper itself so
   *  the scroll container's DOM identity stays stable across empty ↔
   *  non-empty transitions. */
  emptyState?: ReactNode;
  /** Outer scroll container className. Defaults to plain `overflow-auto`
   *  so the wrapper fills its parent; pass a max-h / flex-1 class for
   *  bounded contexts. */
  className?: string;
}

const DEFAULT_OVERSCAN = 4;
const DEFAULT_LOAD_MORE_THRESHOLD = 5;
const DEFAULT_ANIM_THRESHOLD = 20;

/** 1D virtualized list with key-based diff detection and CSS-driven
 *  enter/move animations. The animation cost is paid only when the diff
 *  is small enough to be useful; bulk renders snap. */
export function VirtualList<T>({
  items,
  getKey,
  renderItem,
  estimateSize,
  measureElement = false,
  overscan = DEFAULT_OVERSCAN,
  onLoadMore,
  hasMore = false,
  loadMoreThreshold = DEFAULT_LOAD_MORE_THRESHOLD,
  animationDiffThreshold = DEFAULT_ANIM_THRESHOLD,
  scrollContainerRef,
  loadingMoreSlot,
  emptyState,
  className = 'overflow-auto',
}: VirtualListProps<T>) {
  // State-backed scroll element + optional parent ref bridge. State is
  // mandatory so that when the wrapper mounts inside a Portal (popover,
  // dialog) the virtualizer's first measure sees a real element on the
  // second render. Parent ref is convenience for scroll-reset effects.
  const [scrollEl, setScrollEl] = useState<HTMLDivElement | null>(null);
  const refCallback = useCallback(
    (el: HTMLDivElement | null) => {
      setScrollEl(el);
      if (scrollContainerRef) {
        scrollContainerRef.current = el;
      }
    },
    [scrollContainerRef]
  );

  const skipAnim = useDiffAnimation(items, getKey, animationDiffThreshold);

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollEl,
    estimateSize:
      typeof estimateSize === 'number' ? () => estimateSize : estimateSize,
    overscan,
    measureElement: measureElement
      ? (el) => el.getBoundingClientRect().height
      : undefined,
    getItemKey: (i) => {
      const item = items[i];
      return item != null ? getKey(item) : i;
    },
  });
  const virtualItems = virtualizer.getVirtualItems();

  useEffect(() => {
    if (!onLoadMore || !hasMore) return;
    const last = virtualItems[virtualItems.length - 1];
    if (!last) return;
    if (last.index >= items.length - 1 - loadMoreThreshold) {
      onLoadMore();
    }
  }, [virtualItems, items.length, hasMore, onLoadMore, loadMoreThreshold]);

  return (
    <div
      ref={refCallback}
      className={className}
      data-skip-anim={skipAnim ? '' : undefined}
    >
      <div
        style={{ height: virtualizer.getTotalSize(), position: 'relative' }}
      >
        {virtualItems.map((vRow) => {
          const item = items[vRow.index];
          if (!item) return null;
          return (
            <div
              key={vRow.key}
              data-virtual-row=""
              data-index={vRow.index}
              ref={measureElement ? virtualizer.measureElement : undefined}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                right: 0,
                transform: `translateY(${vRow.start}px)`,
                ...(measureElement ? {} : { height: vRow.size }),
              }}
            >
              {renderItem(item, vRow.index)}
            </div>
          );
        })}
      </div>
      {items.length === 0 && emptyState}
      {loadingMoreSlot}
    </div>
  );
}
