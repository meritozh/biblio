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
import { Plus, Pencil, Trash2, User as UserIcon } from 'lucide-react';
import {
  authorList,
  authorCount,
  authorCreate,
  authorUpdate,
  authorDelete,
} from '@/lib/tauri';
import { FilteredFileList } from '@/components/FilteredFileList';
import { makeId as makeConditionId, type Condition } from '@/lib/filters';
import type { AuthorWithUsage } from '@/types';

/** `/authors` mirrors `/tags`: management table when no params, filter
 *  mode when `?author=N`. Filter mode renders the same FileList scoped to
 *  `authors includes N`. */
interface AuthorsSearch {
  author?: number;
}

export const Route = createFileRoute('/authors')({
  validateSearch: (search: Record<string, unknown>): AuthorsSearch => {
    const out: AuthorsSearch = {};
    if (typeof search.author === 'number' && Number.isInteger(search.author)) {
      out.author = search.author;
    }
    return out;
  },
  component: AuthorsPage,
});

const PAGE_SIZE = 200;
const ROW_HEIGHT = 44;
const LOAD_MORE_THRESHOLD = 12;

function AuthorsPage() {
  const search = Route.useSearch();
  const filterConditions = useMemo<Condition[]>(() => {
    if (search.author == null) return [];
    return [
      {
        id: makeConditionId(),
        field: 'authors',
        op: 'includes',
        authorId: search.author,
      },
    ];
  }, [search.author]);

  if (filterConditions.length > 0) {
    return (
      <FilteredFileList
        backHref="/authors"
        backLabel="Authors"
        seededConditions={filterConditions}
        viewKey={`authors-filter::${search.author}`}
      />
    );
  }

  return <AuthorsManagementPage />;
}

function AuthorsManagementPage() {
  const navigate = useNavigate();
  const [authors, setAuthors] = useState<AuthorWithUsage[]>([]);
  const [total, setTotal] = useState<number>(0);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [newAuthorName, setNewAuthorName] = useState('');
  const [saving, setSaving] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editName, setEditName] = useState('');
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletingAuthor, setDeletingAuthor] = useState<AuthorWithUsage | null>(null);
  const [deleting, setDeleting] = useState(false);

  // Initial-mount fetch only. Mutations (create / edit / delete) apply a
  // local diff instead of calling reload — that keeps the virtualizer's
  // count and scrollTop stable, and avoids a redundant IPC round-trip
  // since the mutation command already returned success. See the
  // handler bodies below for the diff shape per action.
  const reload = useCallback(async () => {
    setLoading(true);
    const [page, count] = await Promise.all([
      authorList({ includeUsage: true, limit: PAGE_SIZE, offset: 0 }),
      authorCount(),
    ]);
    setAuthors(page.authors);
    setTotal(count);
    setLoading(false);
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const handleLoadMore = useCallback(async () => {
    if (loadingMore || authors.length >= total) return;
    setLoadingMore(true);
    try {
      const page = await authorList({
        includeUsage: true,
        limit: PAGE_SIZE,
        offset: authors.length,
      });
      // A local `handleCreate` inserts a row and bumps `total`, so
      // `authors.length` no longer equals the number of *server* rows
      // already fetched — using it as the offset can re-request a row we
      // already hold (or skip one). Dedup by id when appending so an
      // overlapping page never duplicates a row in the list.
      setAuthors((prev) => {
        const seen = new Set(prev.map((a) => a.id));
        const additions = page.authors.filter((a) => !seen.has(a.id));
        return additions.length === 0 ? prev : [...prev, ...additions];
      });
    } finally {
      setLoadingMore(false);
    }
  }, [loadingMore, authors.length, total]);

  const handleCreate = async () => {
    const trimmed = newAuthorName.trim();
    if (!trimmed) return;
    setSaving(true);
    try {
      const { id } = await authorCreate(trimmed);
      setCreateDialogOpen(false);
      setNewAuthorName('');
      // Local diff: insert at the alphabetical position so the new row
      // appears where the user expects without refetching. The server
      // sorts by raw `name` (binary compare); JS `localeCompare` is a
      // close-enough approximation for visual placement — order
      // resolves exactly on next mount.
      const created: AuthorWithUsage = {
        id,
        name: trimmed,
        created_at: new Date().toISOString(),
        usageCount: 0,
      };
      setAuthors((prev) => {
        const insertAt = prev.findIndex(
          (a) => a.name.localeCompare(trimmed) > 0
        );
        if (insertAt === -1) return [...prev, created];
        return [...prev.slice(0, insertAt), created, ...prev.slice(insertAt)];
      });
      setTotal((t) => t + 1);
    } catch (error) {
      console.error('Failed to create author:', error);
      alert(`Failed to create author: ${error}`);
    }
    setSaving(false);
  };

  const handleStartEdit = (author: AuthorWithUsage) => {
    setEditingId(author.id);
    setEditName(author.name);
  };

  const handleSaveEdit = async () => {
    const trimmed = editName.trim();
    if (!editingId || !trimmed) return;
    const targetId = editingId;
    try {
      await authorUpdate(targetId, trimmed);
      setEditingId(null);
      setEditName('');
      // Local diff: patch the row's name in place. The list stays at
      // the row's original position even if the rename moved it
      // alphabetically — re-sorting client-side would jump the row
      // away from the user, which is worse UX than a slight ordering
      // anomaly until next mount.
      setAuthors((prev) =>
        prev.map((a) => (a.id === targetId ? { ...a, name: trimmed } : a))
      );
    } catch (error) {
      console.error('Failed to update author:', error);
      alert(`Failed to update author: ${error}`);
    }
  };

  const handleCancelEdit = () => {
    setEditingId(null);
    setEditName('');
  };

  const handleDeleteClick = (author: AuthorWithUsage) => {
    setDeletingAuthor(author);
    setDeleteDialogOpen(true);
  };

  const handleConfirmDelete = async () => {
    if (!deletingAuthor) return;
    setDeleting(true);
    const deletedId = deletingAuthor.id;
    try {
      await authorDelete(deletedId);
      setDeleteDialogOpen(false);
      setDeletingAuthor(null);
      // Local diff: drop the row, decrement the header count. Surrounding
      // rows stay in place, so the virtualizer's scrollTop is preserved.
      setAuthors((prev) => prev.filter((a) => a.id !== deletedId));
      setTotal((t) => Math.max(0, t - 1));
    } catch (error) {
      console.error('Failed to delete author:', error);
      alert(`Failed to delete author: ${error}`);
    }
    setDeleting(false);
  };

  // Single click stays on /authors with `?author=N`. The page re-renders
  // into filter mode and FilteredFileList takes over.
  const handleRowClick = (author: AuthorWithUsage) => {
    void navigate({ to: '/authors', search: { author: author.id } });
  };

  const tally = useMemo(() => {
    if (loading) return '…';
    if (authors.length < total) return `${authors.length} of ${total}`;
    return String(total);
  }, [authors.length, total, loading]);

  return (
    <>
      <div
        className="flex items-end justify-between px-8 pt-14 pb-5 border-b border-border"
        data-tauri-drag-region
      >
        <div className="flex items-baseline gap-3">
          <h1 className="text-3xl text-foreground flex items-center gap-3">
            <UserIcon className="h-6 w-6 text-muted-foreground" aria-hidden="true" />
            Authors
          </h1>
          <span className="font-serif-italic text-sm text-muted-foreground">
            — {tally} {total === 1 ? 'author' : 'authors'}
          </span>
        </div>
        <Button onClick={() => setCreateDialogOpen(true)}>
          <Plus className="h-4 w-4 mr-1" />
          Add Author
        </Button>
      </div>

      <div className="flex-1 overflow-hidden px-8 py-6">
        {loading ? (
          <div className="flex items-center justify-center h-32">
            <p className="text-sm text-muted-foreground">Loading...</p>
          </div>
        ) : authors.length === 0 ? (
          <div className="flex items-center justify-center h-32">
            <p className="text-sm text-muted-foreground">
              No authors yet. Click "Add Author" to create one.
            </p>
          </div>
        ) : (
          <div className="rounded-md border h-full flex flex-col overflow-hidden">
            <div className="grid grid-cols-[1fr_120px] items-center gap-3 px-4 py-2 border-b border-border bg-muted/30 text-xs font-medium text-muted-foreground uppercase tracking-wide">
              <span>Name</span>
              <span>Actions</span>
            </div>
            <VirtualList<AuthorWithUsage>
              items={authors}
              getKey={(a) => a.id}
              estimateSize={ROW_HEIGHT}
              overscan={8}
              onLoadMore={handleLoadMore}
              hasMore={authors.length < total}
              loadMoreThreshold={LOAD_MORE_THRESHOLD}
              className="flex-1 overflow-auto"
              renderItem={(author) => (
                <div
                  role="row"
                  className="grid grid-cols-[1fr_120px] items-center gap-3 px-4 border-b border-border hover:bg-muted/30"
                  style={{ height: ROW_HEIGHT }}
                >
                  <div className="min-w-0">
                    {editingId === author.id ? (
                      <Input
                        value={editName}
                        onChange={(e) => setEditName(e.target.value)}
                        className="h-8 w-full"
                        autoFocus
                      />
                    ) : (
                      <button
                        type="button"
                        onClick={() => handleRowClick(author)}
                        className="text-foreground hover:text-primary hover:underline underline-offset-4 text-left focus:outline-none"
                        aria-label={`Filter Library by ${author.name}`}
                      >
                        {author.name}
                        {author.usageCount > 0 && (
                          <span className="ml-2 text-xs text-muted-foreground font-serif-italic">
                            — {author.usageCount}
                          </span>
                        )}
                      </button>
                    )}
                  </div>
                  <div className="flex gap-1">
                    {editingId === author.id ? (
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
                            handleStartEdit(author);
                          }}
                          aria-label={`Edit ${author.name}`}
                        >
                          <Pencil className="h-4 w-4" />
                        </Button>
                        <Button
                          size="icon"
                          variant="ghost"
                          className="h-8 w-8 text-destructive"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleDeleteClick(author);
                          }}
                          aria-label={`Delete ${author.name}`}
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </>
                    )}
                  </div>
                </div>
              )}
              loadingMoreSlot={
                loadingMore ? (
                  <div className="flex items-center justify-center py-3 text-xs text-muted-foreground font-serif-italic">
                    Loading more…
                  </div>
                ) : null
              }
            />
          </div>
        )}
      </div>

      <Dialog open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create Author</DialogTitle>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div>
              <label className="text-sm font-medium mb-2 block">Name</label>
              <Input
                value={newAuthorName}
                onChange={(e) => setNewAuthorName(e.target.value)}
                placeholder="Author name"
                autoFocus
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && newAuthorName.trim()) {
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
            <Button onClick={handleCreate} disabled={saving || !newAuthorName.trim()}>
              {saving ? 'Creating...' : 'Create'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <AlertDialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete Author</AlertDialogTitle>
            <AlertDialogDescription>
              Are you sure you want to delete "{deletingAuthor?.name}"? They will
              be removed from any files currently crediting them.
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
