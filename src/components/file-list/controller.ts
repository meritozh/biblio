import type { Dispatch, RefObject, SetStateAction } from 'react';
import { Store } from '@tanstack/react-store';
import { applyConditions, type Condition } from '@/lib/filters';
import { isImportable } from '@/lib/categorySchema';
import { fileStore } from '@/stores/fileStore';
import type { SortKey } from '@/components/FileListHeader';
import type { Author, FileEntry, Tag, ViewMode } from '@/types';
import type {
  FileListProps,
  NormalizedFileListProps,
  FileListCallbacks,
  FileListControllerState,
  FileListController,
} from './types';

const EMPTY_TAGS: ReadonlyArray<Tag> = [];
const EMPTY_AUTHORS: ReadonlyArray<Author> = [];

function compareFiles(a: FileEntry, b: FileEntry, key: SortKey): number {
  if (key === 'name') return a.display_name.localeCompare(b.display_name);
  if (key === 'created') return a.created_at.localeCompare(b.created_at);
  return a.updated_at.localeCompare(b.updated_at);
}

export function normalizeProps({
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
  availableTags = EMPTY_TAGS,
  availableAuthors = EMPTY_AUTHORS,
  sortBy,
  sortDesc,
  onSortChange,
  applySort = true,
  conditions,
  onConditionsChange,
  applyConditionsClientSide = true,
  viewMode = 'flat',
  onViewModeChange,
  viewModeAvailable = false,
  collections,
  onOpenCollection,
  breadcrumb = null,
}: FileListProps): NormalizedFileListProps {
  return {
    ids,
    total,
    loadingMore,
    onLoadMore,
    filterKey,
    onFileClick,
    onFileEdit,
    onFileDelete,
    onBulkUpload,
    onBulkDownload,
    onBulkDelete,
    onBulkClearCache,
    remoteEnabled,
    availableTags,
    availableAuthors,
    sortBy,
    sortDesc,
    onSortChange,
    applySort,
    conditions,
    onConditionsChange,
    applyConditionsClientSide,
    viewMode,
    onViewModeChange,
    viewModeAvailable,
    collections,
    onOpenCollection,
    breadcrumb,
  };
}

function callbacksFromProps(props: NormalizedFileListProps): FileListCallbacks {
  return {
    onLoadMore: props.onLoadMore,
    onFileClick: props.onFileClick,
    onFileEdit: props.onFileEdit,
    onFileDelete: props.onFileDelete,
    onBulkUpload: props.onBulkUpload,
    onBulkDownload: props.onBulkDownload,
    onBulkDelete: props.onBulkDelete,
    onBulkClearCache: props.onBulkClearCache,
    onSortChange: props.onSortChange,
    onConditionsChange: props.onConditionsChange,
    onViewModeChange: props.onViewModeChange,
    onOpenCollection: props.onOpenCollection,
    onBack: props.breadcrumb?.onBack,
  };
}

function initialStateFromProps(props: NormalizedFileListProps): FileListControllerState {
  return {
    ids: props.ids,
    total: props.total,
    loadingMore: props.loadingMore,
    filterKey: props.filterKey,
    remoteEnabled: props.remoteEnabled,
    availableTags: props.availableTags,
    availableAuthors: props.availableAuthors,
    sortByProp: props.sortBy,
    sortDescProp: props.sortDesc,
    applySort: props.applySort,
    conditionsProp: props.conditions,
    applyConditionsClientSide: props.applyConditionsClientSide,
    viewMode: props.viewMode,
    viewModeAvailable: props.viewModeAvailable,
    hasViewModeChange: !!props.onViewModeChange,
    collections: props.collections,
    breadcrumbLabel: props.breadcrumb?.label ?? null,
    canBulkDownload: !!props.onBulkDownload,
    canBulkDelete: !!props.onBulkDelete,
    canBulkClearCache: !!props.onBulkClearCache,
    internalSortBy: 'name',
    internalSortDesc: false,
    internalConditions: [],
    filterOpen: false,
    selectionMode: false,
    selectedIds: new Set(),
    visibleCount: 0,
    eligibleIds: [],
  };
}

export function sortByOf(state: FileListControllerState): SortKey {
  return state.sortByProp ?? state.internalSortBy;
}

export function sortDescOf(state: FileListControllerState): boolean {
  return state.sortDescProp ?? state.internalSortDesc;
}

export function conditionsOf(state: FileListControllerState): Condition[] {
  return state.conditionsProp ?? state.internalConditions;
}

export function showCollectionsOf(state: FileListControllerState): boolean {
  return state.viewMode !== 'flat' && state.breadcrumbLabel == null && state.collections != null;
}

function sameNumberArray(a: ReadonlyArray<number>, b: ReadonlyArray<number>): boolean {
  if (a === b) return true;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 1) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

function stateChanged(a: FileListControllerState, b: FileListControllerState): boolean {
  return (
    a.ids !== b.ids ||
    a.total !== b.total ||
    a.loadingMore !== b.loadingMore ||
    a.filterKey !== b.filterKey ||
    a.remoteEnabled !== b.remoteEnabled ||
    a.availableTags !== b.availableTags ||
    a.availableAuthors !== b.availableAuthors ||
    a.sortByProp !== b.sortByProp ||
    a.sortDescProp !== b.sortDescProp ||
    a.applySort !== b.applySort ||
    a.conditionsProp !== b.conditionsProp ||
    a.applyConditionsClientSide !== b.applyConditionsClientSide ||
    a.viewMode !== b.viewMode ||
    a.viewModeAvailable !== b.viewModeAvailable ||
    a.hasViewModeChange !== b.hasViewModeChange ||
    a.collections !== b.collections ||
    a.breadcrumbLabel !== b.breadcrumbLabel ||
    a.canBulkDownload !== b.canBulkDownload ||
    a.canBulkDelete !== b.canBulkDelete ||
    a.canBulkClearCache !== b.canBulkClearCache ||
    a.internalSortBy !== b.internalSortBy ||
    a.internalSortDesc !== b.internalSortDesc ||
    a.internalConditions !== b.internalConditions ||
    a.filterOpen !== b.filterOpen ||
    a.selectionMode !== b.selectionMode ||
    a.selectedIds !== b.selectedIds ||
    a.visibleCount !== b.visibleCount ||
    a.eligibleIds !== b.eligibleIds
  );
}

export function visibleEntriesFromInputs(
  ids: ReadonlyArray<number>,
  applyConditionsClientSide: boolean,
  conditions: Condition[],
  applySort: boolean,
  sortBy: SortKey,
  sortDesc: boolean,
  byId: Map<number, FileEntry>
): { importableEntries: FileEntry[]; visibleEntries: FileEntry[] } {
  // Resolve incoming ids to entries. Importability is judged by the real
  // filename's extension — but remote objects use an opaque, extension-less
  // storage path (the real name lives in `original_path`), so never drop a
  // remote library row: it's already in the catalog and its type can't be
  // reconstructed from the opaque path.
  const importableEntries: FileEntry[] = [];
  for (const id of ids) {
    const f = byId.get(id);
    if (!f) continue;
    if (f.storage_kind === 'remote' || isImportable(f.path)) {
      importableEntries.push(f);
    }
  }

  const filtered = applyConditionsClientSide
    ? applyConditions(importableEntries, conditions)
    : importableEntries;
  if (!applySort) return { importableEntries, visibleEntries: filtered };

  const visibleEntries = [...filtered].sort((a, b) => {
    const cmp = compareFiles(a, b, sortBy);
    return sortDesc ? -cmp : cmp;
  });
  return { importableEntries, visibleEntries };
}

export function createFileListController(initialProps: NormalizedFileListProps): FileListController {
  const store = new Store(initialStateFromProps(initialProps));
  const callbacks = { current: callbacksFromProps(initialProps) };
  const scrollContainerRef: RefObject<HTMLDivElement | null> = { current: null };
  const savedCollectionScrollRef = { current: 0 };

  const commit = (next: FileListControllerState) => {
    if (!stateChanged(store.state, next)) return;
    store.setState(() => next);
  };

  const updateInput = (nextProps: NormalizedFileListProps) => {
    callbacks.current = callbacksFromProps(nextProps);

    const prev = store.state;
    const filterChanged = prev.filterKey !== nextProps.filterKey;
    const wasDrilled = prev.breadcrumbLabel != null;
    const isDrilled = nextProps.breadcrumb != null;
    const drillChanged = wasDrilled !== isDrilled;
    const el = scrollContainerRef.current;

    if (filterChanged) {
      el?.scrollTo(0, 0);
      savedCollectionScrollRef.current = 0;
      if (nextProps.onConditionsChange) {
        nextProps.onConditionsChange([]);
      }
    } else if (drillChanged && el) {
      if (isDrilled) {
        savedCollectionScrollRef.current = el.scrollTop;
        el.scrollTo(0, 0);
      } else {
        el.scrollTo(0, savedCollectionScrollRef.current);
      }
    }

    const nextIds = sameNumberArray(prev.ids, nextProps.ids) ? prev.ids : nextProps.ids;
    const nextConditionsProp =
      filterChanged && nextProps.conditions != null ? [] : nextProps.conditions;

    const nextState: FileListControllerState = {
      ...prev,
      ids: nextIds,
      total: nextProps.total,
      loadingMore: nextProps.loadingMore,
      filterKey: nextProps.filterKey,
      remoteEnabled: nextProps.remoteEnabled,
      availableTags: nextProps.availableTags,
      availableAuthors: nextProps.availableAuthors,
      sortByProp: nextProps.sortBy,
      sortDescProp: nextProps.sortDesc,
      applySort: nextProps.applySort,
      conditionsProp: nextConditionsProp,
      applyConditionsClientSide: nextProps.applyConditionsClientSide,
      viewMode: nextProps.viewMode,
      viewModeAvailable: nextProps.viewModeAvailable,
      hasViewModeChange: !!nextProps.onViewModeChange,
      collections: nextProps.collections,
      breadcrumbLabel: nextProps.breadcrumb?.label ?? null,
      canBulkDownload: !!nextProps.onBulkDownload,
      canBulkDelete: !!nextProps.onBulkDelete,
      canBulkClearCache: !!nextProps.onBulkClearCache,
      internalConditions:
        filterChanged && nextProps.conditions == null ? [] : prev.internalConditions,
      filterOpen: filterChanged ? false : prev.filterOpen,
      selectionMode: filterChanged || drillChanged ? false : prev.selectionMode,
      selectedIds: filterChanged || drillChanged ? new Set<number>() : prev.selectedIds,
      visibleCount: filterChanged ? 0 : prev.visibleCount,
      eligibleIds: filterChanged ? [] : prev.eligibleIds,
    };

    commit(nextState);
  };

  const setSortBy = (key: SortKey) => {
    const state = store.state;
    if (callbacks.current.onSortChange) {
      callbacks.current.onSortChange(key, sortDescOf(state));
      return;
    }
    commit({ ...state, internalSortBy: key });
  };

  const setSortDesc = (desc: boolean) => {
    const state = store.state;
    if (callbacks.current.onSortChange) {
      callbacks.current.onSortChange(sortByOf(state), desc);
      return;
    }
    commit({ ...state, internalSortDesc: desc });
  };

  const setConditions: Dispatch<SetStateAction<Condition[]>> = (next) => {
    const state = store.state;
    const current = conditionsOf(state);
    const resolved = typeof next === 'function' ? next(current) : next;
    if (callbacks.current.onConditionsChange) {
      callbacks.current.onConditionsChange(resolved);
      return;
    }
    commit({ ...state, internalConditions: resolved });
  };

  const setFilterOpen = (open: boolean) => {
    const state = store.state;
    commit({ ...state, filterOpen: open });
  };

  const removeCondition = (id: string) => {
    setConditions((prev) => prev.filter((c) => c.id !== id));
  };

  const setViewMode = (mode: ViewMode) => {
    callbacks.current.onViewModeChange?.(mode);
  };

  const enterSelectionMode = () => {
    const state = store.state;
    commit({ ...state, selectionMode: true });
  };

  const exitSelectionMode = () => {
    const state = store.state;
    commit({ ...state, selectionMode: false, selectedIds: new Set() });
  };

  const clearSelection = () => {
    const state = store.state;
    commit({ ...state, selectedIds: new Set() });
  };

  const selectFirstN = (n: number) => {
    const state = store.state;
    commit({
      ...state,
      selectedIds: new Set(state.eligibleIds.slice(0, n)),
    });
  };

  const toggleSelection = (id: number) => {
    const state = store.state;
    const selectedIds = new Set(state.selectedIds);
    if (selectedIds.has(id)) selectedIds.delete(id);
    else selectedIds.add(id);
    commit({ ...state, selectedIds });
  };

  const setVisibleState = (visibleCount: number, eligibleIds: number[]) => {
    const state = store.state;
    const stableEligibleIds = sameNumberArray(state.eligibleIds, eligibleIds)
      ? state.eligibleIds
      : eligibleIds;
    commit({
      ...state,
      visibleCount,
      eligibleIds: stableEligibleIds,
    });
  };

  const handleCardClick = (file: FileEntry, inFlightAnyIds: Set<number>) => {
    const state = store.state;
    if (state.selectionMode) {
      if (!inFlightAnyIds.has(file.id)) toggleSelection(file.id);
    } else {
      callbacks.current.onFileClick?.(file);
    }
  };

  const handleBulkUpload = (inFlightUploadIds: Set<number>) => {
    const ids = Array.from(store.state.selectedIds).filter((id) => {
      const f = fileStore.state.byId.get(id);
      return !!f && f.storage_kind !== 'remote' && !inFlightUploadIds.has(id);
    });
    if (ids.length > 0) callbacks.current.onBulkUpload?.(ids);
    exitSelectionMode();
  };

  const handleBulkDownload = (inFlightDownloadIds: Set<number>) => {
    const ids = Array.from(store.state.selectedIds).filter((id) => {
      const f = fileStore.state.byId.get(id);
      return !!f && f.storage_kind === 'remote' && !inFlightDownloadIds.has(id);
    });
    if (ids.length > 0) callbacks.current.onBulkDownload?.(ids);
    exitSelectionMode();
  };

  const handleBulkDelete = (inFlightDeleteIds: Set<number>) => {
    const ids = Array.from(store.state.selectedIds).filter((id) => !inFlightDeleteIds.has(id));
    if (ids.length > 0) callbacks.current.onBulkDelete?.(ids);
    exitSelectionMode();
  };

  const handleBulkClearCache = () => {
    const ids = Array.from(store.state.selectedIds).filter((id) => {
      const f = fileStore.state.byId.get(id);
      return f && f.local_cache_path != null && f.local_cache_path !== '';
    });
    if (ids.length > 0) callbacks.current.onBulkClearCache?.(ids);
    exitSelectionMode();
  };

  return {
    store,
    callbacks,
    scrollContainerRef,
    updateInput,
    setSortBy,
    setSortDesc,
    setConditions,
    setFilterOpen,
    removeCondition,
    setViewMode,
    enterSelectionMode,
    exitSelectionMode,
    clearSelection,
    selectFirstN,
    toggleSelection,
    setVisibleState,
    handleCardClick,
    handleBulkUpload,
    handleBulkDownload,
    handleBulkDelete,
    handleBulkClearCache,
    handleLoadMore: () => callbacks.current.onLoadMore?.(),
    handleFileEdit: (file) => callbacks.current.onFileEdit?.(file),
    handleFileDelete: (file) => callbacks.current.onFileDelete?.(file),
    handleOpenCollection: (collection) => callbacks.current.onOpenCollection?.(collection),
    handleBack: () => callbacks.current.onBack?.(),
  };
}
