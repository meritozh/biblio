import { createFileRoute } from '@tanstack/react-router';
import { useEffect, useState, useCallback } from 'react';
import { FilePicker } from '@/components/FilePicker';
import { SearchBar } from '@/components/SearchBar';
import { FileList } from '@/components/FileList';
import { CategorySidebar } from '@/components/CategorySidebar';
import { ProcessingPipeline } from '@/components/ProcessingPipeline';
import { fetchFiles } from '@/stores';
import {
  fileCreate,
  coverSet,
  storageGetPath,
  storageCheckAccess,
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

  // Debounce the search input. `searchQuery` reflects every keystroke;
  // `debouncedQuery` is what actually drives fetches, so typing quickly
  // collapses to a single request.
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(searchQuery.trim()), 300);
    return () => clearTimeout(timer);
  }, [searchQuery]);

  const loadFiles = useCallback(async () => {
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
    setSelectedFiles(paths);
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

  return (
    <div className="flex h-screen bg-background">
      <CategorySidebar
        categories={categories}
        selectedCategoryId={selectedCategoryId}
        onCategorySelect={setSelectedCategoryId}
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <main className="flex-1 flex flex-col overflow-hidden">
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

          {loading ? (
            <div className="flex items-center justify-center h-32">
              <p className="text-sm text-muted-foreground">Loading...</p>
            </div>
          ) : (
            <FileList
              files={files}
              total={total}
              loadingMore={loadingMore}
              onLoadMore={handleLoadMore}
              filterKey={`${selectedCategoryId ?? 'all'}::${debouncedQuery}`}
              onFileClick={handleFileClick}
              onFileEdit={handleFileEdit}
              onFileDelete={handleFileDeleteClick}
            />
          )}
        </div>
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
