import { createFileRoute, Link } from '@tanstack/react-router';
import { useEffect, useRef, useState, useCallback, useMemo } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { FilePicker } from '@/components/FilePicker';
import { SearchBar } from '@/components/SearchBar';
import { FileList } from '@/components/FileList';
import { CategorySidebar } from '@/components/CategorySidebar';
import { ProcessingPipeline } from '@/components/ProcessingPipeline';
import { RemoteUploadProgressPanel } from '@/components/RemoteUploadProgress';
import { fetchFiles } from '@/stores';
import { fileStore } from '@/stores/fileStore';
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
  fileCreate,
  coverSet,
  storageGetPath,
  storageCheckAccess,
  remoteConfigGet,
  enqueueImport,
} from '@/lib/tauri';
import { Button } from '@/components/ui/button';
import { AlertCircle } from 'lucide-react';
import { SettingsDialog } from '@/components/SettingsDialog';
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
import { schemaForPath, isImportable } from '@/lib/fileKind';
import { useFileActions } from '@/hooks/useFileActions';
import type { FileEntry } from '@/types';

const EMPTY_FORM_VALUES: DynamicMetadataFormValues = {
  display_name: '',
  category_id: null,
  tag_ids: [],
  author_ids: [],
  metadata: [],
};

// Load the whole visible category in a single fetch so client-side filters
// (FileList header) operate over the complete row set instead of whatever's
// happened to scroll into view. Sized for realistic single-user libraries;
// the load-more virtualizer trigger remains as a safety net beyond this.
const FILES_PAGE_SIZE = 5000;

export const Route = createFileRoute('/')({
  component: HomePage,
});

function HomePage() {
  const [loadingMore, setLoadingMore] = useState(false);
  const [selectedCategoryId, setSelectedCategoryId] = useState<number | null>(null);
  // `searchQuery` is the live input value; `debouncedQuery` is the effective
  // value used for fetches, updated 300ms after the user stops typing so we
  // don't fire one backend request per keystroke.
  const [searchQuery, setSearchQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
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
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [pipelineOpen, setPipelineOpen] = useState(false);
  const [isDraggingFiles, setIsDraggingFiles] = useState(false);

  const [remoteEnabled, setRemoteEnabled] = useState(false);
  const uploadState = useRemoteUploadStore();

  // Debounce the search input. `searchQuery` reflects every keystroke;
  // `debouncedQuery` is what actually drives fetches, so typing quickly
  // collapses to a single request.
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(searchQuery.trim()), 300);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  const viewKey = useMemo(
    () => `home::category=${selectedCategoryId ?? 'none'}::query=${debouncedQuery}`,
    [selectedCategoryId, debouncedQuery]
  );

  const fetchView = useCallback(async (): Promise<ViewFetcherResult> => {
    if (selectedCategoryId === null) return { files: [], total: 0 };
    return await fetchFiles({
      category_id: selectedCategoryId,
      query: debouncedQuery || undefined,
      limit: FILES_PAGE_SIZE,
      offset: 0,
    });
  }, [selectedCategoryId, debouncedQuery]);

  const { ids, total, loading, reload, appendMore } = useView(viewKey, fetchView);

  const handleLoadMore = useCallback(async () => {
    if (loadingMore || selectedCategoryId === null) return;
    setLoadingMore(true);
    try {
      const result = await fetchFiles({
        category_id: selectedCategoryId,
        query: debouncedQuery || undefined,
        limit: FILES_PAGE_SIZE,
        offset: ids.length,
      });
      appendMore(result);
    } finally {
      setLoadingMore(false);
    }
  }, [loadingMore, selectedCategoryId, debouncedQuery, ids.length, appendMore]);

  // Shared dialog state + edit/delete handlers + supporting relation state.
  // The hook drives store mutations directly — it no longer needs a reload
  // callback. Tag/author rename events trigger a view refresh internally.
  const {
    categories,
    tags,
    authors,
    handleCategoryCreated,
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

  // Snap to a real category whenever the list changes:
  // - On first load, pick the first category (replaces the old "All Files"
  //   default of null).
  // - If the currently-selected category disappears (deletion, rename), fall
  //   back to the first remaining one. Empty list → null, the empty state
  //   handles it.
  useEffect(() => {
    if (categories.length === 0) {
      if (selectedCategoryId !== null) setSelectedCategoryId(null);
      return;
    }
    const stillExists = categories.some((c) => c.id === selectedCategoryId);
    if (!stillExists) {
      const first = categories[0];
      if (first) setSelectedCategoryId(first.id);
    }
  }, [categories, selectedCategoryId]);

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

  const handleSettingsOpenChange = useCallback(
    (open: boolean) => {
      setSettingsOpen(open);
      if (!open) {
        void checkStoragePath();
      }
    },
    [checkStoragePath]
  );

  const handleFilesSelected = (
    paths: string[],
    pathFolderRoots?: Record<string, string>
  ) => {
    // Folder picks trust the backend's own filtering: `list_files_in_folder`
    // already drops dotfiles and collapses image-only sub-trees into
    // directory paths (which `isImportable` would otherwise reject for
    // having no extension). For file picks and drag-drop, run the
    // extension filter so we can surface unsupported types up-front.
    const kept: string[] = [];
    const keptFolderRoots: Record<string, string> = {};
    const skipped: string[] = [];
    const fromFolderPick = pathFolderRoots && Object.keys(pathFolderRoots).length > 0;
    for (const p of paths) {
      if (fromFolderPick || isImportable(p)) {
        kept.push(p);
        if (pathFolderRoots && pathFolderRoots[p]) {
          keptFolderRoots[p] = pathFolderRoots[p];
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
            void handleFilesSelectedRef.current(p.paths);
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

  const handleBulkUpload = useCallback((fileIds: number[]) => {
    const fileNames = new Map<number, string>();
    const byId = fileStore.state.byId;
    for (const id of fileIds) {
      const f = byId.get(id);
      if (f) fileNames.set(id, f.display_name);
    }
    void enqueueUpload(fileIds, fileNames);
  }, []);

  return (
    <div className="flex h-screen bg-background">
      <CategorySidebar
        categories={categories}
        selectedCategoryId={selectedCategoryId}
        onCategorySelect={setSelectedCategoryId}
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <main className="flex-1 flex flex-col overflow-hidden relative">
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
              ids={ids}
              total={total}
              loadingMore={loadingMore}
              onLoadMore={handleLoadMore}
              filterKey={`${selectedCategoryId ?? 'none'}::${debouncedQuery}`}
              onFileClick={handleFileClick}
              onFileEdit={handleFileEdit}
              onFileDelete={handleFileDeleteClick}
              onBulkUpload={handleBulkUpload}
              remoteEnabled={remoteEnabled}
              availableTags={tags}
            />
          )}
        </div>

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
      </main>

      <SettingsDialog open={settingsOpen} onOpenChange={handleSettingsOpenChange} />

      <EditFileDialog
        open={editDialogOpen}
        onOpenChange={setEditDialogOpen}
        file={editingFile}
        categories={categories}
        tags={tags}
        authors={authors}
        onCategoryCreated={handleCategoryCreated}
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
          }
        }}
        paths={selectedFiles}
        pathFolderRoots={selectedPathFolderRoots}
        categories={categories}
        tags={tags}
        authors={authors}
        onCategoryCreated={handleCategoryCreated}
        onTagCreate={handleTagCreate}
        onAuthorCreate={handleAuthorCreate}
        onImportComplete={() => {
          setPipelineOpen(false);
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
              fields={schemaForPath(selectedFiles[0])?.formFields ?? []}
              categories={categories}
              tags={tags}
              authors={authors}
              onCategoryCreated={handleCategoryCreated}
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
                  fields={schemaForPath(selectedFiles[0])?.formFields ?? []}
                  categories={categories}
                  tags={tags}
                  authors={authors}
                  onCategoryCreated={handleCategoryCreated}
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
    </div>
  );
}
