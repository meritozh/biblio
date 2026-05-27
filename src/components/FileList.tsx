import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useStore } from '@tanstack/react-store';
import { ArrowLeft } from 'lucide-react';
import { FileListContent } from '@/components/FileListContent';
import {
  FileListHeader,
  type SortKey,
} from '@/components/FileListHeader';
import { applyConditions, type Condition } from '@/lib/filters';
import { isImportable } from '@/lib/categorySchema';
import { fileStore } from '@/stores/fileStore';
import { useRemoteDeleteStore } from '@/stores/remoteDeleteStore';
import { useRemoteDownloadStore } from '@/stores/remoteDownloadStore';
import { useRemoteUploadStore } from '@/stores/remoteUploadStore';
import type {
  Author,
  Collection,
  ViewMode,
  FileEntry,
  Tag,
} from '@/types';

function compareFiles(a: FileEntry, b: FileEntry, key: SortKey): number {
  if (key === 'name') return a.display_name.localeCompare(b.display_name);
  if (key === 'created') return a.created_at.localeCompare(b.created_at);
  return a.updated_at.localeCompare(b.updated_at);
}

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
  /** Clear the local cache copy for selected rows that currently carry a
   *  `local_cache_path` (the remote copy stays). Rows without a cache are
   *  silently skipped — the button is disabled when none qualify. */
  onBulkClearCache?: (fileIds: number[]) => void;
  remoteEnabled?: boolean;
  availableTags?: ReadonlyArray<Tag>;
  availableAuthors?: ReadonlyArray<Author>;
  /** Optional controlled sort. When supplied alongside `onSortChange`,
   *  parent owns the sort state — typically because it's pushed into a
   *  server-side query. In that case set `applySort={false}` so the local
   *  comparator doesn't reshuffle pre-sorted rows from the server. */
  sortBy?: SortKey;
  sortDesc?: boolean;
  onSortChange?: (sortBy: SortKey, sortDesc: boolean) => void;
  /** Default true. Set false when the parent feeds already-sorted ids. */
  applySort?: boolean;
  /** Optional controlled filter conditions. Same shape as sort: parent
   *  owns the editor state when it's pushed into the server query. */
  conditions?: Condition[];
  onConditionsChange?: (conditions: Condition[]) => void;
  /** Default true. Set false when the parent feeds already-filtered ids. */
  applyConditionsClientSide?: boolean;
  /** Controlled view-mode toggle. When `viewModeAvailable` is true the
   *  header surfaces a "View" select with Flat / By author / By series. */
  viewMode?: ViewMode;
  onViewModeChange?: (mode: ViewMode) => void;
  viewModeAvailable?: boolean;
  /** Required when rendering the collection grid. Empty array is a valid
   *  state (no multi-member collections in scope). */
  collections?: Collection[];
  onOpenCollection?: (c: Collection) => void;
  /** Rendered above the grid as a back chip — used during the drill-down
   *  from a collection card into its constituent files. */
  breadcrumb?: { label: string; onBack: () => void } | null;
}

/** Orchestrator for the library file list. Owns selection state, the
 *  controlled/uncontrolled bridge for sort + filter, and the worker-store
 *  subscriptions used by bulk actions. Header and content panes are
 *  presentational — `<FileListHeader>` for the toolbar,
 *  `<FileListContent>` for the virtualized grid. */
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
  onBulkClearCache,
  remoteEnabled = false,
  availableTags = [],
  availableAuthors = [],
  sortBy: sortByProp,
  sortDesc: sortDescProp,
  onSortChange,
  applySort = true,
  conditions: conditionsProp,
  onConditionsChange,
  applyConditionsClientSide = true,
  viewMode = 'flat',
  onViewModeChange,
  viewModeAvailable = false,
  collections,
  onOpenCollection,
  breadcrumb = null,
}: FileListProps) {
  // The body renders collection cards instead of files when the view mode
  // is non-flat AND we're not currently drilled into a specific
  // collection. Once the user drills in, the parent provides the
  // collection's file_ids via `ids` and the breadcrumb chip.
  const showCollections =
    viewMode !== 'flat' && breadcrumb == null && collections != null;

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
  // row's contents change. Per-card subscription lives inside the card
  // components; this top-level subscription only feeds the filter
  // pipeline, which produces a new id ordering.
  const byId = useStore(fileStore, (s) => s.byId);

  const tagsById = useMemo(() => {
    const m = new Map<number, Tag>();
    for (const t of availableTags) m.set(t.id, t);
    return m;
  }, [availableTags]);
  const authorsById = useMemo(() => {
    const m = new Map<number, Author>();
    for (const a of availableAuthors) m.set(a.id, a);
    return m;
  }, [availableAuthors]);

  const uploadState = useRemoteUploadStore();
  const downloadState = useRemoteDownloadStore();
  const deleteState = useRemoteDeleteStore();
  // Files that are queued OR actively in-flight. Both states block
  // re-selection and re-enqueue — without `pending`, a queued-but-not-yet
  // -running file would still be selectable because its `storage_kind`
  // is still `local` until the worker flips it on success.
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

  // Resolve incoming ids to entries. Importability is judged by the real
  // filename's extension — but remote objects use an opaque, extension-less
  // storage path (the real name lives in `original_path`), so judge those by
  // `original_path` and never drop a remote library row: it's already in the
  // catalog and its type can't be reconstructed from the opaque path. Local
  // rows keep the gate; their `path` IS the real file on disk. Missing ids
  // (briefly possible between `removeFile` and the parent re-render) skip.
  const importableEntries = useMemo(() => {
    const out: FileEntry[] = [];
    for (const id of ids) {
      const f = byId.get(id);
      if (!f) continue;
      if (f.storage_kind === 'remote' || isImportable(f.original_path ?? f.path)) {
        out.push(f);
      }
    }
    return out;
  }, [ids, byId]);

  // Filter + sort, derived. Both flags flip to false when the parent has
  // pushed the operation into a server query — re-running locally would
  // either duplicate work or, worse, fight the server's ordering
  // (SQLite's NOCASE byte compare vs. JS's locale-aware `localeCompare`).
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

  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const prevFilterKeyRef = useRef(filterKey);
  const savedCollectionScrollRef = useRef(0);

  // Scroll to top + reset selection + clear filter conditions when
  // category/search changes. Conditions are scoped to the current view.
  useEffect(() => {
    if (prevFilterKeyRef.current === filterKey) return;
    prevFilterKeyRef.current = filterKey;
    scrollContainerRef.current?.scrollTo(0, 0);
    savedCollectionScrollRef.current = 0;
    setSelectedIds(new Set());
    setSelectionMode(false);
    setConditions([]);
    setFilterOpen(false);
  }, [filterKey, setConditions]);

  // Collection drill-in / drill-out is sub-navigation inside the same
  // filterKey scope, so it shouldn't go through the reset effect above —
  // doing so would wipe the collection-grid scroll position every time
  // the user clicked Back. Save the grid scroll position on drill-in
  // and restore it on drill-out; selection still resets on either
  // transition since the drilled-in file ids and the collection-grid
  // items live in different domains.
  const isDrilled = breadcrumb != null;
  const prevDrilledRef = useRef(isDrilled);
  useEffect(() => {
    const wasDrilled = prevDrilledRef.current;
    if (isDrilled === wasDrilled) return;
    prevDrilledRef.current = isDrilled;
    setSelectedIds(new Set());
    setSelectionMode(false);
    const el = scrollContainerRef.current;
    if (!el) return;
    if (isDrilled) {
      savedCollectionScrollRef.current = el.scrollTop;
      el.scrollTo(0, 0);
    } else {
      el.scrollTo(0, savedCollectionScrollRef.current);
    }
  }, [isDrilled]);

  const toggleSelection = useCallback((id: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);
  const clearSelection = useCallback(() => setSelectedIds(new Set()), []);
  const enterSelectionMode = useCallback(() => setSelectionMode(true), []);
  const exitSelectionMode = useCallback(() => {
    setSelectionMode(false);
    setSelectedIds(new Set());
  }, []);

  /** Quick-select the first N files in the current sort/filter view that
   *  aren't being touched by a worker. Per-action eligibility (upload-
   *  only-local etc.) is re-checked at click time on each bulk button. */
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

  const removeCondition = useCallback(
    (id: string) => {
      setConditions((prev) => prev.filter((c) => c.id !== id));
    },
    [setConditions]
  );

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

  // Bulk actions: compute eligibility-filtered ids at click time so the
  // header doesn't need to know about storage_kind / inFlight state.
  const handleBulkUploadClick = useCallback(() => {
    const ids = Array.from(selectedIds).filter((id) => {
      const f = byId.get(id);
      return !!f && f.storage_kind !== 'remote' && !inFlightUploadIds.has(id);
    });
    if (ids.length > 0) onBulkUpload?.(ids);
    exitSelectionMode();
  }, [selectedIds, byId, inFlightUploadIds, onBulkUpload, exitSelectionMode]);

  const handleBulkDownloadClick = useCallback(() => {
    const ids = Array.from(selectedIds).filter((id) => {
      const f = byId.get(id);
      return !!f && f.storage_kind === 'remote' && !inFlightDownloadIds.has(id);
    });
    if (ids.length > 0) onBulkDownload?.(ids);
    exitSelectionMode();
  }, [selectedIds, byId, inFlightDownloadIds, onBulkDownload, exitSelectionMode]);

  const handleBulkDeleteClick = useCallback(() => {
    const ids = Array.from(selectedIds).filter((id) => !inFlightDeleteIds.has(id));
    if (ids.length > 0) onBulkDelete?.(ids);
    exitSelectionMode();
  }, [selectedIds, inFlightDeleteIds, onBulkDelete, exitSelectionMode]);

  // Clear-cache eligibility: row must have a non-empty `local_cache_path`.
  // Pure-local rows have a null cache column and stay out (their `path` IS
  // the canonical disk location; "clearing" would be deleting the file
  // itself, which is what Delete is for).
  const cacheableSelectedIds = useMemo(() => {
    const out: number[] = [];
    for (const id of selectedIds) {
      const f = byId.get(id);
      if (f && f.local_cache_path != null && f.local_cache_path !== '') {
        out.push(id);
      }
    }
    return out;
  }, [selectedIds, byId]);

  const handleBulkClearCacheClick = useCallback(() => {
    if (cacheableSelectedIds.length > 0) onBulkClearCache?.(cacheableSelectedIds);
    exitSelectionMode();
  }, [cacheableSelectedIds, onBulkClearCache, exitSelectionMode]);

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <FileListHeader
        showCollections={showCollections}
        view={{
          viewMode,
          onViewModeChange,
          available: viewModeAvailable,
        }}
        sort={{ sortBy, sortDesc, setSortBy, setSortDesc }}
        filter={{
          conditions,
          setConditions,
          filterOpen,
          setFilterOpen,
          removeCondition,
          availableTags,
          availableAuthors,
          tagsById,
          authorsById,
        }}
        selection={{
          selectionMode,
          selectedCount: selectedIds.size,
          visibleCount: visibleEntries.length,
          enterSelectionMode,
          exitSelectionMode,
          clearSelection,
          selectFirstN,
        }}
        bulk={{
          remoteEnabled,
          canDownload: !!onBulkDownload,
          canDelete: !!onBulkDelete,
          canClearCache: !!onBulkClearCache,
          hasCacheableSelection: cacheableSelectedIds.length > 0,
          onUpload: handleBulkUploadClick,
          onDownload: handleBulkDownloadClick,
          onDelete: handleBulkDeleteClick,
          onClearCache: handleBulkClearCacheClick,
        }}
      />

      {breadcrumb && (
        <div className="flex items-center pb-3 shrink-0">
          <button
            type="button"
            onClick={breadcrumb.onBack}
            className="inline-flex items-center gap-1.5 rounded-full border bg-secondary/40 hover:bg-secondary transition-colors h-8 px-3 text-xs"
            aria-label="Back to collections"
          >
            <ArrowLeft className="h-3 w-3" aria-hidden="true" />
            <span className="text-muted-foreground">Collection</span>
            <span className="font-medium text-foreground truncate max-w-[200px]">
              {breadcrumb.label}
            </span>
          </button>
        </div>
      )}

      <FileListContent
        scrollContainerRef={scrollContainerRef}
        visibleEntries={visibleEntries}
        hasImportableEntries={importableEntries.length > 0}
        showCollections={showCollections}
        collections={collections}
        viewMode={viewMode}
        total={total}
        loadingMore={loadingMore}
        onLoadMore={onLoadMore}
        loadedCount={ids.length}
        selectionMode={selectionMode}
        selectedIds={selectedIds}
        inFlightAnyIds={inFlightAnyIds}
        inFlightUploadIds={inFlightUploadIds}
        onCardClick={handleCardClick}
        onToggleSelect={toggleSelection}
        onFileEdit={onFileEdit}
        onFileDelete={onFileDelete}
        onOpenCollection={onOpenCollection}
      />
    </div>
  );
}
