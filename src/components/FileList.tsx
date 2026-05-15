import { useState, useEffect, useRef, useCallback, useMemo, memo } from 'react';
import { useStore } from '@tanstack/react-store';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Button } from '@/components/ui/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  ArrowUp,
  ArrowDown,
  BookOpen,
  ChevronDown,
  Cloud,
  Filter as FilterIcon,
  HardDrive,
  Loader2,
  Trash2,
  X,
} from 'lucide-react';
import { FileContextMenu } from '@/components/FileContextMenu';
import { FilterEditor } from '@/components/FilterEditor';
import { NovelCover } from '@/components/NovelCover';
import { coverGet } from '@/lib/tauri';
import {
  isImportable,
  schemaForCategoryId,
  type CardFieldKey,
  type CategorySchema,
} from '@/lib/categorySchema';
import { useAppState } from '@/stores/appStore';
import { useRemoteUploadStore } from '@/stores/remoteUploadStore';
import { useRemoteDownloadStore } from '@/stores/remoteDownloadStore';
import { useRemoteDeleteStore } from '@/stores/remoteDeleteStore';
import { fileStore, useFile } from '@/stores/fileStore';
import { applyConditions, describeCondition, type Condition } from '@/lib/filters';
import type { FileEntry, Tag } from '@/types';

// ── Subcomponents ─────────────────────────────────────────────────────────────

/** Comic cover: lazy-fetches stored cover art, falls back to a book icon. */
function ComicCover({ fileId }: { fileId: number }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    coverGet(fileId)
      .then(({ data, mime_type }) => setSrc(`data:${mime_type};base64,${data}`))
      .catch(() => {});
  }, [fileId]);
  return src ? (
    <img src={src} alt="Cover" className="h-full w-full object-cover" />
  ) : (
    <BookOpen className="h-8 w-8 text-muted-foreground/40" />
  );
}

/** Routes a card cover to NovelCover (procedural) for novels and ComicCover
 *  (real artwork) for comics, so the grid stays visually rich for both.
 *  Picks by the file's category schema; for files in unknown categories
 *  the default schema (novel) wins, matching old extension-based routing
 *  for the .txt case. */
function CardCover({ file, schema }: { file: FileEntry; schema: CategorySchema }) {
  if (schema.slug === 'novel') {
    return <NovelCover tags={file.tags} fileId={file.id} displayName={file.display_name} />;
  }
  return <ComicCover fileId={file.id} />;
}

/** Render one card body field. Returns null when the row has no value
 *  for the field, so the card doesn't sprout empty rows. */
function CardField({ field, file }: { field: CardFieldKey; file: FileEntry }) {
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
      // Compact inline tag chips — full chip styling lives in the
      // edit dialog. Cap at 3 to keep the card height stable.
      return (
        <p className="text-[11px] text-muted-foreground line-clamp-1">
          {file.tags.slice(0, 3).map((t) => `#${t.name}`).join(' ')}
        </p>
      );
  }
}

/** Storage status pill — identical geometry across states; only icon and color
 *  change so the badge reads as one consistent visual element. */
function CardStatus({
  storageKind,
  isUploading,
  hasLocalCache,
}: {
  storageKind?: string;
  isUploading: boolean;
  hasLocalCache: boolean;
}) {
  // `rounded-md` matches the card cover's corner radius so the badge reads
  // as part of the same visual system instead of a circular sticker.
  const wrapper =
    'flex items-center justify-center h-6 w-6 rounded-md bg-background/90 backdrop-blur-sm border border-border/40 shadow-sm';
  if (isUploading) {
    return (
      <div className={wrapper} title="Uploading…" aria-label="Uploading">
        <Loader2 className="h-3.5 w-3.5 animate-spin text-amber-600 dark:text-amber-400" />
      </div>
    );
  }
  if (storageKind === 'remote') {
    // The dot in the corner indicates a local cache is also present —
    // user can read the file without re-downloading. Color-matched to
    // the success token so the badge reads the same way the upload
    // panel does.
    const title = hasLocalCache ? 'Synced to cloud · cached locally' : 'Synced to cloud';
    return (
      <div className={`${wrapper} relative`} title={title} aria-label={title}>
        <Cloud className="h-3.5 w-3.5 text-primary" />
        {hasLocalCache && (
          <span
            className="absolute -bottom-0.5 -right-0.5 h-2 w-2 rounded-full bg-success border border-background"
            aria-hidden="true"
          />
        )}
      </div>
    );
  }
  return (
    <div className={wrapper} title="Local only" aria-label="Local only">
      <HardDrive className="h-3.5 w-3.5 text-muted-foreground" />
    </div>
  );
}

// ── Constants ─────────────────────────────────────────────────────────────────

const CARD_WIDTH = 180;
const CARD_HEIGHT = 280;
const GRID_GAP = 16;
const GRID_PAD = 4;
const OVERSCAN = 4;
const LOAD_MORE_THRESHOLD = 5;
const DEBOUNCE_MS = 150;

type SortKey = 'name' | 'created' | 'updated';

const SORT_OPTIONS: ReadonlyArray<{ value: SortKey; label: string }> = [
  { value: 'name', label: 'Name' },
  { value: 'created', label: 'Date added' },
  { value: 'updated', label: 'Last updated' },
];

function compareFiles(a: FileEntry, b: FileEntry, key: SortKey): number {
  if (key === 'name') return a.display_name.localeCompare(b.display_name);
  if (key === 'created') return a.created_at.localeCompare(b.created_at);
  return a.updated_at.localeCompare(b.updated_at);
}

// ── FileCard ──────────────────────────────────────────────────────────────────

interface FileCardProps {
  id: number;
  isSelected: boolean;
  isUploading: boolean;
  blocked: boolean;
  selectionMode: boolean;
  onCardClick: (file: FileEntry) => void;
  onToggleSelect: (id: number) => void;
  onEdit?: (file: FileEntry) => void;
  onDelete?: (file: FileEntry) => void;
}

/** Per-row component subscribed to its own entry via `useFile(id)`. Wrapped
 *  in `memo` so single-row patches in the store re-render only this card,
 *  not the rest of the grid. */
const FileCard = memo(function FileCard({
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
  // Brief absence is possible right after `removeFile(id)`: byId loses the
  // row in the same setState that drops it from the view's ids, but a stale
  // render frame can still ask for it. Render nothing instead of crashing.
  const categories = useAppState((s) => s.categories);
  if (!file) return null;

  // Resolve the schema for this row's category. Drives both the cover
  // style (NovelCover vs ComicCover) and the card body field list.
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
              // Slightly smaller than the storage icon's `h-3.5` so the
              // filled-checked state doesn't crowd the wrapper edges — the
              // storage icon's SVG has built-in whitespace, the native
              // checkbox doesn't, so we shrink the box to match the
              // apparent inset.
              className="h-3 w-3 rounded border-border accent-primary disabled:opacity-30 cursor-pointer disabled:cursor-not-allowed"
              aria-label={`Select ${file.display_name}`}
            />
          </label>
        </div>
      )}
      <button
        type="button"
        onClick={() => onCardClick(file)}
        className={`w-full h-full flex flex-col gap-2 text-left rounded-lg p-2 transition-colors focus:outline-none focus:ring-2 focus:ring-ring ${
          isSelected
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
        <div className="relative aspect-[2/3] w-full rounded-md overflow-hidden bg-secondary/40 border flex items-center justify-center">
          <CardCover file={file} schema={schema} />
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

// ── Props ─────────────────────────────────────────────────────────────────────

interface FileListProps {
  ids: number[];
  total?: number;
  loadingMore?: boolean;
  onLoadMore?: () => void;
  filterKey?: string | number | null;
  onFileClick?: (file: FileEntry) => void;
  onFileEdit?: (file: FileEntry) => void;
  onFileDelete?: (file: FileEntry) => void;
  onBulkUpload?: (fileIds: number[]) => void;
  /** Pull selected remote files to local cache (cloud copy stays). */
  onBulkDownload?: (fileIds: number[]) => void;
  /** Delete selected files (any mix of local + remote) via the worker. */
  onBulkDelete?: (fileIds: number[]) => void;
  remoteEnabled?: boolean;
  /** All defined tags, used by the filter editor's tag picker. */
  availableTags?: ReadonlyArray<Tag>;
  /** Optional controlled sort. When supplied alongside `onSortChange`,
   *  parent owns the sort state — typically because it's pushed into a
   *  server-side query. In that case set `applySort={false}` so the local
   *  comparator doesn't reshuffle pre-sorted rows from the server. */
  sortBy?: SortKey;
  sortDesc?: boolean;
  onSortChange?: (sortBy: SortKey, sortDesc: boolean) => void;
  /** Default true. Set false when the parent feeds already-sorted ids
   *  (server-side sort). */
  applySort?: boolean;
  /** Optional controlled filter conditions. Same shape as sort: parent owns
   *  the editor state when it's pushed into the server query. Set
   *  `applyConditions={false}` so the local filter doesn't double-filter
   *  rows the server has already filtered. */
  conditions?: Condition[];
  onConditionsChange?: (conditions: Condition[]) => void;
  /** Default true. Set false when the parent feeds already-filtered ids
   *  (server-side filter). */
  applyConditionsClientSide?: boolean;
}

// ── FileList ──────────────────────────────────────────────────────────────────

export function FileList({
  ids,
  total,
  loadingMore = false,
  onLoadMore,
  filterKey = null,
  onFileClick,
  onFileEdit,
  onFileDelete,
  onBulkUpload,
  onBulkDownload,
  onBulkDelete,
  remoteEnabled = false,
  availableTags = [],
  sortBy: sortByProp,
  sortDesc: sortDescProp,
  onSortChange,
  applySort = true,
  conditions: conditionsProp,
  onConditionsChange,
  applyConditionsClientSide = true,
}: FileListProps) {
  const [containerWidth, setContainerWidth] = useState(0);
  const [internalSortBy, setInternalSortBy] = useState<SortKey>('name');
  const [internalSortDesc, setInternalSortDesc] = useState(false);
  const sortBy = sortByProp ?? internalSortBy;
  const sortDesc = sortDescProp ?? internalSortDesc;
  const setSortBy = useCallback(
    (next: SortKey) => {
      if (onSortChange) onSortChange(next, sortDesc);
      else setInternalSortBy(next);
    },
    [onSortChange, sortDesc]
  );
  const setSortDesc = useCallback(
    (next: boolean) => {
      if (onSortChange) onSortChange(sortBy, next);
      else setInternalSortDesc(next);
    },
    [onSortChange, sortBy]
  );
  const [internalConditions, setInternalConditions] = useState<Condition[]>([]);
  const conditions = conditionsProp ?? internalConditions;
  const setConditions = useCallback<React.Dispatch<React.SetStateAction<Condition[]>>>(
    (next) => {
      const resolved =
        typeof next === 'function' ? next(conditionsProp ?? internalConditions) : next;
      if (onConditionsChange) onConditionsChange(resolved);
      else setInternalConditions(resolved);
    },
    [onConditionsChange, conditionsProp, internalConditions]
  );
  const [filterOpen, setFilterOpen] = useState(false);
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());

  // Subscribe to the entire byId map so filter/sort recomputes when any
  // row's contents change. Per-card subscription lives inside <FileCard>;
  // this top-level subscription only feeds the filter pipeline, which
  // produces a new id ordering. Cards themselves still skip render via
  // `memo` because their `id` prop is stable.
  const byId = useStore(fileStore, (s) => s.byId);

  // Map for chip rendering — looks up tag names by id.
  const tagsById = useMemo(() => {
    const m = new Map<number, Tag>();
    for (const t of availableTags) m.set(t.id, t);
    return m;
  }, [availableTags]);

  const uploadState = useRemoteUploadStore();
  const downloadState = useRemoteDownloadStore();
  const deleteState = useRemoteDeleteStore();
  // Files that are queued OR actively uploading. Both states block re-selection
  // and re-enqueue — without `pending`, a queued-but-not-yet-running file
  // would still be selectable because its `storage_kind` is still `local`
  // until the worker flips it on success.
  const inFlightUploadIds = useMemo(
    () =>
      new Set(
        uploadState.uploads
          .filter((u) => u.status === 'pending' || u.status === 'uploading')
          .map((u) => u.file_id)
      ),
    [uploadState.uploads]
  );
  const inFlightDownloadIds = useMemo(
    () =>
      new Set(
        downloadState.downloads
          .filter((d) => d.status === 'pending' || d.status === 'downloading')
          .map((d) => d.file_id)
      ),
    [downloadState.downloads]
  );
  const inFlightDeleteIds = useMemo(
    () =>
      new Set(
        deleteState.deletes
          .filter((d) => d.status === 'pending' || d.status === 'deleting')
          .map((d) => d.file_id)
      ),
    [deleteState.deletes]
  );
  // A file is unselectable while any worker is touching it, regardless of
  // storage_kind. Per-action eligibility (upload-only-local etc.) is
  // re-checked at click time on each bulk button.
  const inFlightAnyIds = useMemo(() => {
    const s = new Set<number>();
    for (const id of inFlightUploadIds) s.add(id);
    for (const id of inFlightDownloadIds) s.add(id);
    for (const id of inFlightDeleteIds) s.add(id);
    return s;
  }, [inFlightUploadIds, inFlightDownloadIds, inFlightDeleteIds]);

  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const prevFilterKeyRef = useRef(filterKey);

  // Resolve incoming ids to their current entries. Drop any rows whose
  // extension is no longer in the supported set (.epub / .pdf / standalone
  // images from the pre-removal era). Missing ids (briefly possible during
  // the ms between `removeFile` and the parent re-render) are skipped.
  const importableEntries = useMemo(() => {
    const out: FileEntry[] = [];
    for (const id of ids) {
      const f = byId.get(id);
      if (f && isImportable(f.path)) out.push(f);
    }
    return out;
  }, [ids, byId]);

  // Filter + sort, derived. Keeps virtualizer / interaction state in lockstep
  // with whatever the user has chosen above the grid. Both `applySort` and
  // `applyConditionsClientSide` flip to false when the parent has pushed the
  // operation into a server query — re-running the predicate locally would
  // either duplicate work or, worse, fight the server's ordering (SQLite's
  // NOCASE byte compare vs. JS's locale-aware `localeCompare`).
  const visibleEntries = useMemo(() => {
    const filtered = applyConditionsClientSide
      ? applyConditions(importableEntries, conditions)
      : importableEntries;
    if (!applySort) return filtered;
    const sorted = [...filtered].sort((a, b) => {
      const cmp = compareFiles(a, b, sortBy);
      return sortDesc ? -cmp : cmp;
    });
    return sorted;
  }, [importableEntries, conditions, sortBy, sortDesc, applySort, applyConditionsClientSide]);

  // Scroll to top + reset selection + clear filter conditions when
  // category/search changes. Conditions are scoped to the current view.
  useEffect(() => {
    if (prevFilterKeyRef.current === filterKey) return;
    prevFilterKeyRef.current = filterKey;
    scrollContainerRef.current?.scrollTo(0, 0);
    setSelectedIds(new Set());
    setSelectionMode(false);
    setConditions([]);
    setFilterOpen(false);
  }, [filterKey]);

  const toggleSelection = useCallback((id: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const clearSelection = useCallback(() => setSelectedIds(new Set()), []);

  /** Quick-select the first N files in the current sort/filter view that
   *  aren't already being touched by a worker. Selection itself is generic
   *  now — the bulk Upload / Download / Delete buttons each apply their own
   *  eligibility filter at click time. */
  const selectFirstN = useCallback(
    (n: number) => {
      const eligible: number[] = [];
      for (const f of visibleEntries) {
        if (inFlightAnyIds.has(f.id)) continue;
        eligible.push(f.id);
        if (eligible.length >= n) break;
      }
      setSelectedIds(new Set(eligible));
    },
    [visibleEntries, inFlightAnyIds]
  );

  const exitSelectionMode = useCallback(() => {
    setSelectionMode(false);
    setSelectedIds(new Set());
  }, []);

  const removeCondition = useCallback((id: string) => {
    setConditions((prev) => prev.filter((c) => c.id !== id));
  }, []);

  const handleCardClick = useCallback(
    (file: FileEntry) => {
      if (selectionMode) {
        if (!inFlightAnyIds.has(file.id)) toggleSelection(file.id);
      } else {
        onFileClick?.(file);
      }
    },
    [selectionMode, inFlightAnyIds, toggleSelection, onFileClick]
  );

  // ── Grid sizing ──────────────────────────────────────────────────────────
  const colCount = Math.max(
    1,
    Math.floor((containerWidth - GRID_PAD * 2 + GRID_GAP) / (CARD_WIDTH + GRID_GAP))
  );
  const gridRowCount = Math.ceil(visibleEntries.length / colCount);

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
  // shrinking the view doesn't fake a "we need more rows" signal. With the
  // route's bumped page size this is normally a no-op, but it stays correct
  // if pagination ever returns.
  useEffect(() => {
    const lastItem = virtualItems[virtualItems.length - 1];
    if (!lastItem || loadingMore || ids.length >= (total ?? 0)) return;
    if (lastItem.index >= gridRowCount - 1 - LOAD_MORE_THRESHOLD) {
      onLoadMore?.();
    }
  }, [virtualItems, gridRowCount, ids.length, total, loadingMore, onLoadMore]);

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
  }, []);

  return (
    <div className="flex-1 flex flex-col min-h-0">
      {/* ── Header bar ──────────────────────────────────────────────────── */}
      {!selectionMode ? (
        <div className="flex items-center gap-2 pb-3 shrink-0 flex-wrap">
          <Select value={sortBy} onValueChange={(v) => setSortBy(v as SortKey)}>
            <SelectTrigger className="h-8 w-auto text-xs gap-1.5">
              <span className="text-muted-foreground">Sort</span>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {SORT_OPTIONS.map((opt) => (
                <SelectItem key={opt.value} value={opt.value} className="text-xs">
                  {opt.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            variant="outline"
            size="icon"
            className="h-8 w-8 shrink-0"
            aria-label={sortDesc ? 'Sort descending' : 'Sort ascending'}
            onClick={() => setSortDesc(!sortDesc)}
          >
            {sortDesc ? (
              <ArrowDown className="h-3.5 w-3.5" />
            ) : (
              <ArrowUp className="h-3.5 w-3.5" />
            )}
          </Button>
          <span className="h-5 w-px bg-border mx-1" aria-hidden="true" />
          <Popover open={filterOpen} onOpenChange={setFilterOpen}>
            <PopoverTrigger asChild>
              <Button
                variant={conditions.length > 0 ? 'default' : 'outline'}
                size="sm"
                className="h-8 text-xs gap-1.5"
              >
                <FilterIcon className="h-3.5 w-3.5" />
                Filter
                {conditions.length > 0 && (
                  <span className="ml-0.5 rounded-full bg-background/30 px-1.5 text-[10px] leading-tight">
                    {conditions.length}
                  </span>
                )}
              </Button>
            </PopoverTrigger>
            <PopoverContent align="start" sideOffset={6} className="w-auto p-3">
              <FilterEditor
                conditions={conditions}
                onConditionsChange={setConditions}
                tags={availableTags}
              />
            </PopoverContent>
          </Popover>
          {conditions.map((c) => (
            <div
              key={c.id}
              className="inline-flex items-center rounded-full border bg-secondary/50 hover:bg-secondary transition-colors h-8"
            >
              <button
                type="button"
                onClick={() => setFilterOpen(true)}
                className="text-xs pl-3 pr-1.5 h-full text-foreground/80 focus:outline-none"
                aria-label={`Edit condition: ${describeCondition(c, tagsById)}`}
              >
                {describeCondition(c, tagsById)}
              </button>
              <button
                type="button"
                onClick={() => removeCondition(c.id)}
                className="px-1.5 h-full text-muted-foreground hover:text-foreground rounded-r-full focus:outline-none"
                aria-label="Remove condition"
              >
                <X className="h-3 w-3" />
              </button>
            </div>
          ))}
          <div className="flex-1" />
          <Button
            variant="outline"
            size="sm"
            className="h-8 text-xs"
            onClick={() => setSelectionMode(true)}
            disabled={visibleEntries.length === 0}
          >
            Select
          </Button>
        </div>
      ) : (
        <div className="flex items-center gap-3 pb-3 shrink-0">
          <span className="text-sm font-medium text-foreground">
            {selectedIds.size === 0
              ? 'Select files'
              : `${selectedIds.size} file${selectedIds.size === 1 ? '' : 's'} selected`}
          </span>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" size="sm" className="h-8 text-xs gap-1.5">
                Select first
                <ChevronDown className="h-3 w-3" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
              {[10, 25, 50, 100].map((n) => (
                <DropdownMenuItem
                  key={n}
                  className="text-xs"
                  onClick={() => selectFirstN(n)}
                >
                  First {n}
                </DropdownMenuItem>
              ))}
              <DropdownMenuSeparator />
              <DropdownMenuItem
                className="text-xs"
                onClick={() => selectFirstN(visibleEntries.length)}
              >
                All eligible
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <div className="flex-1" />
          {selectedIds.size > 0 && (
            <>
              <Button
                variant="outline"
                size="sm"
                className="h-8 text-xs"
                onClick={clearSelection}
              >
                Clear
              </Button>
              <Button
                size="sm"
                variant="outline"
                className="h-8 text-xs"
                disabled={!remoteEnabled}
                onClick={() => {
                  // Upload only fires for currently-local files; remote
                  // selections are silently dropped from this batch.
                  const ids = Array.from(selectedIds).filter((id) => {
                    const f = byId.get(id);
                    return (
                      !!f &&
                      f.storage_kind !== 'remote' &&
                      !inFlightUploadIds.has(id)
                    );
                  });
                  if (ids.length > 0) onBulkUpload?.(ids);
                  exitSelectionMode();
                }}
              >
                ☁ Upload
              </Button>
              <Button
                size="sm"
                variant="outline"
                className="h-8 text-xs"
                disabled={!remoteEnabled || !onBulkDownload}
                onClick={() => {
                  // Download is the inverse: only remote files. Cloud copy
                  // stays in place (this is "copy back, keep cloud").
                  const ids = Array.from(selectedIds).filter((id) => {
                    const f = byId.get(id);
                    return (
                      !!f &&
                      f.storage_kind === 'remote' &&
                      !inFlightDownloadIds.has(id)
                    );
                  });
                  if (ids.length > 0) onBulkDownload?.(ids);
                  exitSelectionMode();
                }}
              >
                ⬇ Download
              </Button>
              <Button
                size="sm"
                variant="destructive"
                className="h-8 text-xs"
                disabled={!onBulkDelete}
                onClick={() => {
                  const ids = Array.from(selectedIds).filter(
                    (id) => !inFlightDeleteIds.has(id)
                  );
                  if (ids.length > 0) onBulkDelete?.(ids);
                  exitSelectionMode();
                }}
              >
                <Trash2 className="h-3.5 w-3.5 mr-1" />
                Delete
              </Button>
            </>
          )}
          <Button
            variant="ghost"
            size="sm"
            className="h-8 text-xs"
            onClick={exitSelectionMode}
            aria-label="Cancel selection"
          >
            <X className="h-3.5 w-3.5 mr-1" />
            Cancel
          </Button>
        </div>
      )}

      <div ref={scrollContainerRef} className="flex-1 min-h-0 overflow-auto">
        <div
          style={{ height: totalVirtualSize, position: 'relative', padding: GRID_PAD }}
        >
          {virtualItems.map((virtualRow) => {
            const startIdx = virtualRow.index * colCount;
            const endIdx = Math.min(startIdx + colCount, visibleEntries.length);
            const slice = visibleEntries.slice(startIdx, endIdx);
            return (
              <div
                key={virtualRow.index}
                style={{
                  position: 'absolute',
                  // Offset by GRID_PAD so the first row's selection ring
                  // (rendered as box-shadow 1px outside the button) has
                  // breathing room before the scroll container's clip
                  // boundary. The parent's `padding: GRID_PAD` doesn't
                  // affect absolute children, so we have to apply the
                  // top inset here.
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
                {slice.map((file) => {
                  const blocked = inFlightAnyIds.has(file.id);
                  const isSelected = selectedIds.has(file.id);
                  return (
                    <FileCard
                      key={file.id}
                      id={file.id}
                      isSelected={isSelected}
                      isUploading={inFlightUploadIds.has(file.id)}
                      blocked={blocked}
                      selectionMode={selectionMode}
                      onCardClick={handleCardClick}
                      onToggleSelect={toggleSelection}
                      onEdit={onFileEdit}
                      onDelete={onFileDelete}
                    />
                  );
                })}
              </div>
            );
          })}
        </div>

        {visibleEntries.length === 0 && (
          <div className="flex items-center justify-center h-24 text-sm text-muted-foreground">
            {importableEntries.length === 0
              ? 'No files in library.'
              : 'No files match the active filters.'}
          </div>
        )}

        {loadingMore && (
          <div
            className="flex items-center justify-center py-3 gap-2 text-xs text-muted-foreground font-serif-italic"
            aria-live="polite"
          >
            <Loader2 className="h-3 w-3 animate-spin" aria-hidden="true" />
            {total != null ? `loading ${total - visibleEntries.length} more…` : 'loading more…'}
          </div>
        )}
      </div>
    </div>
  );
}
