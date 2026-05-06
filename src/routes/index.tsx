import { createFileRoute, Link } from '@tanstack/react-router';
import { useEffect, useRef, useState, useCallback } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { FilePicker } from '@/components/FilePicker';
import { SearchBar } from '@/components/SearchBar';
import { FileList } from '@/components/FileList';
import { CategorySidebar } from '@/components/CategorySidebar';
import { ProcessingPipeline } from '@/components/ProcessingPipeline';
import { RemoteUploadProgressPanel } from '@/components/RemoteUploadProgress';
import { fetchFiles } from '@/stores';
import { useRemoteUploadStore, startUpload, dismissPanel } from '@/stores/remoteUploadStore';
import {
  fileCreate,
  coverSet,
  storageGetPath,
  storageCheckAccess,
  settingsGet,
  remoteConfigGet,
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
import { schemaForPath } from '@/lib/fileKind';
import { useFileActions } from '@/hooks/useFileActions';
import type { FileEntry } from '@/types';

const EMPTY_FORM_VALUES: DynamicMetadataFormValues = {
  display_name: '',
  category_id: null,
  tag_ids: [],
  author_ids: [],
  metadata: [],
};

// How many files to fetch per request. Loaded rows accumulate in memory,
// client-side pagination (FileList) pages within whatever is loaded.
const FILES_PAGE_SIZE = 100;

export const Route = createFileRoute('/')({
  component: HomePage,
});

function HomePage() {
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [selectedCategoryId, setSelectedCategoryId] = useState<number | null>(null);
  // `searchQuery` is the live input value; `debouncedQuery` is the effective
  // value used for fetches, updated 300ms after the user stops typing so we
  // don't fire one backend request per keystroke.
  const [searchQuery, setSearchQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
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

  const loadFiles = useCallback(async () => {
    if (selectedCategoryId === null) {
      setFiles([]);
      setTotal(0);
      setLoading(false);
      return;
    }
    setLoading(true);
    const result = await fetchFiles({
      category_id: selectedCategoryId,
      query: debouncedQuery || undefined,
      limit: FILES_PAGE_SIZE,
      offset: 0,
    });
    setFiles(result.files);
    setTotal(result.total);
    setLoading(false);
  }, [selectedCategoryId, debouncedQuery]);

  const handleLoadMore = useCallback(async () => {
    if (loadingMore) return;
    setLoadingMore(true);
    const result = await fetchFiles({
      category_id: selectedCategoryId,
      query: debouncedQuery || undefined,
      limit: FILES_PAGE_SIZE,
      offset: files.length,
    });
    // Append; keep `total` in sync in case the DB changed between requests.
    setFiles((prev) => [...prev, ...result.files]);
    setTotal(result.total);
    setLoadingMore(false);
  }, [loadingMore, selectedCategoryId, debouncedQuery, files.length]);

  // Shared dialog state + edit/delete handlers + supporting relation state.
  // The hook also subscribes to tag/author change events and refreshes its
  // own state plus calls `loadFiles` so tag renames/deletes flow through.
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
  } = useFileActions(loadFiles);

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
    void loadFiles();
    void checkStoragePath();
    remoteConfigGet().then(cfg => setRemoteEnabled(cfg.enabled)).catch(() => {});
  }, [loadFiles, checkStoragePath]);

  const handleSettingsOpenChange = useCallback(
    (open: boolean) => {
      setSettingsOpen(open);
      if (!open) {
        void checkStoragePath();
      }
    },
    [checkStoragePath]
  );

  const handleFilesSelected = async (paths: string[]) => {
    const [epubRaw, pdfRaw] = await Promise.all([
      settingsGet('process_novel_epub'),
      settingsGet('process_novel_pdf'),
    ]);
    const parse = (v: string | null, fallback: boolean) =>
      v === null ? fallback : v === '1' || v.toLowerCase() === 'true';
    const allowEpub = parse(epubRaw, true);
    const allowPdf = parse(pdfRaw, false);

    const skipped: string[] = [];
    const kept = paths.filter((p) => {
      const lower = p.toLowerCase();
      if (!allowEpub && lower.endsWith('.epub')) {
        skipped.push(p);
        return false;
      }
      if (!allowPdf && lower.endsWith('.pdf')) {
        skipped.push(p);
        return false;
      }
      return true;
    });

    if (skipped.length > 0) {
      alert(
        `Skipped ${skipped.length} file${skipped.length === 1 ? '' : 's'} ` +
          `because their format is disabled in Settings → Behavior.`
      );
    }

    if (kept.length === 0) return;

    setSelectedFiles(kept);
    setFormValues(EMPTY_FORM_VALUES);
    setAddDialogOpen(false);
    setPipelineOpen(true);
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
      void loadFiles();
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
    for (const f of files) {
      if (fileIds.includes(f.id)) {
        fileNames.set(f.id, f.display_name);
      }
    }
    startUpload(fileIds, fileNames);
  }, [files]);

  const prevUploadingRef = useRef(false);
  useEffect(() => {
    if (prevUploadingRef.current && !uploadState.isUploading) {
      void loadFiles();
    }
    prevUploadingRef.current = uploadState.isUploading;
  }, [uploadState.isUploading, loadFiles]);

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
              files={files}
              total={total}
              loadingMore={loadingMore}
              onLoadMore={handleLoadMore}
              filterKey={`${selectedCategoryId ?? 'none'}::${debouncedQuery}`}
              onFileClick={handleFileClick}
              onFileEdit={handleFileEdit}
              onFileDelete={handleFileDeleteClick}
              onBulkUpload={handleBulkUpload}
              remoteEnabled={remoteEnabled}
            />
          )}
        </div>

        {uploadState.showPanel && (
          <RemoteUploadProgressPanel
            uploads={uploadState.uploads}
            onClose={dismissPanel}
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
        onOpenChange={setPipelineOpen}
        paths={selectedFiles}
        categories={categories}
        tags={tags}
        authors={authors}
        onCategoryCreated={handleCategoryCreated}
        onTagCreate={handleTagCreate}
        onAuthorCreate={handleAuthorCreate}
        onImportComplete={() => {
          setPipelineOpen(false);
          setSelectedFiles([]);
          void loadFiles();
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
              fields={schemaForPath(selectedFiles[0]).formFields}
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
                  fields={schemaForPath(selectedFiles[0]).formFields}
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
