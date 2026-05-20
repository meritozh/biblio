import {
  useCallback,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Check, Loader2, Plus } from 'lucide-react';
import { Input } from '@/components/ui/input';

export interface PickerItem {
  id: number;
  name: string;
  /** Optional swatch (tags carry color). Authors leave it undefined. */
  color?: string | null;
}

export interface PickerPage {
  items: PickerItem[];
  /** Total rows matching the active query, used to know when to stop
   *  asking for more. */
  total: number;
}

interface PaginatedPickerProps {
  /** Already-selected ids — used to render checkmarks. The parent owns
   *  the selection set; this component never mutates state directly. */
  selectedIds: ReadonlyArray<number>;
  /** "multi" toggles on each click; "single" calls `onSelect` then expects
   *  the parent to close the popover. */
  mode: 'single' | 'multi';
  /** Paginated fetcher. `query` is whatever the search input currently
   *  holds; empty means "no filter". `offset` walks the result set. */
  fetcher: (params: {
    query: string;
    offset: number;
    limit: number;
  }) => Promise<PickerPage>;
  /** Multi-mode: called with the next full id list on each toggle. */
  onToggle?: (nextIds: number[]) => void;
  /** Single-mode: called with the picked id and the full item (so the
   *  caller can render a name preview without re-fetching). */
  onSelect?: (id: number, item: PickerItem) => void;
  /** Optional inline-create row. Shown only when the query has no exact
   *  match. Result is auto-toggled into the selection (multi) or set as
   *  the picked value (single). */
  onCreate?: (name: string) => Promise<PickerItem>;
  /** Placeholder for the search input. */
  searchPlaceholder?: string;
  /** Copy when the page is empty (no items in DB) vs. when the query
   *  has zero hits. */
  emptyLabel?: string;
  noMatchLabel?: string;
}

const PAGE_SIZE = 200;
const ROW_HEIGHT = 36;
const OVERSCAN = 8;
const LOAD_MORE_THRESHOLD = 12;
const DEBOUNCE_MS = 200;
const LIST_HEIGHT = 256;

/** Virtualized, searchable, paginated picker body. Designed to live
 *  inside a `PopoverContent`. The parent owns popover open state, the
 *  selected-chips display, and the trigger button — this component
 *  only renders the search input + scrolling list. */
export function PaginatedPicker({
  selectedIds,
  mode,
  fetcher,
  onToggle,
  onSelect,
  onCreate,
  searchPlaceholder = 'Search…',
  emptyLabel = 'Nothing here yet',
  noMatchLabel = 'No matches',
}: PaginatedPickerProps) {
  const [rawQuery, setRawQuery] = useState('');
  const [query, setQuery] = useState('');
  const [items, setItems] = useState<PickerItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [creating, setCreating] = useState(false);

  // Debounce the search input. The raw value drives the controlled
  // input; `query` (debounced) drives the fetch.
  useEffect(() => {
    const id = window.setTimeout(() => setQuery(rawQuery.trim()), DEBOUNCE_MS);
    return () => window.clearTimeout(id);
  }, [rawQuery]);

  // First page (or refetch on query change). Ignores stale responses
  // via the `cancelled` flag — type-fast users will trigger several
  // overlapping fetches; only the latest one wins.
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    fetcher({ query, offset: 0, limit: PAGE_SIZE })
      .then((page) => {
        if (cancelled) return;
        setItems(page.items);
        setTotal(page.total);
      })
      .catch((err) => {
        if (cancelled) return;
        console.error('Picker fetch failed:', err);
        setItems([]);
        setTotal(0);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [query, fetcher]);

  const selected = useMemo(() => new Set(selectedIds), [selectedIds]);

  const handleToggle = useCallback(
    (item: PickerItem) => {
      if (mode === 'single') {
        onSelect?.(item.id, item);
        return;
      }
      const next = selected.has(item.id)
        ? Array.from(selected).filter((x) => x !== item.id)
        : [...selected, item.id];
      onToggle?.(next);
    },
    [mode, onSelect, onToggle, selected]
  );

  const showCreate =
    !!onCreate &&
    query.length > 0 &&
    !items.some((it) => it.name.toLowerCase() === query.toLowerCase());

  const handleCreate = useCallback(async () => {
    if (!onCreate || query.length === 0) return;
    setCreating(true);
    try {
      const created = await onCreate(query);
      // Prepend the new row so it's visible without waiting for a
      // refetch. The next query-driven refetch (e.g. user clears the
      // input) will pick it up from the canonical source too.
      setItems((prev) => [created, ...prev]);
      setTotal((t) => t + 1);
      if (mode === 'single') {
        onSelect?.(created.id, created);
      } else {
        onToggle?.([...selected, created.id]);
      }
      setRawQuery('');
    } catch (err) {
      console.error('Picker create failed:', err);
    } finally {
      setCreating(false);
    }
  }, [onCreate, query, mode, onSelect, onToggle, selected]);

  // Holds the scroll element in state (not a ref) so that when the
  // Popover's Portal mounts the container, the resulting re-render gives
  // `useVirtualizer` a non-null `getScrollElement` to attach its scroll
  // listener to. A plain `useRef` doesn't work here: the ref's `current`
  // is null at the virtualizer's first effect (Portal child hasn't
  // mounted yet) and TanStack Virtual doesn't re-poll the getter on
  // later renders — so the listener never attaches and the wheel events
  // never update virtual items.
  const [scrollEl, setScrollEl] = useState<HTMLDivElement | null>(null);
  // Virtual list spans `items` only; the create row sits outside the
  // virtualizer so it stays pinned to the bottom regardless of scroll.
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => scrollEl,
    estimateSize: () => ROW_HEIGHT,
    overscan: OVERSCAN,
  });
  const virtualItems = virtualizer.getVirtualItems();

  // Load-more on scroll near the bottom. Same shape as FileListContent.
  useEffect(() => {
    const last = virtualItems[virtualItems.length - 1];
    if (!last || loadingMore || loading) return;
    if (items.length >= total) return;
    if (last.index < items.length - 1 - LOAD_MORE_THRESHOLD) return;
    setLoadingMore(true);
    fetcher({ query, offset: items.length, limit: PAGE_SIZE })
      .then((page) => {
        setItems((prev) => [...prev, ...page.items]);
        setTotal(page.total);
      })
      .catch((err) => {
        console.error('Picker load-more failed:', err);
      })
      .finally(() => setLoadingMore(false));
  }, [virtualItems, items.length, total, loading, loadingMore, fetcher, query]);

  return (
    <div className="w-72">
      <div className="p-3 border-b">
        <Input
          value={rawQuery}
          onChange={(e) => setRawQuery(e.target.value)}
          placeholder={searchPlaceholder}
          className="h-9"
          autoFocus
        />
      </div>
      <div
        ref={setScrollEl}
        className="overflow-auto"
        // `maxHeight` (not fixed `height`) so the scroll area shrinks to
        // fit when items are few or empty — keeps the Create row from
        // floating at the bottom of a 256px blank box when the search
        // has no matches.
        style={{ maxHeight: LIST_HEIGHT }}
        onWheel={(e) => {
          // Picker lives in a Radix Portal so its parent's overflow
          // clip can't reach it. That Portal sits outside Radix
          // Dialog's react-remove-scroll shard though, which swallows
          // wheel events on anything outside the dialog — drive scroll
          // manually so the picker remains scrollable inside a dialog.
          //
          // preventDefault suppresses the browser's own scroll on the
          // same container: without it, contexts NOT inside a dialog
          // (cleanup page's picker) would double-scroll (browser default
          // + our manual scrollTop update). stopPropagation prevents
          // ancestor scroll containers from also moving.
          e.preventDefault();
          e.stopPropagation();
          e.currentTarget.scrollTop += e.deltaY;
        }}
      >
        {loading && items.length === 0 ? (
          <div className="flex items-center justify-center gap-2 py-8 text-sm text-muted-foreground">
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
            Loading…
          </div>
        ) : items.length === 0 ? (
          // Show the empty message regardless of `showCreate`. The
          // Create row still renders below this branch when available;
          // letting both show explains *why* the Create is offered.
          <div className="px-3 py-6 text-center text-sm text-muted-foreground">
            {query ? noMatchLabel : emptyLabel}
          </div>
        ) : (
          <div
            style={{
              height: virtualizer.getTotalSize(),
              position: 'relative',
            }}
          >
            {virtualItems.map((virtualRow) => {
              const item = items[virtualRow.index];
              if (!item) return null;
              const isSelected = selected.has(item.id);
              return (
                <button
                  type="button"
                  key={item.id}
                  onClick={() => handleToggle(item)}
                  className="absolute inset-x-1 flex items-center gap-3 px-2 py-2 text-sm rounded-md hover:bg-accent transition-colors text-left"
                  style={{
                    top: 0,
                    transform: `translateY(${virtualRow.start}px)`,
                    height: ROW_HEIGHT,
                  }}
                >
                  <span
                    className={`w-4 h-4 border rounded flex items-center justify-center shrink-0 transition-colors ${
                      isSelected
                        ? 'bg-primary border-primary text-primary-foreground'
                        : 'border-muted-foreground/30'
                    }`}
                  >
                    {isSelected && <Check className="h-3 w-3" />}
                  </span>
                  {item.color && (
                    <span
                      className="w-3 h-3 rounded-full shrink-0"
                      style={{ backgroundColor: item.color }}
                      aria-hidden="true"
                    />
                  )}
                  <span className="flex-1 truncate">{item.name}</span>
                </button>
              );
            })}
          </div>
        )}
        {loadingMore && (
          <div
            className="flex items-center justify-center gap-2 py-2 text-xs text-muted-foreground font-serif-italic"
            aria-live="polite"
          >
            <Loader2 className="h-3 w-3 animate-spin" />
            loading more…
          </div>
        )}
      </div>
      {showCreate && (
        <button
          type="button"
          onClick={handleCreate}
          disabled={creating}
          className="w-full flex items-center gap-2 px-3 py-2 text-sm border-t hover:bg-accent transition-colors text-primary disabled:opacity-50"
        >
          {creating ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Plus className="h-4 w-4" />
          )}
          <span className="truncate">Create &ldquo;{query}&rdquo;</span>
        </button>
      )}
    </div>
  );
}
