import { ConfirmDeleteDialog } from '@/components/ConfirmDeleteDialog';
import type { DynamicMetadataFormValues } from '@/components/DynamicMetadataForm';
import { EditFileDialog } from '@/components/EditFileDialog';
import { LuckyDialog } from '@/components/LuckyDialog';
import { ProcessingPipeline } from '@/components/ProcessingPipeline';
import { RemoteDeleteProgressPanel } from '@/components/RemoteDeleteProgress';
import { RemoteDownloadProgressPanel } from '@/components/RemoteDownloadProgress';
import { RemoteUploadProgressPanel } from '@/components/RemoteUploadProgress';
import {
  clearCompleted,
  dismissPanel,
  expandPanel,
  minimizePanel,
  type useRemoteUploadStore,
} from '@/stores/remoteUploadStore';
import {
  clearCompletedDeletes,
  dismissDeletePanel,
  expandDeletePanel,
  minimizeDeletePanel,
  type useRemoteDeleteStore,
} from '@/stores/remoteDeleteStore';
import {
  clearCompletedDownloads,
  dismissDownloadPanel,
  expandDownloadPanel,
  minimizeDownloadPanel,
  type useRemoteDownloadStore,
} from '@/stores/remoteDownloadStore';
import type { Author, Category, FileEntry, Tag } from '@/types';

interface HomeOverlaysProps {
  uploadState: ReturnType<typeof useRemoteUploadStore>;
  downloadState: ReturnType<typeof useRemoteDownloadStore>;
  deleteState: ReturnType<typeof useRemoteDeleteStore>;
  editDialogOpen: boolean;
  setEditDialogOpen: (open: boolean) => void;
  editingFile: FileEntry | null;
  deleteDialogOpen: boolean;
  setDeleteDialogOpen: (open: boolean) => void;
  deletingFile: FileEntry | null;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  handleTagCreate: (name: string) => Promise<Tag>;
  handleAuthorCreate: (name: string) => Promise<Author>;
  handleFileSave: (fileId: number, values: DynamicMetadataFormValues) => Promise<void>;
  handleFileDeleteConfirm: () => Promise<void>;
  luckyOpen: boolean;
  setLuckyOpen: (open: boolean) => void;
  luckyFiles: FileEntry[];
  luckyLoading: boolean;
  luckyRefreshing: boolean;
  luckyError: string | null;
  canLuckyShuffle: boolean;
  handleLucky: () => void | Promise<void>;
  handleFileClick: (file: FileEntry) => void;
  handleFileEdit: (file: FileEntry) => void;
  handleFileDeleteClick: (file: FileEntry) => void;
  remoteEnabled: boolean;
  pipelineOpen: boolean;
  onPipelineOpenChange: (open: boolean) => void;
  pipelineMinimized: boolean;
  onPipelineMinimize: () => void;
  onPipelineExpand: () => void;
  selectedFiles: string[];
  selectedPathFolderRoots: Record<string, string>;
  selectedCategoryId: number | null;
  onImportComplete: () => void;
}

export function HomeOverlays({
  uploadState,
  downloadState,
  deleteState,
  editDialogOpen,
  setEditDialogOpen,
  editingFile,
  deleteDialogOpen,
  setDeleteDialogOpen,
  deletingFile,
  categories,
  tags,
  authors,
  handleTagCreate,
  handleAuthorCreate,
  handleFileSave,
  handleFileDeleteConfirm,
  luckyOpen,
  setLuckyOpen,
  luckyFiles,
  luckyLoading,
  luckyRefreshing,
  luckyError,
  canLuckyShuffle,
  handleLucky,
  handleFileClick,
  handleFileEdit,
  handleFileDeleteClick,
  remoteEnabled,
  pipelineOpen,
  onPipelineOpenChange,
  pipelineMinimized,
  onPipelineMinimize,
  onPipelineExpand,
  selectedFiles,
  selectedPathFolderRoots,
  selectedCategoryId,
  onImportComplete,
}: HomeOverlaysProps) {
  return (
    <>
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

      <LuckyDialog
        open={luckyOpen}
        onOpenChange={setLuckyOpen}
        files={luckyFiles}
        loading={luckyLoading}
        refreshing={luckyRefreshing}
        error={luckyError}
        canShuffle={canLuckyShuffle}
        onShuffle={handleLucky}
        onFileClick={handleFileClick}
        onFileEdit={handleFileEdit}
        onFileDelete={handleFileDeleteClick}
        remoteEnabled={remoteEnabled}
      />

      <ProcessingPipeline
        open={pipelineOpen}
        onOpenChange={onPipelineOpenChange}
        minimized={pipelineMinimized}
        onMinimize={onPipelineMinimize}
        onExpand={onPipelineExpand}
        paths={selectedFiles}
        pathFolderRoots={selectedPathFolderRoots}
        targetCategoryId={selectedCategoryId}
        categories={categories}
        tags={tags}
        authors={authors}
        onTagCreate={handleTagCreate}
        onAuthorCreate={handleAuthorCreate}
        onImportComplete={onImportComplete}
      />
    </>
  );
}
