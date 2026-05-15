import { type ReactNode, useCallback, useMemo } from 'react';
import { Link } from '@tanstack/react-router';
import { Button } from '@/components/ui/button';
import { FileList } from '@/components/FileList';
import { EditFileDialog } from '@/components/EditFileDialog';
import { ConfirmDeleteDialog } from '@/components/ConfirmDeleteDialog';
import { ArrowLeft } from 'lucide-react';
import { useFileActions } from '@/hooks/useFileActions';
import { useView } from '@/hooks/useView';
import { fileStore } from '@/stores/fileStore';
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
import { RemoteUploadProgressPanel } from '@/components/RemoteUploadProgress';
import { RemoteDownloadProgressPanel } from '@/components/RemoteDownloadProgress';
import { RemoteDeleteProgressPanel } from '@/components/RemoteDeleteProgress';
import type { FileEntry } from '@/types';

interface FileDetailPageProps {
  /** Display name — e.g. "sci-fi" for a tag, "刘慈欣" for an author. */
  title: string;
  /** Short type label shown before the title ("Tag" / "Author"). */
  kind: string;
  /** Small icon shown alongside the title. */
  icon: ReactNode;
  /** Where the Back button should go. */
  backTo: string;
  /** Async source of files for this page. Runs on mount + after mutations. */
  fetcher: () => Promise<FileEntry[]>;
  /** Identity of the underlying record (e.g. tag id). Drives the view key
   *  in the normalized store so different records keep separate slices. */
  viewKey: string;
  /** Optional FileList filterKey — defaults to viewKey. */
  filterKey?: string | number | null;
}

/**
 * Generic "list of files matching a criterion" page. Used by tag and author
 * detail routes — they pass a `fetcher` that calls the corresponding reverse-
 * index command and the page handles loading, editing, and deletion.
 *
 * Files are owned by the normalized `fileStore` via `useView`. Edit/delete
 * mutations patch single rows in place; tag/author rename events refresh
 * via the store's epoch counter.
 *
 * Bulk Upload / Download / Delete share the same producer-consumer queues
 * Library uses, so a workflow that crosses pages (e.g. select on a tag,
 * then continue selecting on Library) keeps a single ordered queue.
 */
export function FileDetailPage({
  title,
  kind,
  icon,
  backTo,
  fetcher,
  viewKey,
  filterKey,
}: FileDetailPageProps) {
  const wrappedFetcher = useCallback(async () => {
    const files = await fetcher();
    return { files, total: files.length };
  }, [fetcher]);

  const { ids, total, loading } = useView(viewKey, wrappedFetcher);

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

  const effectiveFilterKey = useMemo(
    () => filterKey ?? viewKey,
    [filterKey, viewKey]
  );

  const uploadState = useRemoteUploadStore();
  const downloadState = useRemoteDownloadStore();
  const deleteState = useRemoteDeleteStore();

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

  return (
    <>
      <div
        className="flex items-center justify-between px-8 pt-14 pb-5 border-b border-border"
        data-tauri-drag-region
      >
        <div className="flex items-center gap-4">
          <Link to={backTo}>
            <Button variant="ghost" size="sm">
              <ArrowLeft className="h-4 w-4 mr-1" />
              Back
            </Button>
          </Link>
          <div className="flex items-baseline gap-3">
            <span className="font-serif-italic text-sm text-muted-foreground inline-flex items-center gap-2">
              {icon}
              {kind}
            </span>
            <h1 className="text-3xl text-foreground">{title}</h1>
            <span
              className="font-serif-italic text-sm text-muted-foreground"
              aria-label={`${total} files`}
            >
              — {total} {total === 1 ? 'volume' : 'volumes'}
            </span>
          </div>
        </div>
      </div>

      <div className="flex-1 flex flex-col overflow-hidden px-8 py-6">
        {loading ? (
          <div className="flex items-center justify-center h-32">
            <p className="text-sm text-muted-foreground font-serif-italic">Loading…</p>
          </div>
        ) : ids.length === 0 ? (
          <div className="flex items-center justify-center h-32">
            <p className="text-sm text-muted-foreground font-serif-italic">
              No files under this {kind.toLowerCase()}.
            </p>
          </div>
        ) : (
          <FileList
            ids={ids}
            total={total}
            filterKey={effectiveFilterKey}
            onFileEdit={handleFileEdit}
            onFileDelete={handleFileDeleteClick}
            onBulkUpload={handleBulkUpload}
            onBulkDownload={handleBulkDownload}
            onBulkDelete={handleBulkDelete}
            availableTags={tags}
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
    </>
  );
}
