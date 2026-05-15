import { createFileRoute } from '@tanstack/react-router';
import { useState, useEffect } from 'react';
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
import { Plus, Pencil, Trash2, FolderOpen } from 'lucide-react';
import { categoryCreate, categoryUpdate, categoryDelete } from '@/lib/tauri';
import { loadCategories, useAppState } from '@/stores/appStore';
import { SCHEMA_LABELS, coerceSchemaSlug } from '@/lib/categorySchema';
import type { Category, SchemaSlug } from '@/types';

export const Route = createFileRoute('/categories')({
  component: CategoriesPage,
});

function CategoriesPage() {
  const categories = useAppState((s) => s.categories);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [newCategoryName, setNewCategoryName] = useState('');
  const [newCategoryDescription, setNewCategoryDescription] = useState('');
  const [newCategorySchema, setNewCategorySchema] = useState<SchemaSlug>('novel');
  const [saving, setSaving] = useState(false);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editName, setEditName] = useState('');
  const [editDescription, setEditDescription] = useState('');
  const [editSchema, setEditSchema] = useState<SchemaSlug>('novel');
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletingCategory, setDeletingCategory] = useState<Category | null>(null);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    void loadCategories();
  }, []);

  const handleCreate = async () => {
    if (!newCategoryName.trim()) return;
    setSaving(true);
    try {
      await categoryCreate(
        newCategoryName.trim(),
        undefined,
        newCategoryDescription.trim() || undefined,
        newCategorySchema
      );
      setCreateDialogOpen(false);
      setNewCategoryName('');
      setNewCategoryDescription('');
      setNewCategorySchema('novel');
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
    setEditSchema(coerceSchemaSlug(category.schema_slug));
  };

  const handleSaveEdit = async () => {
    if (!editingId || !editName.trim()) return;
    try {
      await categoryUpdate(
        editingId,
        editName.trim(),
        undefined,
        editDescription.trim() || undefined,
        editSchema
      );
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
    <>
      <div
        className="flex items-end justify-between px-8 pt-14 pb-5 border-b border-border"
        data-tauri-drag-region
      >
        <div className="flex items-baseline gap-3">
          <h1 className="text-3xl text-foreground flex items-center gap-3">
            <FolderOpen className="h-6 w-6 text-muted-foreground" aria-hidden="true" />
            Categories
          </h1>
          <span className="font-serif-italic text-sm text-muted-foreground">
            — {categories.length} {categories.length === 1 ? 'category' : 'categories'}
          </span>
        </div>
        <Button onClick={() => setCreateDialogOpen(true)}>
          <Plus className="h-4 w-4 mr-1" />
          Add Category
        </Button>
      </div>

      <div className="flex-1 overflow-auto px-8 py-6">
        <div className="rounded-md border">
          <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>Description</TableHead>
                    <TableHead className="w-[120px]">Schema</TableHead>
                    <TableHead className="w-[150px]">Folder</TableHead>
                    <TableHead className="w-[100px]">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {categories.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={5} className="h-24 text-center">
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
                          {editingId === category.id ? (
                            <select
                              value={editSchema}
                              onChange={(e) => setEditSchema(e.target.value as SchemaSlug)}
                              className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs"
                            >
                              {(Object.keys(SCHEMA_LABELS) as SchemaSlug[]).map((slug) => (
                                <option key={slug} value={slug}>
                                  {SCHEMA_LABELS[slug]}
                                </option>
                              ))}
                            </select>
                          ) : (
                            <Badge variant="gray" className="text-xs">
                              {SCHEMA_LABELS[coerceSchemaSlug(category.schema_slug)]}
                            </Badge>
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
      </div>

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
            <div>
              <label className="text-sm font-medium mb-2 block">Schema</label>
              <select
                value={newCategorySchema}
                onChange={(e) => setNewCategorySchema(e.target.value as SchemaSlug)}
                className="w-full h-9 rounded-md border border-input bg-background px-3 text-sm"
              >
                {(Object.keys(SCHEMA_LABELS) as SchemaSlug[]).map((slug) => (
                  <option key={slug} value={slug}>
                    {SCHEMA_LABELS[slug]}
                  </option>
                ))}
              </select>
              <p className="text-xs text-muted-foreground mt-1.5">
                Decides which form fields appear in import / edit dialogs,
                what the file card shows, and which prompts the LLM runs.
                Pick <em>Novel</em> for text-based libraries, <em>Comic</em>
                for image-based ones.
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
    </>
  );
}
