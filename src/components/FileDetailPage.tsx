import { type ReactNode, useState, useEffect, useCallback } from 'react';
import { Link } from '@tanstack/react-router';
import { Button } from '@/components/ui/button';
import { FileList } from '@/components/FileList';
import { EditFileDialog } from '@/components/EditFileDialog';
import { ConfirmDeleteDialog } from '@/components/ConfirmDeleteDialog';
import { ArrowLeft } from 'lucide-react';
import { useFileActions } from '@/hooks/useFileActions';
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
  /** Optional identity of the underlying record (e.g. tag id). Used as the
   *  FileList filterKey so pagination resets if the consumer ever reuses
   *  the same mounted instance across different records. */
  filterKey?: string | number | null;
}

/**
 * Generic "list of files matching a criterion" page. Used by tag and author
 * detail routes — they pass a `fetcher` that calls the corresponding reverse-
 * index command and the page handles loading, editing, and deletion.
 *
 * Dialog state + handlers + supporting relation state come from the shared
 * `useFileActions` hook so this page stays in sync with Library.
 */
export function FileDetailPage({
  title,
  kind,
  icon,
  backTo,
  fetcher,
  filterKey,
}: FileDetailPageProps) {
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [loading, setLoading] = useState(true);

  const loadFiles = useCallback(async () => {
    setLoading(true);
    try {
      const result = await fetcher();
      setFiles(result);
    } catch (error) {
      console.error('Failed to fetch files:', error);
      setFiles([]);
    } finally {
      setLoading(false);
    }
  }, [fetcher]);

  useEffect(() => {
    void loadFiles();
  }, [loadFiles]);

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

  return (
    <div className="flex h-screen bg-background">
      <main className="flex-1 flex flex-col overflow-hidden">
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
                aria-label={`${files.length} files`}
              >
                — {files.length} {files.length === 1 ? 'volume' : 'volumes'}
              </span>
            </div>
          </div>
        </div>

        <div className="flex-1 flex flex-col overflow-hidden px-8 py-6">
          {loading ? (
            <div className="flex items-center justify-center h-32">
              <p className="text-sm text-muted-foreground font-serif-italic">Loading…</p>
            </div>
          ) : files.length === 0 ? (
            <div className="flex items-center justify-center h-32">
              <p className="text-sm text-muted-foreground font-serif-italic">
                No files under this {kind.toLowerCase()}.
              </p>
            </div>
          ) : (
            <FileList
              files={files}
              filterKey={filterKey ?? null}
              onFileEdit={handleFileEdit}
              onFileDelete={handleFileDeleteClick}
            />
          )}
        </div>
      </main>

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
    </div>
  );
}
