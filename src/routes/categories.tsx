import { createFileRoute } from '@tanstack/react-router';
import { useState, useEffect, useCallback } from 'react';
import { Link } from '@tanstack/react-router';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
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
import { Plus, Pencil, Trash2, ArrowLeft, FolderOpen } from 'lucide-react';
import { categoryList, categoryCreate, categoryUpdate, categoryDelete } from '@/lib/tauri';
import type { Category } from '@/types';

export const Route = createFileRoute('/categories')({
  component: CategoriesPage,
});

function CategoriesPage() {
  const [categories, setCategories] = useState<Category[]>([]);
  const [loading, setLoading] = useState(true);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [newCategoryName, setNewCategoryName] = useState('');
  const [newCategoryDescription, setNewCategoryDescription] = useState('');
  const [saving, setSaving] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editName, setEditName] = useState('');
  const [editDescription, setEditDescription] = useState('');
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletingCategory, setDeletingCategory] = useState<Category | null>(null);
  const [deleting, setDeleting] = useState(false);

  const loadCategories = useCallback(async () => {
    setLoading(true);
    const result = await categoryList();
    setCategories(result);
    setLoading(false);
  }, []);

  useEffect(() => {
    void loadCategories();
  }, [loadCategories]);

  const handleCreate = async () => {
    if (!newCategoryName.trim()) return;
    setSaving(true);
    try {
      await categoryCreate(newCategoryName.trim(), undefined, newCategoryDescription.trim() || undefined);
      setCreateDialogOpen(false);
      setNewCategoryName('');
      setNewCategoryDescription('');
      void loadCategories();
    } catch (error) {
      console.error('Failed to create category:', error);
      alert(`Failed to create category: ${error}`);
    }
    setSaving(false);
  };

  const handleStartEdit = (category: Category) => {
    setEditingId(category.id);
    setEditName(category.name);
    setEditDescription(category.description ?? '');
  };

  const handleSaveEdit = async () => {
    if (!editingId || !editName.trim()) return;
    try {
      await categoryUpdate(editingId, editName.trim(), undefined, editDescription.trim() || undefined);
      setEditingId(null);
      setEditName('');
      setEditDescription('');
      void loadCategories();
    } catch (error) {
      console.error('Failed to update category:', error);
      alert(`Failed to update category: ${error}`);
    }
  };

  const handleCancelEdit = () => {
    setEditingId(null);
    setEditName('');
    setEditDescription('');
  };

  const handleDeleteClick = (category: Category) => {
    setDeletingCategory(category);
    setDeleteDialogOpen(true);
  };

  const handleConfirmDelete = async () => {
    if (!deletingCategory) return;
    setDeleting(true);
    try {
      await categoryDelete(deletingCategory.id);
      setDeleteDialogOpen(false);
      setDeletingCategory(null);
      void loadCategories();
    } catch (error) {
      console.error('Failed to delete category:', error);
      alert(`Failed to delete category: ${error}`);
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
                <FolderOpen className="h-5 w-5" />
                Categories
              </h1>
              <p className="text-xs text-muted-foreground mt-0.5">
                {categories.length} categories
              </p>
            </div>
          </div>
          <Button onClick={() => setCreateDialogOpen(true)}>
            <Plus className="h-4 w-4 mr-1" />
            Add Category
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
                    <TableHead>Description</TableHead>
                    <TableHead className="w-[150px]">Folder</TableHead>
                    <TableHead className="w-[100px]">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {categories.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={4} className="h-24 text-center">
                        No categories yet. Click "Add Category" to create one.
                      </TableCell>
                    </TableRow>
                  ) : (
                    categories.map((category) => (
                      <TableRow key={category.id}>
                        <TableCell>
                          <div className="flex items-center gap-2">
                            {editingId === category.id ? (
                              <Input
                                value={editName}
                                onChange={(e) => setEditName(e.target.value)}
                                className="h-8 w-full"
                                autoFocus
                              />
                            ) : (
                              <>
                                {category.name}
                                {category.is_default && (
                                  <Badge variant="secondary" className="text-xs">
                                    Default
                                  </Badge>
                                )}
                              </>
                            )}
                          </div>
                        </TableCell>
                        <TableCell>
                          {editingId === category.id ? (
                            <Input
                              value={editDescription}
                              onChange={(e) => setEditDescription(e.target.value)}
                              className="h-8 w-full"
                              placeholder="Short description for LLM"
                            />
                          ) : (
                            <span className="text-xs text-muted-foreground">
                              {category.description || '-'}
                            </span>
                          )}
                        </TableCell>
                        <TableCell>
                          <span className="text-xs text-muted-foreground font-mono">
                            {category.folder_name || '-'}
                          </span>
                        </TableCell>
                        <TableCell>
                          {editingId === category.id ? (
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
                                onClick={() => handleStartEdit(category)}
                              >
                                <Pencil className="h-4 w-4" />
                              </Button>
                              {!category.is_default && (
                                <Button
                                  size="icon"
                                  variant="ghost"
                                  className="h-8 w-8 text-destructive"
                                  onClick={() => handleDeleteClick(category)}
                                >
                                  <Trash2 className="h-4 w-4" />
                                </Button>
                              )}
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
            <DialogTitle>Create Category</DialogTitle>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div>
              <label className="text-sm font-medium mb-2 block">Name</label>
              <Input
                value={newCategoryName}
                onChange={(e) => setNewCategoryName(e.target.value)}
                placeholder="Category name"
              />
            </div>
            <div>
              <label className="text-sm font-medium mb-2 block">Description</label>
              <Input
                value={newCategoryDescription}
                onChange={(e) => setNewCategoryDescription(e.target.value)}
                placeholder="e.g., Chinese web novels, light novels, manga..."
              />
              <p className="text-xs text-muted-foreground mt-1.5">
                Helps the LLM pick the right category during import.
              </p>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setCreateDialogOpen(false)}>
              Cancel
            </Button>
            <Button onClick={handleCreate} disabled={saving || !newCategoryName.trim()}>
              {saving ? 'Creating...' : 'Create'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <AlertDialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete Category</AlertDialogTitle>
            <AlertDialogDescription>
              Are you sure you want to delete "{deletingCategory?.name}"? Categories with files
              cannot be deleted.
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
