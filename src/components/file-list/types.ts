import type { Dispatch, RefObject, SetStateAction } from 'react';
import type { Store } from '@tanstack/react-store';
import type { SortKey } from '@/components/FileListHeader';
import type { Condition } from '@/lib/filters';
import type { Author, Collection, ViewMode, FileEntry, Tag } from '@/types';

export type Breadcrumb = { label: string; onBack: () => void };

export interface FileListProps {
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
  /** Rendered in the header as a back chip — used during the drill-down
   *  from a collection card into its constituent files. */
  breadcrumb?: Breadcrumb | null;
}

export interface NormalizedFileListProps {
  ids: number[];
  total?: number;
  loadingMore: boolean;
  onLoadMore?: () => void;
  filterKey: string | number | null;
  onFileClick?: (file: FileEntry) => void;
  onFileEdit?: (file: FileEntry) => void;
  onFileDelete?: (file: FileEntry) => void;
  onBulkUpload?: (fileIds: number[]) => void;
  onBulkDownload?: (fileIds: number[]) => void;
  onBulkDelete?: (fileIds: number[]) => void;
  onBulkClearCache?: (fileIds: number[]) => void;
  remoteEnabled: boolean;
  availableTags: ReadonlyArray<Tag>;
  availableAuthors: ReadonlyArray<Author>;
  sortBy?: SortKey;
  sortDesc?: boolean;
  onSortChange?: (sortBy: SortKey, sortDesc: boolean) => void;
  applySort: boolean;
  conditions?: Condition[];
  onConditionsChange?: (conditions: Condition[]) => void;
  applyConditionsClientSide: boolean;
  viewMode: ViewMode;
  onViewModeChange?: (mode: ViewMode) => void;
  viewModeAvailable: boolean;
  collections?: Collection[];
  onOpenCollection?: (c: Collection) => void;
  breadcrumb: Breadcrumb | null;
}

export interface FileListCallbacks {
  onLoadMore?: () => void;
  onFileClick?: (file: FileEntry) => void;
  onFileEdit?: (file: FileEntry) => void;
  onFileDelete?: (file: FileEntry) => void;
  onBulkUpload?: (fileIds: number[]) => void;
  onBulkDownload?: (fileIds: number[]) => void;
  onBulkDelete?: (fileIds: number[]) => void;
  onBulkClearCache?: (fileIds: number[]) => void;
  onSortChange?: (sortBy: SortKey, sortDesc: boolean) => void;
  onConditionsChange?: (conditions: Condition[]) => void;
  onViewModeChange?: (mode: ViewMode) => void;
  onOpenCollection?: (c: Collection) => void;
  onBack?: () => void;
}

export interface FileListControllerState {
  ids: number[];
  total?: number;
  loadingMore: boolean;
  filterKey: string | number | null;
  remoteEnabled: boolean;
  availableTags: ReadonlyArray<Tag>;
  availableAuthors: ReadonlyArray<Author>;
  sortByProp?: SortKey;
  sortDescProp?: boolean;
  applySort: boolean;
  conditionsProp?: Condition[];
  applyConditionsClientSide: boolean;
  viewMode: ViewMode;
  viewModeAvailable: boolean;
  hasViewModeChange: boolean;
  collections?: Collection[];
  breadcrumbLabel: string | null;
  canBulkDownload: boolean;
  canBulkDelete: boolean;
  canBulkClearCache: boolean;
  internalSortBy: SortKey;
  internalSortDesc: boolean;
  internalConditions: Condition[];
  filterOpen: boolean;
  selectionMode: boolean;
  selectedIds: Set<number>;
  visibleCount: number;
  eligibleIds: number[];
}

export interface FileListController {
  store: Store<FileListControllerState>;
  callbacks: { current: FileListCallbacks };
  scrollContainerRef: RefObject<HTMLDivElement | null>;
  updateInput: (next: NormalizedFileListProps) => void;
  setSortBy: (key: SortKey) => void;
  setSortDesc: (desc: boolean) => void;
  setConditions: Dispatch<SetStateAction<Condition[]>>;
  setFilterOpen: (open: boolean) => void;
  removeCondition: (id: string) => void;
  setViewMode: (mode: ViewMode) => void;
  enterSelectionMode: () => void;
  exitSelectionMode: () => void;
  clearSelection: () => void;
  selectFirstN: (n: number) => void;
  toggleSelection: (id: number) => void;
  setVisibleState: (visibleCount: number, eligibleIds: number[]) => void;
  handleCardClick: (file: FileEntry, inFlightAnyIds: Set<number>) => void;
  handleBulkUpload: (inFlightUploadIds: Set<number>) => void;
  handleBulkDownload: (inFlightDownloadIds: Set<number>) => void;
  handleBulkDelete: (inFlightDeleteIds: Set<number>) => void;
  handleBulkClearCache: () => void;
  handleLoadMore: () => void;
  handleFileEdit: (file: FileEntry) => void;
  handleFileDelete: (file: FileEntry) => void;
  handleOpenCollection: (collection: Collection) => void;
  handleBack: () => void;
}
