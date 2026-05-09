import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
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
  X,
} from 'lucide-react';
import { FileContextMenu } from '@/components/FileContextMenu';
import { FilterEditor } from '@/components/FilterEditor';
import { NovelCover } from '@/components/NovelCover';
import { coverGet } from '@/lib/tauri';
import { isImportable, kindForPath } from '@/lib/fileKind';
import { useRemoteUploadStore } from '@/stores/remoteUploadStore';
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
 *  (real artwork) for comics, so the grid stays visually rich for both. */
function CardCover({ file }: { file: FileEntry }) {
  if (kindForPath(file.path) === 'novel') {
    return <NovelCover tags={file.tags} fileId={file.id} displayName={file.display_name} />;
  }
  return <ComicCover fileId={file.id} />;
}

/** Storage status pill — identical geometry across states; only icon and color
 *  change so the badge reads as one consistent visual element. */
function CardStatus({ storageKind, isUploading }: { storageKind?: string; isUploading: boolean }) {
  const wrapper =
    'flex items-center justify-center h-6 w-6 rounded-full bg-background/90 backdrop-blur-sm border border-border/40 shadow-sm';
  if (isUploading) {
    return (
      <div className={wrapper} title="Uploading…" aria-label="Uploading">
        <Loader2 className="h-3.5 w-3.5 animate-spin text-amber-600 dark:text-amber-400" />
      </div>
    );
  }
  if (storageKind === 'remote') {
    return (
      <div className={wrapper} title="Synced to cloud" aria-label="Synced to cloud">
        <Cloud className="h-3.5 w-3.5 text-primary" />
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

// ── Props ─────────────────────────────────────────────────────────────────────

interface FileListProps {
  files: FileEntry[];
  total?: number;
  loadingMore?: boolean;
  onLoadMore?: () => void;
  filterKey?: string | number | null;
  onFileClick?: (file: FileEntry) => void;
  onFileEdit?: (file: FileEntry) => void;
  onFileDelete?: (file: FileEntry) => void;
  onBulkUpload?: (fileIds: number[]) => void;
  remoteEnabled?: boolean;
  /** All defined tags, used by the filter editor's tag picker. */
  availableTags?: ReadonlyArray<Tag>;
}

// ── FileList ──────────────────────────────────────────────────────────────────

export function FileList({
  files: rawFiles,
  total,
  loadingMore = false,
  onLoadMore,
  filterKey = null,
  onFileClick,
  onFileEdit,
  onFileDelete,
  onBulkUpload,
  remoteEnabled = false,
  availableTags = [],
}: FileListProps) {
  const [containerWidth, setContainerWidth] = useState(0);
  const [sortBy, setSortBy] = useState<SortKey>('name');
  const [sortDesc, setSortDesc] = useState(false);
  const [conditions, setConditions] = useState<Condition[]>([]);
  const [filterOpen, setFilterOpen] = useState(false);
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());

  // Map for chip rendering — looks up tag names by id.
  const tagsById = useMemo(() => {
    const m = new Map<number, Tag>();
    for (const t of availableTags) m.set(t.id, t);
    return m;
  }, [availableTags]);

  const uploadState = useRemoteUploadStore();
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

  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const prevFilterKeyRef = useRef(filterKey);

  // Drop legacy rows whose extension is no longer in the supported set
  // (.epub / .pdf / standalone images from the pre-removal era).
  const importable = useMemo(
    () => rawFiles.filter((f) => isImportable(f.path)),
    [rawFiles]
  );

  // Filter + sort, derived. Keeps virtualizer / interaction state in lockstep
  // with whatever the user has chosen above the grid.
  const files = useMemo(() => {
    const filtered = applyConditions(importable, conditions);
    const sorted = [...filtered].sort((a, b) => {
      const cmp = compareFiles(a, b, sortBy);
      return sortDesc ? -cmp : cmp;
    });
    return sorted;
  }, [importable, conditions, sortBy, sortDesc]);

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

  /** Quick-select the first N eligible files in the current sort/filter view.
   *  Eligibility matches the per-card block check: skip files already remote
   *  and files with an in-flight upload entry. Replaces the existing
   *  selection (`+10` would be a different action; this is "set to first N"). */
  const selectFirstN = useCallback(
    (n: number) => {
      const eligible: number[] = [];
      for (const f of files) {
        if (f.storage_kind === 'remote' || inFlightUploadIds.has(f.id)) continue;
        eligible.push(f.id);
        if (eligible.length >= n) break;
      }
      setSelectedIds(new Set(eligible));
    },
    [files, inFlightUploadIds]
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
        const blocked = file.storage_kind === 'remote' || inFlightUploadIds.has(file.id);
        if (!blocked) toggleSelection(file.id);
      } else {
        onFileClick?.(file);
      }
    },
    [selectionMode, inFlightUploadIds, toggleSelection, onFileClick]
  );

  // ── Grid sizing ──────────────────────────────────────────────────────────
  const colCount = Math.max(
    1,
    Math.floor((containerWidth - GRID_PAD * 2 + GRID_GAP) / (CARD_WIDTH + GRID_GAP))
  );
  const gridRowCount = Math.ceil(files.length / colCount);

  const virtualizer = useVirtualizer({
    count: gridRowCount,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: () => CARD_HEIGHT + GRID_GAP,
    overscan: OVERSCAN,
  });
  const virtualItems = virtualizer.getVirtualItems();
  const totalVirtualSize = virtualizer.getTotalSize();

  // Trigger backend load-more when the last virtual row is near the end.
  // Compare against the *unfiltered* loaded count (`rawFiles.length`) so an
  // active filter pill shrinking `files` doesn't fake a "we need more rows"
  // signal. With the route's bumped page size this is normally a no-op, but
  // it stays correct if pagination ever returns.
  useEffect(() => {
    const lastItem = virtualItems[virtualItems.length - 1];
    if (!lastItem || loadingMore || rawFiles.length >= (total ?? 0)) return;
    if (lastItem.index >= gridRowCount - 1 - LOAD_MORE_THRESHOLD) {
      onLoadMore?.();
    }
  }, [virtualItems, gridRowCount, rawFiles.length, total, loadingMore, onLoadMore]);

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
            onClick={() => setSortDesc((d) => !d)}
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
            disabled={files.length === 0}
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
                onClick={() => selectFirstN(files.length)}
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
                className="h-8 text-xs"
                disabled={!remoteEnabled}
                onClick={() => {
                  const eligibleIds = Array.from(selectedIds).filter(
                    (id) => !inFlightUploadIds.has(id)
                  );
                  if (eligibleIds.length > 0) onBulkUpload?.(eligibleIds);
                  exitSelectionMode();
                }}
              >
                ☁ Upload to Cloud
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
            const endIdx = Math.min(startIdx + colCount, files.length);
            const slice = files.slice(startIdx, endIdx);
            return (
              <div
                key={virtualRow.index}
                style={{
                  position: 'absolute',
                  top: 0,
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
                  const blocked = file.storage_kind === 'remote' || inFlightUploadIds.has(file.id);
                  const isSelected = selectedIds.has(file.id);
                  return (
                    <div
                      key={file.id}
                      className="relative group"
                      style={{ width: CARD_WIDTH, height: CARD_HEIGHT }}
                    >
                      {selectionMode && (
                        <div className="absolute top-2 left-2 z-10">
                          <label className="flex items-center justify-center h-6 w-6 rounded-full bg-background/90 backdrop-blur-sm border border-border/40 shadow-sm cursor-pointer hover:bg-background transition-colors">
                            <input
                              type="checkbox"
                              checked={blocked ? false : isSelected}
                              onChange={() => !blocked && toggleSelection(file.id)}
                              onClick={(e) => e.stopPropagation()}
                              disabled={blocked}
                              className="h-3.5 w-3.5 rounded border-border accent-primary disabled:opacity-30 cursor-pointer disabled:cursor-not-allowed"
                              aria-label={`Select ${file.display_name}`}
                            />
                          </label>
                        </div>
                      )}
                      <button
                        type="button"
                        onClick={() => handleCardClick(file)}
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
                          <CardCover file={file} />
                          <div className="absolute bottom-1.5 left-1.5">
                            <CardStatus
                              storageKind={file.storage_kind}
                              isUploading={inFlightUploadIds.has(file.id)}
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
                          {file.authors && file.authors.length > 0 && (
                            <p className="text-xs text-muted-foreground line-clamp-1">
                              {file.authors.map((a) => a.name).join(', ')}
                            </p>
                          )}
                        </div>
                      </button>
                      {!selectionMode && onFileEdit && onFileDelete && (
                        <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity z-10">
                          <FileContextMenu
                            file={file}
                            onEdit={onFileEdit}
                            onDelete={onFileDelete}
                          />
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            );
          })}
        </div>

        {files.length === 0 && (
          <div className="flex items-center justify-center h-24 text-sm text-muted-foreground">
            {importable.length === 0
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
            {total != null ? `loading ${total - files.length} more…` : 'loading more…'}
          </div>
        )}
      </div>
    </div>
  );
}
