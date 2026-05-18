import { type ReactNode, useCallback, useMemo, useState } from 'react';
import type { Author, Tag } from '@/types';
import { Link } from '@tanstack/react-router';
import { ArrowLeft } from 'lucide-react';
import { FileList } from '@/components/FileList';
import { EditFileDialog } from '@/components/EditFileDialog';
import { ConfirmDeleteDialog } from '@/components/ConfirmDeleteDialog';
import { useFileActions } from '@/hooks/useFileActions';
import { useView, type ViewFetcherResult } from '@/hooks/useView';
import { useAppState } from '@/stores/appStore';
import {
  enqueueUpload,
} from '@/stores/remoteUploadStore';
import { enqueueDownload } from '@/stores/remoteDownloadStore';
import { enqueueDelete } from '@/stores/remoteDeleteStore';
import { fileStore } from '@/stores/fileStore';
import { fetchFiles, type SortKey } from '@/stores';
import { type Condition } from '@/lib/filters';
import type { FileEntry } from '@/types';

interface FilteredFileListProps {
  /** Override for the header title. When omitted, FilteredFileList
   *  resolves a sensible default from the `seededConditions` using the
   *  tags / authors maps that `useFileActions` already provides — so
   *  callers don't have to load the same data twice just to render a
   *  name in the header. */
  title?: ReactNode;
  /** Where the back chip routes to (e.g. "/tags" or "/authors"). */
  backHref: string;
  /** Short label inside the back chip — typically the singular of the
   *  parent route's title ("Tags", "Authors"). */
  backLabel: string;
  /** Conditions injected as the seed of the filter. The user can add more
   *  conditions on top via the FilterEditor — they AND-combine with these
   *  base conditions. */
  seededConditions: ReadonlyArray<Condition>;
  /** Stable cache key for `useView`. Different filter pages must use
   *  disjoint keys so their slices don't cross-contaminate. */
  viewKey: string;
}

const FILES_PAGE_SIZE = 200;

/** Build a header label like "Tag: sci-fi" or "Author: 刘慈欣" from a
 *  small set of seeded conditions + lookup maps. Returns `null` if the
 *  shape isn't one this helper knows how to label — caller can fall
 *  back to a generic string in that case. */
function deriveTitle(
  seeded: ReadonlyArray<Condition>,
  tagsById: Map<number, Tag>,
  authorsById: Map<number, Author>
): string | null {
  if (seeded.length !== 1) return null;
  const c = seeded[0]!;
  if (c.field === 'tags' && c.op === 'includes' && c.tagId !== undefined) {
    return `Tag: ${tagsById.get(c.tagId)?.name ?? `#${c.tagId}`}`;
  }
  if (c.field === 'tags' && c.op === 'includes_any' && c.tagIds.length > 0) {
    const names = c.tagIds.map((id) => tagsById.get(id)?.name ?? `#${id}`);
    if (names.length === 1) return `Tag: ${names[0]}`;
    if (names.length <= 3) return `Tags: ${names.join(', ')}`;
    return `Tags: ${names.slice(0, 2).join(', ')} +${names.length - 2} more`;
  }
  if (c.field === 'authors' && c.op === 'includes' && c.authorId !== undefined) {
    return `Author: ${authorsById.get(c.authorId)?.name ?? `#${c.authorId}`}`;
  }
  return null;
}

/**
 * Library-style file list scoped by a fixed set of seeded conditions.
 *
 * Used by `/tags?tag=N` and `/authors?author=N` to render a filtered
 * library inside the management route — no navigation back to `/`, no
 * cross-route state hand-off, no URL-seed race. The seed conditions are
 * pushed straight into the SQL query alongside the active category from
 * the sidebar; the chip editor that lives inside `FileList` can layer
 * more conditions on top.
 */
export function FilteredFileList({
  title,
  backHref,
  backLabel,
  seededConditions,
  viewKey,
}: FilteredFileListProps) {
  const selectedCategoryId = useAppState((s) => s.selectedCategoryId);

  // Local sort + extra filter conditions, owned by this page. Mirrors
  // HomePage's shape but lives entirely inside the filtered view —
  // toggling sidebar category re-runs the fetch via the viewKey change.
  const [sortBy, setSortBy] = useState<SortKey>('name');
  const [sortDesc, setSortDesc] = useState(false);
  const handleSortChange = useCallback((next: SortKey, desc: boolean) => {
    setSortBy(next);
    setSortDesc(desc);
  }, []);
  const [extraConditions, setExtraConditions] = useState<Condition[]>([]);
  const [loadingMore, setLoadingMore] = useState(false);

  // Effective conditions = seed (from URL) + user-added (from chip
  // editor). Memoized so identity stays stable across renders.
  const effectiveConditions = useMemo(
    () => [...seededConditions, ...extraConditions],
    [seededConditions, extraConditions]
  );
  const conditionsKey = useMemo(
    () => JSON.stringify(effectiveConditions),
    [effectiveConditions]
  );

  const fullViewKey = useMemo(
    () =>
      `${viewKey}::category=${selectedCategoryId ?? 'none'}::sort=${sortBy}:${sortDesc ? 'desc' : 'asc'}::filters=${conditionsKey}`,
    [viewKey, selectedCategoryId, sortBy, sortDesc, conditionsKey]
  );

  const fetchView = useCallback(async (): Promise<ViewFetcherResult> => {
    if (selectedCategoryId === null) return { files: [], total: 0 };
    return await fetchFiles({
      category_id: selectedCategoryId,
      sort_by: sortBy,
      sort_desc: sortDesc,
      conditions: effectiveConditions,
      limit: FILES_PAGE_SIZE,
      offset: 0,
    });
  }, [selectedCategoryId, sortBy, sortDesc, effectiveConditions]);

  const { ids, total, loading, appendMore } = useView(fullViewKey, fetchView);

  const handleLoadMore = useCallback(async () => {
    if (loadingMore || selectedCategoryId === null) return;
    setLoadingMore(true);
    try {
      const result = await fetchFiles({
        category_id: selectedCategoryId,
        sort_by: sortBy,
        sort_desc: sortDesc,
        conditions: effectiveConditions,
        limit: FILES_PAGE_SIZE,
        offset: ids.length,
      });
      appendMore(result);
    } finally {
      setLoadingMore(false);
    }
  }, [
    loadingMore,
    selectedCategoryId,
    sortBy,
    sortDesc,
    effectiveConditions,
    ids.length,
    appendMore,
  ]);

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

  const handleFileClick = useCallback((file: FileEntry) => {
    // Library uses console.log here too; the placeholder kept for now —
    // tap-to-open isn't wired to anything in this UI either.
    console.log('File clicked:', file);
  }, []);

  // Name lookup for bulk-action progress panels. The fileStore has the
  // hydrated rows because `useView` populated `byId`.
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

  // Name resolution maps for the header title. `useFileActions` already
  // fetched these — no second IPC call needed.
  const tagsById = useMemo(() => {
    const m = new Map<number, Tag>();
    for (const t of tags) m.set(t.id, t);
    return m;
  }, [tags]);
  const authorsById = useMemo(() => {
    const m = new Map<number, Author>();
    for (const a of authors) m.set(a.id, a);
    return m;
  }, [authors]);
  const derivedTitle = useMemo(
    () => deriveTitle(seededConditions, tagsById, authorsById),
    [seededConditions, tagsById, authorsById]
  );
  const headerTitle = title ?? derivedTitle ?? 'Filtered files';

  return (
    <>
      <div
        className="flex items-end justify-between px-8 pt-14 pb-5 border-b border-border"
        data-tauri-drag-region
      >
        <div className="flex items-baseline gap-3 min-w-0">
          <Link
            to={backHref}
            className="inline-flex items-center gap-1.5 rounded-full border bg-secondary/40 hover:bg-secondary transition-colors h-8 px-3 text-xs shrink-0"
            aria-label={`Back to ${backLabel}`}
          >
            <ArrowLeft className="h-3 w-3" aria-hidden="true" />
            <span className="text-muted-foreground">{backLabel}</span>
          </Link>
          <div className="text-3xl text-foreground truncate min-w-0">{headerTitle}</div>
          <span
            className="font-serif-italic text-sm text-muted-foreground shrink-0"
            aria-label={`${total} files`}
          >
            — {total} {total === 1 ? 'volume' : 'volumes'}
          </span>
        </div>
      </div>

      <div className="flex-1 flex flex-col overflow-hidden px-8 py-6">
        {loading ? (
          <div className="flex items-center justify-center h-32">
            <p className="text-sm text-muted-foreground">Loading...</p>
          </div>
        ) : (
          <FileList
            ids={ids}
            total={total}
            loadingMore={loadingMore}
            onLoadMore={handleLoadMore}
            filterKey={fullViewKey}
            onFileClick={handleFileClick}
            onFileEdit={handleFileEdit}
            onFileDelete={handleFileDeleteClick}
            onBulkUpload={handleBulkUpload}
            onBulkDownload={handleBulkDownload}
            onBulkDelete={handleBulkDelete}
            availableTags={tags}
            availableAuthors={authors}
            sortBy={sortBy}
            sortDesc={sortDesc}
            onSortChange={handleSortChange}
            applySort={false}
            conditions={extraConditions}
            onConditionsChange={setExtraConditions}
            applyConditionsClientSide={false}
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
    </>
  );
}
