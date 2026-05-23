import { createFileRoute, Link } from '@tanstack/react-router';
import { useEffect, useRef, useState, useCallback, useMemo } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { FilePicker } from '@/components/FilePicker';
import { SearchBar } from '@/components/SearchBar';
import { FileList } from '@/components/FileList';
import { ProcessingPipeline } from '@/components/ProcessingPipeline';
import { RemoteUploadProgressPanel } from '@/components/RemoteUploadProgress';
import { fetchFiles, type SortKey } from '@/stores';
import type { Condition } from '@/lib/filters';
import { fileStore } from '@/stores/fileStore';
import { setSettingsOpen, useAppState } from '@/stores/appStore';
import { useView, type ViewFetcherResult } from '@/hooks/useView';
import {
  useRemoteUploadStore,
  enqueueUpload,
  dismissPanel,
  clearCompleted,
  minimizePanel,
  expandPanel,
} from '@/stores/remoteUploadStore';
import {
  useRemoteDownloadStore,
  enqueueDownload,
  dismissDownloadPanel,
  clearCompletedDownloads,
  minimizeDownloadPanel,
  expandDownloadPanel,
} from '@/stores/remoteDownloadStore';
import {
  useRemoteDeleteStore,
  enqueueDelete,
  dismissDeletePanel,
  clearCompletedDeletes,
  minimizeDeletePanel,
  expandDeletePanel,
} from '@/stores/remoteDeleteStore';
import { RemoteDownloadProgressPanel } from '@/components/RemoteDownloadProgress';
import { RemoteDeleteProgressPanel } from '@/components/RemoteDeleteProgress';
import {
  cacheClear,
  fileCreate,
  coverSet,
  storageGetPath,
  storageCheckAccess,
  remoteConfigGet,
  enqueueImport,
  expandDropPaths,
  comicCollectionList,
  fileListByIds,
} from '@/lib/tauri';
import { hydrateFiles, patchFile } from '@/stores/fileStore';
import type { ComicCollection, ComicViewMode } from '@/types';
import { Button } from '@/components/ui/button';
import { AlertCircle } from 'lucide-react';
import { EditFileDialog } from '@/components/EditFileDialog';
import { ConfirmDeleteDialog } from '@/components/ConfirmDeleteDialog';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import {
  DynamicMetadataForm,
  type DynamicMetadataFormValues,
} from '@/components/DynamicMetadataForm';
import { schemaForCategoryId, schemaForPath, isImportable } from '@/lib/categorySchema';
import { resolveViewConfig } from '@/lib/categoryViewConfig';
import { useFileActions } from '@/hooks/useFileActions';
import type { FileEntry } from '@/types';

const EMPTY_FORM_VALUES: DynamicMetadataFormValues = {
  display_name: '',
  category_id: null,
  tag_ids: [],
  author_ids: [],
  metadata: [],
};

// First fetch fills the viewport (a few rows worth), then the virtualizer's
// load-more trigger streams the rest as the user scrolls. Larger pages
// would let client-side filter pills operate over more rows up-front, but
// at the cost of a multi-MB initial IPC payload that delays first paint.
const FILES_PAGE_SIZE = 200;

export const Route = createFileRoute('/')({
  component: HomePage,
});

function HomePage() {
  const [loadingMore, setLoadingMore] = useState(false);
  const selectedCategoryId = useAppState((s) => s.selectedCategoryId);
  const settingsOpen = useAppState((s) => s.settingsOpen);
  // `searchQuery` is the live input value; `debouncedQuery` is the effective
  // value used for fetches, updated 300ms after the user stops typing so we
  // don't fire one backend request per keystroke.
  const [searchQuery, setSearchQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  // Sort lives here (not in FileList) because it goes into the SQL ORDER BY
  // and the view cache key — the server returns rows already in this order
  // and the load-more pagination has to keep that ordering stable across
  // pages. Defaults match the prior client-side defaults: name ascending.
  const [sortBy, setSortBy] = useState<SortKey>('name');
  const [sortDesc, setSortDesc] = useState(false);
  const handleSortChange = useCallback((next: SortKey, desc: boolean) => {
    setSortBy(next);
    setSortDesc(desc);
  }, []);
  // Same reasoning as sort: filter pills become part of the SQL WHERE so
  // results match across paginated load-more requests. Ownership belongs to
  // Library; FileList receives them as controlled props.
  const [conditions, setConditions] = useState<Condition[]>([]);
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
  // Map of every imported path → the folder the user picked it from. Empty
  // for plain file picks. Used to drive per-comic author hints and the
  // post-import empty-dir cleanup, both of which are scoped per root.
  const [selectedPathFolderRoots, setSelectedPathFolderRoots] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [formValues, setFormValues] = useState<DynamicMetadataFormValues>(EMPTY_FORM_VALUES);
  const [storagePathConfigured, setStoragePathConfigured] = useState<boolean | null>(null);
  const [storagePathAccessible, setStoragePathAccessible] = useState(true);
  const [pipelineOpen, setPipelineOpen] = useState(false);
  // Minimize collapses the import dialog into a corner pill while leaving
  // its state + listeners alive. Reset to false on every fresh open so
  // re-opening the picker doesn't surprise the user with a hidden modal.
  const [pipelineMinimized, setPipelineMinimized] = useState(false);
  const [isDraggingFiles, setIsDraggingFiles] = useState(false);

  const [remoteEnabled, setRemoteEnabled] = useState(false);
  const uploadState = useRemoteUploadStore();
  const downloadState = useRemoteDownloadStore();
  const deleteState = useRemoteDeleteStore();

  // Debounce the search input. `searchQuery` reflects every keystroke;
  // `debouncedQuery` is what actually drives fetches, so typing quickly
  // collapses to a single request.
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(searchQuery.trim()), 300);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  // Stable string key for the view cache. Conditions hash by JSON because
  // the array identity churns on every keystroke in the editor; the JSON
  // string only changes when the predicate itself does.
  const conditionsKey = useMemo(() => JSON.stringify(conditions), [conditions]);

  const viewKey = useMemo(
    () =>
      `home::category=${selectedCategoryId ?? 'none'}::query=${debouncedQuery}::sort=${sortBy}:${sortDesc ? 'desc' : 'asc'}::filters=${conditionsKey}`,
    [selectedCategoryId, debouncedQuery, sortBy, sortDesc, conditionsKey]
  );

  const fetchView = useCallback(async (): Promise<ViewFetcherResult> => {
    if (selectedCategoryId === null) return { files: [], total: 0 };
    return await fetchFiles({
      category_id: selectedCategoryId,
      query: debouncedQuery || undefined,
      sort_by: sortBy,
      sort_desc: sortDesc,
      conditions,
      limit: FILES_PAGE_SIZE,
      offset: 0,
    });
  }, [selectedCategoryId, debouncedQuery, sortBy, sortDesc, conditions]);

  const { ids, total, loading, reload, appendMore } = useView(viewKey, fetchView);

  const handleLoadMore = useCallback(async () => {
    if (loadingMore || selectedCategoryId === null) return;
    setLoadingMore(true);
    try {
      const result = await fetchFiles({
        category_id: selectedCategoryId,
        query: debouncedQuery || undefined,
        sort_by: sortBy,
        sort_desc: sortDesc,
        conditions,
        limit: FILES_PAGE_SIZE,
        offset: ids.length,
      });
      appendMore(result);
    } finally {
      setLoadingMore(false);
    }
  }, [loadingMore, selectedCategoryId, debouncedQuery, sortBy, sortDesc, conditions, ids.length, appendMore]);

  // Shared dialog state + edit/delete handlers + supporting relation state.
  // The hook drives store mutations directly — it no longer needs a reload
  // callback. Tag/author rename events trigger a view refresh internally.
  const {
    categories,
    tags,
    authors,
    handleTagCreate,
    handleAuthorCreate,
    editingFile,
    editDialogOpen,
    setEditDialogOpen,
    handleFileEdit,
    handleFileSave,
    deletingFile,
    deleteDialogOpen,
    setDeleteDialogOpen,
    handleFileDeleteClick,
    handleFileDeleteConfirm,
  } = useFileActions();

  // ── Comic collections view ──────────────────────────────────────────────
  // The view-mode toggle in the FileList header swaps the per-file grid for
  // grouped collection cards (one per author or per derived series prefix).
  // Only meaningful when the active category uses the comic schema; for
  // novels we hide the toggle entirely. The block lives after
  // `useFileActions()` so `categories` is in scope when we resolve the
  // active category's schema.
  const [viewMode, setViewMode] = useState<ComicViewMode>('flat');
  const [collections, setCollections] = useState<ComicCollection[] | null>(null);
  const [expandedCollection, setExpandedCollection] = useState<ComicCollection | null>(null);
  const selectedCategorySchema = useMemo(
    () => schemaForCategoryId(selectedCategoryId, categories),
    [selectedCategoryId, categories]
  );
  const isComicCategory = selectedCategorySchema.slug === 'comic';

  // Snap the view-mode toggle back to flat whenever the user switches to a
  // non-comic category. Without this, walking out of a comic category while
  // in 'author' mode would briefly try to render collection cards for a
  // category that has no collections endpoint shape.
  useEffect(() => {
    if (!isComicCategory && viewMode !== 'flat') {
      setViewMode('flat');
    }
  }, [isComicCategory, viewMode]);

  // Reset drill-down whenever the grouping axis or category changes — the
  // previous expandedCollection's `file_ids` would otherwise dangle into the
  // new context.
  useEffect(() => {
    setExpandedCollection(null);
  }, [viewMode, selectedCategoryId]);

  // Fetch collections when the user picks a non-flat view. Empty array is a
  // valid result ("no multi-member groups in scope"); `null` means we have
  // not yet fetched, so the body can show a quick loading hint instead of
  // the "no series detected" empty state.
  useEffect(() => {
    if (viewMode === 'flat' || !isComicCategory || selectedCategoryId == null) {
      setCollections(null);
      return;
    }
    let cancelled = false;
    setCollections(null);
    comicCollectionList({
      mode: viewMode,
      category_id: selectedCategoryId,
    })
      .then((result) => {
        if (!cancelled) setCollections(result);
      })
      .catch((err) => {
        console.error('comic_collection_list failed:', err);
        if (!cancelled) setCollections([]);
      });
    return () => {
      cancelled = true;
    };
  }, [viewMode, isComicCategory, selectedCategoryId]);

  // FileList ids: pass the drill-down's `file_ids` when one is expanded,
  // otherwise the normal page of ids from `useView`. `byId` is hydrated by
  // the effect below when a collection is opened — without that, files
  // beyond the flat view's first paginated page would resolve to
  // `undefined` and silently disappear from the grid.
  const fileListIds = expandedCollection ? expandedCollection.file_ids : ids;
  const fileListTotal = expandedCollection ? expandedCollection.file_ids.length : total;

  // Hydrate fileStore.byId with the drilled-into collection's rows. The
  // backend's collection list returns ids only; the flat-view pagination
  // (200/page) may not have loaded those rows, so without this fetch the
  // FileList grid would render a wrong (truncated) count or appear empty
  // even though the collection has members.
  useEffect(() => {
    if (!expandedCollection) return;
    let cancelled = false;
    fileListByIds(expandedCollection.file_ids)
      .then((files) => {
        if (!cancelled) hydrateFiles(files);
      })
      .catch((err) => {
        console.error('file_list_by_ids failed:', err);
      });
    return () => {
      cancelled = true;
    };
  }, [expandedCollection]);

  // Re-seed sort + filter conditions from the active category's stored
  // view_config whenever the user switches categories (or the user edits
  // the category's view_config from the Categories page and comes back).
  // Within a category, transient sort/filter changes stay in place — only
  // a category switch or a config edit triggers re-seeding. Reuses the
  // `categories` array already resolved above for the comic-schema check
  // so we don't search it twice per render.
  const currentCategory = useMemo(
    () => categories.find((c) => c.id === selectedCategoryId) ?? null,
    [categories, selectedCategoryId]
  );
  const seedKey = `${selectedCategoryId ?? 'none'}::${currentCategory?.view_config ?? ''}`;
  const prevSeedKeyRef = useRef<string | null>(null);
  useEffect(() => {
    if (categories.length === 0) return;
    if (prevSeedKeyRef.current === seedKey) return;
    prevSeedKeyRef.current = seedKey;
    const resolved = resolveViewConfig(currentCategory);
    setSortBy(resolved.sortBy);
    setSortDesc(resolved.sortDesc);
    setConditions(resolved.conditions);
    // Only apply a non-flat view mode for comic categories — author /
    // name_prefix collapse the grid into collection cards and rely on
    // the comic-only `comicCollectionList` endpoint. For novels the
    // toggle is hidden, so a stored 'author' default would silently
    // never activate; force 'flat' to keep the rendered state honest.
    setViewMode(
      resolved.viewMode !== 'flat' && currentCategory?.schema_slug === 'comic'
        ? resolved.viewMode
        : 'flat'
    );
  }, [seedKey, currentCategory, categories.length]);

  const checkStoragePath = useCallback(async () => {
    const path = await storageGetPath();
    if (path && path !== '') {
      const accessible = await storageCheckAccess();
      setStoragePathConfigured(true);
      setStoragePathAccessible(accessible);
    } else {
      setStoragePathConfigured(false);
      setStoragePathAccessible(true);
    }
  }, []);

  useEffect(() => {
    void checkStoragePath();
    remoteConfigGet().then(cfg => setRemoteEnabled(cfg.enabled)).catch(() => {});
  }, [checkStoragePath]);

  // The Settings dialog now lives in the app shell, so we react to its
  // close via the shared store: re-validate storage path whenever it
  // transitions from open to closed.
  const prevSettingsOpenRef = useRef(false);
  useEffect(() => {
    if (prevSettingsOpenRef.current && !settingsOpen) {
      void checkStoragePath();
    }
    prevSettingsOpenRef.current = settingsOpen;
  }, [settingsOpen, checkStoragePath]);

  const handleFilesSelected = (
    paths: string[],
    pathFolderRoots?: Record<string, string>
  ) => {
    // Folder-scanned paths trust the backend's own filtering:
    // `list_files_in_folder` / `expand_drop_paths` already drop dotfiles
    // and collapse image-only sub-trees into directory paths (which
    // `isImportable` would otherwise reject for having no extension).
    // Standalone paths — plain file picks and file-only drops — still
    // go through the extension filter so we surface unsupported types
    // up-front. The check is per-path because a single drag-drop can
    // mix folder-scanned entries with standalone files.
    const kept: string[] = [];
    const keptFolderRoots: Record<string, string> = {};
    const skipped: string[] = [];
    for (const p of paths) {
      const folderRoot = pathFolderRoots?.[p];
      if (folderRoot || isImportable(p)) {
        kept.push(p);
        if (folderRoot) {
          keptFolderRoots[p] = folderRoot;
        }
      } else {
        skipped.push(p);
      }
    }

    if (skipped.length > 0) {
      alert(
        `Skipped ${skipped.length} unsupported file${skipped.length === 1 ? '' : 's'}. ` +
          `Only .txt and comic archives (.cbz / .zip / .cbr / .rar) are supported.`
      );
    }

    if (kept.length === 0) return;

    // Producer-consumer: appending instead of replacing means new picks while
    // the dialog is open join the in-flight queue instead of clobbering it.
    // The backend worker drains everything serially.
    setSelectedFiles((prev) => {
      const seen = new Set(prev);
      const additions = kept.filter((p) => !seen.has(p));
      return additions.length === 0 ? prev : [...prev, ...additions];
    });
    setSelectedPathFolderRoots((prev) => ({ ...prev, ...keptFolderRoots }));
    setFormValues(EMPTY_FORM_VALUES);
    setAddDialogOpen(false);
    setPipelineMinimized(false);
    setPipelineOpen(true);

    // Push the new paths to the import worker. Returns immediately; per-file
    // events drive the dialog state via the existing listeners. Logging the
    // failure path keeps the dev console useful — placeholder items would
    // otherwise sit in `pending` forever with no diagnostic.
    enqueueImport(kept, keptFolderRoots).catch((err) => {
      console.error('enqueue_import failed:', err);
    });
  };

  const handleAddFile = async () => {
    if (saving) return;
    setSaving(true);
    try {
      for (const path of selectedFiles) {
        const defaultName = path.substring(Math.max(0, path.lastIndexOf('/') + 1)) || path;
        const display_name = selectedFiles.length === 1 ? formValues.display_name : defaultName;

        const result = await fileCreate({
          path,
          display_name,
          category_id: formValues.category_id,
          tag_ids: formValues.tag_ids,
          author_ids: formValues.author_ids,
          metadata: formValues.metadata,
        });

        if (formValues.cover_data && result.id) {
          const binaryString = atob(formValues.cover_data);
          const bytes = new Uint8Array(binaryString.length);
          for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
          }
          await coverSet(result.id, Array.from(bytes));
        }
      }
      setAddDialogOpen(false);
      setSelectedFiles([]);
      setFormValues(EMPTY_FORM_VALUES);
      void reload();
    } catch (error) {
      console.error('Failed to add file:', error);
      alert(`Failed to add file: ${error}`);
    }
    setSaving(false);
  };

  const handleFileClick = (file: FileEntry) => {
    console.log('File clicked:', file);
  };

  // Tauri webview drag-drop. The HTML5 DnD API doesn't surface real OS file
  // paths inside a Tauri webview, so we listen at the window level and route
  // dropped paths through the same import flow as FilePicker.
  const handleFilesSelectedRef = useRef(handleFilesSelected);
  const storageReadyRef = useRef(storagePathConfigured !== false);

  useEffect(() => {
    handleFilesSelectedRef.current = handleFilesSelected;
    storageReadyRef.current = storagePathConfigured !== false;
  });

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === 'enter' || p.type === 'over') {
          if (storageReadyRef.current) setIsDraggingFiles(true);
        } else if (p.type === 'leave') {
          setIsDraggingFiles(false);
        } else if (p.type === 'drop') {
          setIsDraggingFiles(false);
          if (storageReadyRef.current && p.paths.length > 0) {
            // Resolve dropped paths first: any folder gets walked the
            // same way `FilePicker` walks an explicit folder pick, so a
            // mixed drop of files + folders feeds the import worker the
            // same shape both paths produce.
            void expandDropPaths(p.paths)
              .then(({ files, path_folder_roots, empty_folders }) => {
                if (empty_folders.length > 0) {
                  alert(
                    empty_folders.length === 1
                      ? 'The dropped folder is empty.'
                      : `${empty_folders.length} dropped folders are empty.`
                  );
                }
                if (files.length === 0) return;
                handleFilesSelectedRef.current(files, path_folder_roots);
              })
              .catch((err) => {
                console.error('expand_drop_paths failed:', err);
                alert(`Failed to read dropped items: ${String(err)}`);
              });
          }
        }
      })
      .then((un) => {
        if (cancelled) un();
        else unlisten = un;
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const namesFor = useCallback((fileIds: number[]) => {
    const fileNames = new Map<number, string>();
    const byId = fileStore.state.byId;
    for (const id of fileIds) {
      const f = byId.get(id);
      if (f) fileNames.set(id, f.display_name);
    }
    return fileNames;
  }, []);

  const handleBulkUpload = useCallback(
    (fileIds: number[]) => {
      void enqueueUpload(fileIds, namesFor(fileIds));
    },
    [namesFor]
  );

  const handleBulkDownload = useCallback(
    (fileIds: number[]) => {
      void enqueueDownload(fileIds, namesFor(fileIds));
    },
    [namesFor]
  );

  const handleBulkDelete = useCallback(
    (fileIds: number[]) => {
      void enqueueDelete(fileIds, namesFor(fileIds));
    },
    [namesFor]
  );

  // Clear-cache: per-file IPC is cheap (disk unlink + UPDATE). Parallel
  // via Promise.allSettled and patch each row's `local_cache_path` to
  // null on success so the grid badges flip without a full reload.
  const handleBulkClearCache = useCallback(
    async (fileIds: number[]) => {
      await Promise.allSettled(
        fileIds.map(async (id) => {
          try {
            await cacheClear(id);
            patchFile(id, { local_cache_path: null });
          } catch (err) {
            console.error(`Failed to clear cache for file ${id}:`, err);
          }
        })
      );
    },
    []
  );

  return (
    <>
      {isDraggingFiles && (
        <div
          className="pointer-events-none absolute inset-0 z-50 flex items-center justify-center bg-primary/5 border-2 border-dashed border-primary rounded-lg m-4"
          aria-hidden="true"
        >
          <p className="text-sm font-medium text-primary">
            Drop files to import
          </p>
        </div>
      )}
      <div
        className="flex items-end justify-between px-8 pt-14 pb-5 border-b border-border"
        data-tauri-drag-region
      >
        <div className="flex items-baseline gap-3">
          <h1 className="text-3xl text-foreground">Library</h1>
          <span
            className="font-serif-italic text-sm text-muted-foreground"
            aria-label={`${total} files`}
          >
            — {total} {total === 1 ? 'volume' : 'volumes'}
          </span>
        </div>
        <div className="flex items-center gap-3">
          <div className="w-64">
            <SearchBar
              value={searchQuery}
              onChange={setSearchQuery}
              onSearch={setDebouncedQuery}
              placeholder="Search title, path…"
            />
          </div>
          <FilePicker
            onFilesSelected={handleFilesSelected}
            disabled={storagePathConfigured === false}
          />
        </div>
      </div>

      <div className="flex-1 flex flex-col overflow-hidden px-8 py-6">
        {storagePathConfigured === false && (
          <div className="mb-6 p-4 bg-secondary/50 rounded flex items-center gap-3">
            <AlertCircle className="h-4 w-4 text-muted-foreground shrink-0" />
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-foreground">Storage path not configured</p>
              <p className="text-xs text-muted-foreground mt-0.5">
                Select a storage folder to start adding files.
              </p>
            </div>
            <Button variant="secondary" size="sm" onClick={() => setSettingsOpen(true)}>
              Configure
            </Button>
          </div>
        )}

        {storagePathConfigured === true && !storagePathAccessible && (
          <div className="mb-6 p-4 bg-destructive/5 rounded flex items-center gap-3">
            <AlertCircle className="h-4 w-4 text-destructive shrink-0" />
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-foreground">Storage path inaccessible</p>
              <p className="text-xs text-muted-foreground mt-0.5">
                The storage folder cannot be accessed.
              </p>
            </div>
            <Button variant="secondary" size="sm" onClick={() => setSettingsOpen(true)}>
              Reconfigure
            </Button>
          </div>
        )}

        {categories.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="max-w-sm text-center space-y-3">
              <p className="text-sm font-medium text-foreground">
                No categories yet
              </p>
              <p className="text-xs text-muted-foreground">
                Create a category to start organizing your library — comics,
                novels, or anything else.
              </p>
              <Link to="/categories">
                <Button size="sm" variant="secondary">
                  Manage Categories
                </Button>
              </Link>
            </div>
          </div>
        ) : loading ? (
          <div className="flex items-center justify-center h-32">
            <p className="text-sm text-muted-foreground">Loading...</p>
          </div>
        ) : (
          <FileList
            ids={fileListIds}
            total={fileListTotal}
            loadingMore={loadingMore}
            onLoadMore={handleLoadMore}
            filterKey={`${selectedCategoryId ?? 'none'}::${debouncedQuery}::${viewMode}`}
            onFileClick={handleFileClick}
            onFileEdit={handleFileEdit}
            onFileDelete={handleFileDeleteClick}
            onBulkUpload={handleBulkUpload}
            onBulkDownload={handleBulkDownload}
            onBulkDelete={handleBulkDelete}
            onBulkClearCache={handleBulkClearCache}
            remoteEnabled={remoteEnabled}
            availableTags={tags}
            availableAuthors={authors}
            sortBy={sortBy}
            sortDesc={sortDesc}
            onSortChange={handleSortChange}
            applySort={!!expandedCollection}
            conditions={conditions}
            onConditionsChange={setConditions}
            applyConditionsClientSide={!!expandedCollection}
            viewMode={viewMode}
            onViewModeChange={setViewMode}
            viewModeAvailable={isComicCategory}
            collections={collections ?? undefined}
            onOpenCollection={(c) => setExpandedCollection(c)}
            breadcrumb={
              expandedCollection
                ? {
                    label: expandedCollection.title,
                    onBack: () => setExpandedCollection(null),
                  }
                : null
            }
          />
        )}
      </div>

      <div className="fixed bottom-4 right-4 z-50 flex flex-col-reverse gap-2 items-end">
        {uploadState.showPanel && (
          <RemoteUploadProgressPanel
            uploads={uploadState.uploads}
            minimized={uploadState.minimized}
            onMinimize={minimizePanel}
            onExpand={expandPanel}
            onDismiss={dismissPanel}
            onClearCompleted={clearCompleted}
          />
        )}
        {downloadState.showPanel && (
          <RemoteDownloadProgressPanel
            downloads={downloadState.downloads}
            minimized={downloadState.minimized}
            onMinimize={minimizeDownloadPanel}
            onExpand={expandDownloadPanel}
            onDismiss={dismissDownloadPanel}
            onClearCompleted={clearCompletedDownloads}
          />
        )}
        {deleteState.showPanel && (
          <RemoteDeleteProgressPanel
            deletes={deleteState.deletes}
            minimized={deleteState.minimized}
            onMinimize={minimizeDeletePanel}
            onExpand={expandDeletePanel}
            onDismiss={dismissDeletePanel}
            onClearCompleted={clearCompletedDeletes}
          />
        )}
      </div>

      <EditFileDialog
        open={editDialogOpen}
        onOpenChange={setEditDialogOpen}
        file={editingFile}
        categories={categories}
        tags={tags}
        authors={authors}
        onTagCreate={handleTagCreate}
        onAuthorCreate={handleAuthorCreate}
        onSave={handleFileSave}
      />

      <ConfirmDeleteDialog
        open={deleteDialogOpen}
        onOpenChange={setDeleteDialogOpen}
        fileName={deletingFile?.display_name ?? ''}
        onConfirm={handleFileDeleteConfirm}
      />

      <ProcessingPipeline
        open={pipelineOpen}
        onOpenChange={(open) => {
          setPipelineOpen(open);
          if (!open) {
            // Clearing on close ensures the next open starts fresh — without
            // it, the dialog would re-init from the previous session's paths.
            setSelectedFiles([]);
            setSelectedPathFolderRoots({});
            setPipelineMinimized(false);
          }
        }}
        minimized={pipelineMinimized}
        onMinimize={() => setPipelineMinimized(true)}
        onExpand={() => setPipelineMinimized(false)}
        paths={selectedFiles}
        pathFolderRoots={selectedPathFolderRoots}
        categories={categories}
        tags={tags}
        authors={authors}
        onTagCreate={handleTagCreate}
        onAuthorCreate={handleAuthorCreate}
        onImportComplete={() => {
          setPipelineOpen(false);
          setPipelineMinimized(false);
          setSelectedFiles([]);
          setSelectedPathFolderRoots({});
          void reload();
        }}
      />

      <Dialog open={addDialogOpen} onOpenChange={setAddDialogOpen}>
        <DialogContent className="max-w-md max-h-[90vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>Add Files</DialogTitle>
          </DialogHeader>
          {selectedFiles.length === 1 ? (
            <DynamicMetadataForm
              values={formValues}
              onChange={setFormValues}
              schema={schemaForPath(selectedFiles[0]) ?? undefined}
              categories={categories}
              tags={tags}
              authors={authors}
                    onTagCreate={handleTagCreate}
              onAuthorCreate={handleAuthorCreate}
            />
          ) : (
            <div className="py-4">
              <p className="text-sm text-muted-foreground">Adding {selectedFiles.length} files.</p>
              <div className="mt-4">
                <DynamicMetadataForm
                  values={formValues}
                  onChange={setFormValues}
                  schema={schemaForPath(selectedFiles[0]) ?? undefined}
                  categories={categories}
                  tags={tags}
                  authors={authors}
                            onTagCreate={handleTagCreate}
                  onAuthorCreate={handleAuthorCreate}
                />
              </div>
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setAddDialogOpen(false)}>
              Cancel
            </Button>
            <Button onClick={handleAddFile} disabled={saving}>
              {saving ? 'Adding...' : 'Add'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
