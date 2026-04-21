import { createFileRoute } from '@tanstack/react-router';
import { useState, useEffect, useCallback } from 'react';
import { Link } from '@tanstack/react-router';
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { Plus, Pencil, Trash2, ArrowLeft, User } from 'lucide-react';
import { authorList, authorCreate, authorUpdate, authorDelete } from '@/lib/tauri';
import type { AuthorWithUsage } from '@/types';

export const Route = createFileRoute('/authors')({
  component: AuthorsPage,
});

function AuthorsPage() {
  const [authors, setAuthors] = useState<AuthorWithUsage[]>([]);
  const [loading, setLoading] = useState(true);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [newAuthorName, setNewAuthorName] = useState('');
  const [saving, setSaving] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editName, setEditName] = useState('');
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletingAuthor, setDeletingAuthor] = useState<AuthorWithUsage | null>(null);
  const [deleting, setDeleting] = useState(false);

  const loadAuthors = useCallback(async () => {
    setLoading(true);
    const result = await authorList(true);
    setAuthors(result.authors);
    setLoading(false);
  }, []);

  useEffect(() => {
    void loadAuthors();
  }, [loadAuthors]);

  const handleCreate = async () => {
    if (!newAuthorName.trim()) return;
    setSaving(true);
    try {
      await authorCreate(newAuthorName.trim());
      setCreateDialogOpen(false);
      setNewAuthorName('');
      void loadAuthors();
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
    if (!editingId || !editName.trim()) return;
    try {
      await authorUpdate(editingId, editName.trim());
      setEditingId(null);
      setEditName('');
      void loadAuthors();
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
    try {
      await authorDelete(deletingAuthor.id);
      setDeleteDialogOpen(false);
      setDeletingAuthor(null);
      void loadAuthors();
    } catch (error) {
      console.error('Failed to delete author:', error);
      alert(`Failed to delete author: ${error}`);
    }
    setDeleting(false);
  };

  return (
    <div className="flex h-screen bg-background">
      <main className="flex-1 flex flex-col overflow-hidden">
        <div
          className="flex items-center justify-between px-8 pt-14 pb-4 border-b border-border"
          data-tauri-drag-region
        >
          <div className="flex items-center gap-4">
            <Link to="/">
              <Button variant="ghost" size="sm">
                <ArrowLeft className="h-4 w-4 mr-1" />
                Back
              </Button>
            </Link>
            <div>
              <h1 className="text-xl font-bold text-foreground flex items-center gap-2">
                <User className="h-5 w-5" />
                Authors
              </h1>
              <p className="text-xs text-muted-foreground mt-0.5">{authors.length} authors</p>
            </div>
          </div>
          <Button onClick={() => setCreateDialogOpen(true)}>
            <Plus className="h-4 w-4 mr-1" />
            Add Author
          </Button>
        </div>

        <div className="flex-1 overflow-auto px-8 py-6">
          {loading ? (
            <div className="flex items-center justify-center h-32">
              <p className="text-sm text-muted-foreground">Loading...</p>
            </div>
          ) : (
            <div className="rounded-md border">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead className="w-[100px]">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {authors.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={2} className="h-24 text-center">
                        No authors yet. Click "Add Author" to create one.
                      </TableCell>
                    </TableRow>
                  ) : (
                    authors.map((author) => (
                      <TableRow key={author.id}>
                        <TableCell>
                          {editingId === author.id ? (
                            <Input
                              value={editName}
                              onChange={(e) => setEditName(e.target.value)}
                              className="h-8 w-full"
                              autoFocus
                            />
                          ) : (
                            <Link
                              to="/authors/$authorId"
                              params={{ authorId: String(author.id) }}
                              className="text-foreground hover:text-primary hover:underline underline-offset-4"
                            >
                              {author.name}
                              {author.usageCount > 0 && (
                                <span className="ml-2 text-xs text-muted-foreground font-serif-italic">
                                  — {author.usageCount}
                                </span>
                              )}
                            </Link>
                          )}
                        </TableCell>
                        <TableCell>
                          {editingId === author.id ? (
                            <div className="flex gap-1">
                              <Button size="sm" onClick={handleSaveEdit}>
                                Save
                              </Button>
                              <Button size="sm" variant="ghost" onClick={handleCancelEdit}>
                                Cancel
                              </Button>
                            </div>
                          ) : (
                            <div className="flex gap-1">
                              <Button
                                size="icon"
                                variant="ghost"
                                className="h-8 w-8"
                                onClick={() => handleStartEdit(author)}
                              >
                                <Pencil className="h-4 w-4" />
                              </Button>
                              <Button
                                size="icon"
                                variant="ghost"
                                className="h-8 w-8 text-destructive"
                                onClick={() => handleDeleteClick(author)}
                              >
                                <Trash2 className="h-4 w-4" />
                              </Button>
                            </div>
                          )}
                        </TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
            </div>
          )}
        </div>
      </main>

      <Dialog open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create Author</DialogTitle>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <Input
              value={newAuthorName}
              onChange={(e) => setNewAuthorName(e.target.value)}
              placeholder="Author name"
              onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
            />
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
              Are you sure you want to delete "{deletingAuthor?.name}"?
              {deletingAuthor && deletingAuthor.usageCount > 0 && (
                <> This will remove the author from {deletingAuthor.usageCount} files.</>
              )}
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
    </div>
  );
}
