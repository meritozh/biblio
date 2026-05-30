import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { useState, useEffect, useCallback, useMemo } from 'react';
import { VirtualList } from '@/components/VirtualList';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Plus, Pencil, Trash2, Tag as TagIcon, Filter as FilterIcon } from 'lucide-react';
import { tagList, tagCount, tagCreate, tagUpdate, tagDelete } from '@/lib/tauri';
import { FilteredFileList } from '@/components/FilteredFileList';
import { makeId as makeConditionId, type Condition } from '@/lib/filters';
import type { TagWithUsage } from '@/types';

/** `/tags` has two modes:
 *  - management (no search params): the virtualized tag table
 *  - filter (`?tag=N` or `?tags=5,10,15`): renders FileList scoped by
 *    those tags. Lives on this route — clicking a tag here navigates
 *    in-place rather than punting to `/`, so there's no cross-route
 *    state hand-off race. */
interface TagsSearch {
  tag?: number;
  tags?: number[];
}

export const Route = createFileRoute('/tags')({
  validateSearch: (search: Record<string, unknown>): TagsSearch => {
    const out: TagsSearch = {};
    if (typeof search.tag === 'number' && Number.isInteger(search.tag)) {
      out.tag = search.tag;
    }
    if (Array.isArray(search.tags)) {
      const ids = search.tags.filter(
        (v): v is number => typeof v === 'number' && Number.isInteger(v)
      );
      if (ids.length > 0) out.tags = ids;
    }
    return out;
  },
  component: TagsPage,
});

const PAGE_SIZE = 200;
const ROW_HEIGHT = 44;
const LOAD_MORE_THRESHOLD = 12;

function TagsPage() {
  const search = Route.useSearch();
  // Filter mode: a tag id is in the URL → render the filtered FileList
  // in place of the management table. Pure URL-driven, no race with any
  // other effect. Build the seeded conditions on every render — cheap,
  // and conditions identity is memoized inside FilteredFileList.
  const filterConditions = useMemo<Condition[]>(() => {
    const out: Condition[] = [];
    if (search.tag != null) {
      out.push({
        id: makeConditionId(),
        field: 'tags',
        op: 'includes',
        tagId: search.tag,
      });
    }
    if (search.tags != null && search.tags.length > 0) {
      out.push({
        id: makeConditionId(),
        field: 'tags',
        op: 'includes_any',
        tagIds: [...search.tags],
      });
    }
    return out;
  }, [search.tag, search.tags]);
  const inFilterMode = filterConditions.length > 0;

  if (inFilterMode) {
    return (
      <FilteredFileList
        backHref="/tags"
        backLabel="Tags"
        seededConditions={filterConditions}
        viewKey={`tags-filter::${search.tag ?? ''}::${(search.tags ?? []).join(',')}`}
      />
    );
  }

  return <TagsManagementPage />;
}

function TagsManagementPage() {
  const navigate = useNavigate();
  const [tags, setTags] = useState<TagWithUsage[]>([]);
  const [total, setTotal] = useState<number>(0);
  const [fetchedOffset, setFetchedOffset] = useState<number>(0);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [newTagName, setNewTagName] = useState('');
  const [saving, setSaving] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editName, setEditName] = useState('');
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletingTag, setDeletingTag] = useState<TagWithUsage | null>(null);
  const [deleting, setDeleting] = useState(false);

  // Multi-select drives the "Apply" → navigate-with-filter flow.
  // `selectionMode` flips on the checkbox column; while in selection
  // mode, row clicks toggle membership instead of triggering the
  // single-tag navigation.
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());

  // Initial-mount fetch only. Mutations (create / edit / delete) apply a
  // local diff instead of calling reload — preserves scrollTop, no
  // redundant IPC. See the handler bodies below for the diff shape.
  const reload = useCallback(async () => {
    setLoading(true);
    const [page, count] = await Promise.all([
      tagList({ includeUsage: true, limit: PAGE_SIZE, offset: 0 }),
      tagCount(),
    ]);
    setTags(page.tags);
    setTotal(count);
    setFetchedOffset(page.tags.length);
    setLoading(false);
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const handleLoadMore = useCallback(async () => {
    if (loadingMore || fetchedOffset >= total) return;
    setLoadingMore(true);
    try {
      const page = await tagList({
        includeUsage: true,
        limit: PAGE_SIZE,
        offset: fetchedOffset,
      });
      // Dedup by id when appending so an overlapping page never duplicates
      // a row in the list.
      setTags((prev) => {
        const seen = new Set(prev.map((t) => t.id));
        const additions = page.tags.filter((t) => !seen.has(t.id));
        return additions.length === 0 ? prev : [...prev, ...additions];
      });
      setFetchedOffset((prev) => prev + page.tags.length);
    } finally {
      setLoadingMore(false);
    }
  }, [loadingMore, fetchedOffset, total]);

  const handleCreate = async () => {
    const trimmed = newTagName.trim();
    if (!trimmed) return;
    setSaving(true);
    try {
      const { id } = await tagCreate(trimmed, undefined);
      setCreateDialogOpen(false);
      setNewTagName('');
      // Local diff: insert at the alphabetical position so the new row
      // appears where the user expects without refetching. localeCompare
      // is a close-enough approximation of the server's `ORDER BY name`
      // for visual placement; order resolves exactly on next mount.
      const created: TagWithUsage = {
        id,
        name: trimmed,
        color: null,
        created_at: new Date().toISOString(),
        usageCount: 0,
      };
      setTags((prev) => {
        const insertAt = prev.findIndex(
          (t) => t.name.localeCompare(trimmed) > 0
        );
        if (insertAt === -1) return [...prev, created];
        return [...prev.slice(0, insertAt), created, ...prev.slice(insertAt)];
      });
      setTotal((t) => t + 1);
    } catch (error) {
      console.error('Failed to create tag:', error);
      alert(`Failed to create tag: ${error}`);
    }
    setSaving(false);
  };

  const handleStartEdit = (tag: TagWithUsage) => {
    setEditingId(tag.id);
    setEditName(tag.name);
  };

  const handleSaveEdit = async () => {
    const trimmed = editName.trim();
    if (!editingId || !trimmed) return;
    const targetId = editingId;
    try {
      await tagUpdate(targetId, trimmed, undefined);
      setEditingId(null);
      setEditName('');
      // Local diff: patch the row's name in place. Don't re-sort — the
      // row stays under the user's pointer even if the rename moved it
      // alphabetically. Order resolves on next mount.
      setTags((prev) =>
        prev.map((t) => (t.id === targetId ? { ...t, name: trimmed } : t))
      );
    } catch (error) {
      console.error('Failed to update tag:', error);
      alert(`Failed to update tag: ${error}`);
    }
  };

  const handleCancelEdit = () => {
    setEditingId(null);
    setEditName('');
  };

  const handleDeleteClick = (tag: TagWithUsage) => {
    setDeletingTag(tag);
    setDeleteDialogOpen(true);
  };

  const handleConfirmDelete = async () => {
    if (!deletingTag) return;
    setDeleting(true);
    const deletedId = deletingTag.id;
    try {
      await tagDelete(deletedId);
      setDeleteDialogOpen(false);
      setDeletingTag(null);
      // Drop from selection too — gone is gone.
      setSelectedIds((prev) => {
        const next = new Set(prev);
        next.delete(deletedId);
        return next;
      });
      // Local diff: drop the row, decrement the header tally. Surrounding
      // rows stay in place, so scrollTop is preserved.
      setTags((prev) => prev.filter((t) => t.id !== deletedId));
      setTotal((t) => Math.max(0, t - 1));
    } catch (error) {
      console.error('Failed to delete tag:', error);
      alert(`Failed to delete tag: ${error}`);
    }
    setDeleting(false);
  };

  // Single-click navigation: lands in Library with `?tag=N` which the
  // route's URL-seed effect picks up as a single-tag includes condition.
  const handleRowClick = (tag: TagWithUsage) => {
    if (selectionMode) {
      setSelectedIds((prev) => {
        const next = new Set(prev);
        if (next.has(tag.id)) next.delete(tag.id);
        else next.add(tag.id);
        return next;
      });
      return;
    }
    void navigate({ to: '/tags', search: { tag: tag.id } });
  };

  // Multi-select Apply: stay on /tags but with `?tags=[ids]`. The page
  // re-renders into filter mode and FilteredFileList takes over.
  const handleApplySelection = () => {
    const ids = [...selectedIds];
    if (ids.length === 0) return;
    void navigate({ to: '/tags', search: { tags: ids } });
  };

  const toggleSelectionMode = () => {
    setSelectionMode((prev) => {
      if (prev) setSelectedIds(new Set());
      return !prev;
    });
  };

  const tally = useMemo(() => {
    if (loading) return '…';
    if (tags.length < total) return `${tags.length} of ${total}`;
    return String(total);
  }, [tags.length, total, loading]);

  return (
    <>
      <div
        className="flex items-end justify-between px-8 pt-14 pb-5 border-b border-border"
        data-tauri-drag-region
      >
        <div className="flex items-baseline gap-3">
          <h1 className="text-3xl text-foreground flex items-center gap-3">
            <TagIcon className="h-6 w-6 text-muted-foreground" aria-hidden="true" />
            Tags
          </h1>
          <span className="font-serif-italic text-sm text-muted-foreground">
            — {tally} {total === 1 ? 'tag' : 'tags'}
          </span>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant={selectionMode ? 'default' : 'outline'}
            size="sm"
            onClick={toggleSelectionMode}
          >
            {selectionMode ? 'Done' : 'Select'}
          </Button>
          {selectionMode && (
            <Button
              size="sm"
              onClick={handleApplySelection}
              disabled={selectedIds.size === 0}
            >
              <FilterIcon className="h-3.5 w-3.5 mr-1" />
              Filter Library ({selectedIds.size})
            </Button>
          )}
          <Button onClick={() => setCreateDialogOpen(true)}>
            <Plus className="h-4 w-4 mr-1" />
            Add Tag
          </Button>
        </div>
      </div>

      <div className="flex-1 overflow-hidden px-8 py-6">
        {loading ? (
          <div className="flex items-center justify-center h-32">
            <p className="text-sm text-muted-foreground">Loading...</p>
          </div>
        ) : tags.length === 0 ? (
          <div className="flex items-center justify-center h-32">
            <p className="text-sm text-muted-foreground">
              No tags yet. Click "Add Tag" to create one.
            </p>
          </div>
        ) : (
          <div className="rounded-md border h-full flex flex-col overflow-hidden">
            {/* Header row */}
            <div className="grid grid-cols-[auto_1fr_120px] items-center gap-3 px-4 py-2 border-b border-border bg-muted/30 text-xs font-medium text-muted-foreground uppercase tracking-wide">
              {selectionMode ? <span className="w-4" aria-hidden="true" /> : null}
              <span>Name</span>
              <span>Actions</span>
            </div>
            {/* Virtual scroller */}
            <VirtualList<TagWithUsage>
              items={tags}
              getKey={(t) => t.id}
              estimateSize={ROW_HEIGHT}
              overscan={8}
              onLoadMore={handleLoadMore}
              hasMore={tags.length < total}
              loadMoreThreshold={LOAD_MORE_THRESHOLD}
              className="flex-1 overflow-auto"
              loadingMoreSlot={
                loadingMore ? (
                  <div className="flex items-center justify-center py-3 text-xs text-muted-foreground font-serif-italic">
                    Loading more…
                  </div>
                ) : null
              }
              renderItem={(tag) => {
                const checked = selectedIds.has(tag.id);
                return (
                  <div
                    role="row"
                    className={`grid items-center gap-3 px-4 border-b border-border hover:bg-muted/30 ${
                      selectionMode
                        ? 'grid-cols-[auto_1fr_120px]'
                        : 'grid-cols-[1fr_120px]'
                    } ${checked ? 'bg-primary/5' : ''}`}
                    style={{ height: ROW_HEIGHT }}
                  >
                    {selectionMode && (
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={() => handleRowClick(tag)}
                        aria-label={`Select ${tag.name}`}
                        className="h-4 w-4 accent-primary cursor-pointer"
                        onClick={(e) => e.stopPropagation()}
                      />
                    )}
                    <div className="min-w-0">
                      {editingId === tag.id ? (
                        <Input
                          value={editName}
                          onChange={(e) => setEditName(e.target.value)}
                          className="h-8 w-full"
                          autoFocus
                        />
                      ) : (
                        <button
                          type="button"
                          onClick={() => handleRowClick(tag)}
                          className="text-foreground hover:text-primary hover:underline underline-offset-4 text-left focus:outline-none"
                          aria-label={`Filter Library by ${tag.name}`}
                        >
                          {tag.name}
                          {tag.usageCount > 0 && (
                            <span className="ml-2 text-xs text-muted-foreground font-serif-italic">
                              — {tag.usageCount}
                            </span>
                          )}
                        </button>
                      )}
                    </div>
                    <div className="flex gap-1">
                      {editingId === tag.id ? (
                        <>
                          <Button size="sm" onClick={handleSaveEdit}>
                            Save
                          </Button>
                          <Button size="sm" variant="ghost" onClick={handleCancelEdit}>
                            Cancel
                          </Button>
                        </>
                      ) : (
                        <>
                          <Button
                            size="icon"
                            variant="ghost"
                            className="h-8 w-8"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleStartEdit(tag);
                            }}
                            aria-label={`Edit ${tag.name}`}
                          >
                            <Pencil className="h-4 w-4" />
                          </Button>
                          <Button
                            size="icon"
                            variant="ghost"
                            className="h-8 w-8 text-destructive"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleDeleteClick(tag);
                            }}
                            aria-label={`Delete ${tag.name}`}
                          >
                            <Trash2 className="h-4 w-4" />
                          </Button>
                        </>
                      )}
                    </div>
                  </div>
                );
              }}
            />
          </div>
        )}
      </div>

      <Dialog open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create Tag</DialogTitle>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div>
              <label className="text-sm font-medium mb-2 block">Name</label>
              <Input
                value={newTagName}
                onChange={(e) => setNewTagName(e.target.value)}
                placeholder="Tag name"
                autoFocus
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && newTagName.trim()) {
                    void handleCreate();
                  }
                }}
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setCreateDialogOpen(false)}>
              Cancel
            </Button>
            <Button onClick={handleCreate} disabled={saving || !newTagName.trim()}>
              {saving ? 'Creating...' : 'Create'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <AlertDialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete Tag</AlertDialogTitle>
            <AlertDialogDescription>
              Are you sure you want to delete "{deletingTag?.name}"? It will be
              removed from any files currently using it.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => {
                e.preventDefault();
                void handleConfirmDelete();
              }}
              disabled={deleting}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {deleting ? 'Deleting...' : 'Delete'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
