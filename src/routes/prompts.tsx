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
import { Plus, Pencil, Trash2, ArrowLeft, MessageSquare, Star } from 'lucide-react';
import {
  promptList,
  promptCreate,
  promptUpdate,
  promptDelete,
  promptSetDefault,
} from '@/lib/tauri';
import type { Prompt } from '@/types';

const CATEGORY_OPTIONS = [
  { value: '', label: 'Generic (all categories)' },
  { value: 'Novels', label: 'Novels' },
  { value: 'Comics', label: 'Comics' },
];

export const Route = createFileRoute('/prompts')({
  component: PromptsPage,
});

function PromptsPage() {
  const [prompts, setPrompts] = useState<Prompt[]>([]);
  const [loading, setLoading] = useState(true);
  const [filterCategory, setFilterCategory] = useState<string>('');
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [editDialogOpen, setEditDialogOpen] = useState(false);
  const [newPromptName, setNewPromptName] = useState('');
  const [newPromptContent, setNewPromptContent] = useState('');
  const [newPromptCategory, setNewPromptCategory] = useState<string>('');
  const [saving, setSaving] = useState(false);
  const [editingPrompt, setEditingPrompt] = useState<Prompt | null>(null);
  const [editName, setEditName] = useState('');
  const [editContent, setEditContent] = useState('');
  const [editCategory, setEditCategory] = useState<string>('');
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletingPrompt, setDeletingPrompt] = useState<Prompt | null>(null);
  const [deleting, setDeleting] = useState(false);

  const loadPrompts = useCallback(async (category?: string) => {
    setLoading(true);
    const result = await promptList(category || undefined);
    setPrompts(result);
    setLoading(false);
  }, []);

  useEffect(() => {
    void loadPrompts(filterCategory || undefined);
  }, [loadPrompts, filterCategory]);

  const handleCreate = async () => {
    if (!newPromptName.trim() || !newPromptContent.trim()) return;
    setSaving(true);
    try {
      await promptCreate({
        name: newPromptName.trim(),
        content: newPromptContent.trim(),
        category: newPromptCategory || null,
      });
      setCreateDialogOpen(false);
      setNewPromptName('');
      setNewPromptContent('');
      setNewPromptCategory('');
      void loadPrompts(filterCategory || undefined);
    } catch (error) {
      console.error('Failed to create prompt:', error);
      alert(`Failed to create prompt: ${error}`);
    }
    setSaving(false);
  };

  const handleStartEdit = (prompt: Prompt) => {
    setEditingPrompt(prompt);
    setEditName(prompt.name);
    setEditContent(prompt.content);
    setEditCategory(prompt.category ?? '');
    setEditDialogOpen(true);
  };

  const handleSaveEdit = async () => {
    if (!editingPrompt || !editName.trim() || !editContent.trim()) return;
    setSaving(true);
    try {
      await promptUpdate(editingPrompt.id, {
        name: editName.trim(),
        content: editContent.trim(),
        category: editCategory || null,
      });
      setEditDialogOpen(false);
      setEditingPrompt(null);
      setEditName('');
      setEditContent('');
      setEditCategory('');
      void loadPrompts(filterCategory || undefined);
    } catch (error) {
      console.error('Failed to update prompt:', error);
      alert(`Failed to update prompt: ${error}`);
    }
    setSaving(false);
  };

  const handleDeleteClick = (prompt: Prompt) => {
    setDeletingPrompt(prompt);
    setDeleteDialogOpen(true);
  };

  const handleConfirmDelete = async () => {
    if (!deletingPrompt) return;
    setDeleting(true);
    try {
      await promptDelete(deletingPrompt.id);
      setDeleteDialogOpen(false);
      setDeletingPrompt(null);
      void loadPrompts(filterCategory || undefined);
    } catch (error) {
      console.error('Failed to delete prompt:', error);
      alert(`Failed to delete prompt: ${error}`);
    }
    setDeleting(false);
  };

  const handleSetDefault = async (prompt: Prompt) => {
    try {
      await promptSetDefault(prompt.id);
      void loadPrompts(filterCategory || undefined);
    } catch (error) {
      console.error('Failed to set default prompt:', error);
      alert(`Failed to set default prompt: ${error}`);
    }
  };

  const truncateText = (text: string, maxLength: number = 150) => {
    if (text.length <= maxLength) return text;
    return text.substring(0, maxLength) + '...';
  };

  const getCategoryBadgeVariant = (category: string | null) => {
    switch (category) {
      case 'Novels':
        return 'blue' as const;
      case 'Comics':
        return 'purple' as const;
      default:
        return 'secondary' as const;
    }
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
                <MessageSquare className="h-5 w-5" />
                Prompts
              </h1>
              <p className="text-xs text-muted-foreground mt-0.5">{prompts.length} prompts</p>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <select
              value={filterCategory}
              onChange={(e) => setFilterCategory(e.target.value)}
              className="h-9 rounded-xl border border-input bg-background px-3 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
            >
              <option value="">All Categories</option>
              {CATEGORY_OPTIONS.filter((o) => o.value).map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
            <Button onClick={() => setCreateDialogOpen(true)}>
              <Plus className="h-4 w-4 mr-1" />
              Add Prompt
            </Button>
          </div>
        </div>

        <div className="flex-1 overflow-auto px-8 py-6">
          {loading ? (
            <div className="flex items-center justify-center h-32">
              <p className="text-sm text-muted-foreground">Loading...</p>
            </div>
          ) : prompts.length === 0 ? (
            <div className="flex items-center justify-center h-32">
              <p className="text-sm text-muted-foreground">
                No prompts yet. Click "Add Prompt" to create one.
              </p>
            </div>
          ) : (
            <div className="grid gap-4">
              {prompts.map((prompt) => (
                <div
                  key={prompt.id}
                  className="rounded-xl border border-border bg-card p-4 hover:shadow-sm transition-shadow"
                >
                  <div className="flex items-start justify-between gap-4">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-2">
                        <h3 className="text-base font-semibold text-foreground truncate">
                          {prompt.name}
                        </h3>
                        {prompt.category && (
                          <Badge
                            variant={getCategoryBadgeVariant(prompt.category)}
                            className="text-xs"
                          >
                            {prompt.category}
                          </Badge>
                        )}
                        {prompt.is_default && (
                          <Badge variant="green" className="text-xs">
                            Default
                          </Badge>
                        )}
                      </div>
                      <p className="text-xs text-muted-foreground line-clamp-3">
                        {truncateText(prompt.content)}
                      </p>
                    </div>
                    <div className="flex items-center gap-1 shrink-0">
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-8 w-8"
                        onClick={() => handleStartEdit(prompt)}
                      >
                        <Pencil className="h-4 w-4" />
                      </Button>
                      {!prompt.is_default && (
                        <Button
                          size="icon"
                          variant="ghost"
                          className="h-8 w-8"
                          onClick={() => handleSetDefault(prompt)}
                          title="Set as default"
                        >
                          <Star className="h-4 w-4" />
                        </Button>
                      )}
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-8 w-8 text-destructive"
                        onClick={() => handleDeleteClick(prompt)}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </main>

      <Dialog open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>Create Prompt</DialogTitle>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div>
              <label className="text-sm font-medium mb-2 block">Name</label>
              <Input
                value={newPromptName}
                onChange={(e) => setNewPromptName(e.target.value)}
                placeholder="Prompt name"
              />
            </div>
            <div>
              <label className="text-sm font-medium mb-2 block">Category</label>
              <select
                value={newPromptCategory}
                onChange={(e) => setNewPromptCategory(e.target.value)}
                className="w-full h-9 rounded-xl border border-input bg-background px-3 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
              >
                {CATEGORY_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="text-sm font-medium mb-2 block">Content</label>
              <textarea
                value={newPromptContent}
                onChange={(e) => setNewPromptContent(e.target.value)}
                placeholder="Prompt content..."
                className="w-full min-h-[200px] px-3 py-2 text-sm rounded-xl border border-input bg-background focus:outline-none focus:ring-2 focus:ring-ring resize-y"
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setCreateDialogOpen(false)}>
              Cancel
            </Button>
            <Button
              onClick={handleCreate}
              disabled={saving || !newPromptName.trim() || !newPromptContent.trim()}
            >
              {saving ? 'Creating...' : 'Create'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={editDialogOpen} onOpenChange={setEditDialogOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>Edit Prompt</DialogTitle>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <div>
              <label className="text-sm font-medium mb-2 block">Name</label>
              <Input
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
                placeholder="Prompt name"
              />
            </div>
            <div>
              <label className="text-sm font-medium mb-2 block">Category</label>
              <select
                value={editCategory}
                onChange={(e) => setEditCategory(e.target.value)}
                className="w-full h-9 rounded-xl border border-input bg-background px-3 text-sm focus:outline-none focus:ring-2 focus:ring-ring"
              >
                {CATEGORY_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="text-sm font-medium mb-2 block">Content</label>
              <textarea
                value={editContent}
                onChange={(e) => setEditContent(e.target.value)}
                placeholder="Prompt content..."
                className="w-full min-h-[200px] px-3 py-2 text-sm rounded-xl border border-input bg-background focus:outline-none focus:ring-2 focus:ring-ring resize-y"
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setEditDialogOpen(false)}>
              Cancel
            </Button>
            <Button
              onClick={handleSaveEdit}
              disabled={saving || !editName.trim() || !editContent.trim()}
            >
              {saving ? 'Saving...' : 'Save'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <AlertDialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete Prompt</AlertDialogTitle>
            <AlertDialogDescription>
              Are you sure you want to delete "{deletingPrompt?.name}"?
              {deletingPrompt?.is_default && (
                <span className="text-destructive font-medium">
                  This is the default prompt and cannot be deleted.
                </span>
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
              disabled={deleting || deletingPrompt?.is_default}
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
