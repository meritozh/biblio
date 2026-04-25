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
import { Plus, Pencil, Trash2, ArrowLeft, MessageSquare, Star, Info } from 'lucide-react';
import {
  promptList,
  promptCreate,
  promptUpdate,
  promptDelete,
  promptSetDefault,
} from '@/lib/tauri';
import type { Prompt, PromptMimeGroup, PromptStep } from '@/types';

interface PromptFormState {
  mimeGroup: PromptMimeGroup;
  step: PromptStep;
}

const STEPS_BY_GROUP: Record<PromptMimeGroup, ReadonlyArray<{ step: PromptStep; label: string }>> = {
  text: [
    { step: 'filename', label: 'Filename extraction' },
    { step: 'content', label: 'Content analysis' },
  ],
  archive: [
    { step: 'filename', label: 'Filename extraction' },
    { step: 'cover_pick', label: 'Cover detection' },
  ],
};

const GROUP_LABEL: Record<PromptMimeGroup, string> = {
  text: 'Novel',
  archive: 'Comic',
};

const STEP_LABEL: Record<PromptStep, string> = {
  filename: 'filename',
  content: 'content',
  cover_pick: 'cover',
};

function isValidStep(group: PromptMimeGroup, step: PromptStep): boolean {
  return STEPS_BY_GROUP[group].some((s) => s.step === step);
}

function promptHelpText(group: PromptMimeGroup, step: PromptStep): string {
  if (group === 'text' && step === 'content') {
    return 'Categories, tags, and authors from your library are automatically appended at runtime.';
  }
  if (group === 'text' && step === 'filename') {
    return 'Filename extraction has no runtime context — this prompt is used verbatim.';
  }
  if (group === 'archive' && step === 'filename') {
    return 'Used for comic archive filenames (.zip / .cbz). No runtime context appended.';
  }
  return 'Used to rank candidate cover filenames inside a comic archive. No runtime context appended.';
}

export const Route = createFileRoute('/prompts')({
  component: PromptsPage,
});

function PromptsPage() {
  const [prompts, setPrompts] = useState<Prompt[]>([]);
  const [loading, setLoading] = useState(true);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [editDialogOpen, setEditDialogOpen] = useState(false);
  const [newPromptName, setNewPromptName] = useState('');
  const [newPromptContent, setNewPromptContent] = useState('');
  const [newPromptForm, setNewPromptForm] = useState<PromptFormState>({
    mimeGroup: 'text',
    step: 'content',
  });
  const [saving, setSaving] = useState(false);
  const [editingPrompt, setEditingPrompt] = useState<Prompt | null>(null);
  const [editName, setEditName] = useState('');
  const [editContent, setEditContent] = useState('');
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletingPrompt, setDeletingPrompt] = useState<Prompt | null>(null);
  const [deleting, setDeleting] = useState(false);

  const loadPrompts = useCallback(async () => {
    setLoading(true);
    const result = await promptList();
    setPrompts(result);
    setLoading(false);
  }, []);

  useEffect(() => {
    void loadPrompts();
  }, [loadPrompts]);

  const handleCreate = async () => {
    if (!newPromptName.trim() || !newPromptContent.trim()) return;
    setSaving(true);
    try {
      await promptCreate({
        name: newPromptName.trim(),
        content: newPromptContent.trim(),
        mime_group: newPromptForm.mimeGroup,
        step: newPromptForm.step,
      });
      setCreateDialogOpen(false);
      setNewPromptName('');
      setNewPromptContent('');
      setNewPromptForm({ mimeGroup: 'text', step: 'content' });
      void loadPrompts();
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
    setEditDialogOpen(true);
  };

  const handleSaveEdit = async () => {
    if (!editingPrompt || !editName.trim() || !editContent.trim()) return;
    setSaving(true);
    try {
      await promptUpdate(editingPrompt.id, {
        name: editName.trim(),
        content: editContent.trim(),
        mime_group: editingPrompt.mime_group,
        step: editingPrompt.step,
      });
      setEditDialogOpen(false);
      setEditingPrompt(null);
      setEditName('');
      setEditContent('');
      void loadPrompts();
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
      void loadPrompts();
    } catch (error) {
      console.error('Failed to delete prompt:', error);
      alert(`Failed to delete prompt: ${error}`);
    }
    setDeleting(false);
  };

  const handleSetDefault = async (prompt: Prompt) => {
    try {
      await promptSetDefault(prompt.id);
      void loadPrompts();
    } catch (error) {
      console.error('Failed to set default prompt:', error);
      alert(`Failed to set default prompt: ${error}`);
    }
  };

  const truncateText = (text: string, maxLength: number = 150) => {
    if (text.length <= maxLength) return text;
    return text.substring(0, maxLength) + '...';
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
          <Button onClick={() => setCreateDialogOpen(true)}>
            <Plus className="h-4 w-4 mr-1" />
            Add Prompt
          </Button>
        </div>

        <div className="flex-1 overflow-auto px-8 py-6">
          {/* Info banner */}
          <div className="flex items-start gap-2 rounded-lg border border-border bg-muted/30 p-3 mb-6">
            <Info className="h-4 w-4 text-muted-foreground mt-0.5 shrink-0" />
            <p className="text-xs text-muted-foreground">
              Prompts are grouped by mime type. Novels (.txt / .epub / .pdf) use a filename
              and a content prompt; comics (.zip / .cbz) use a filename and a cover-detection
              prompt. The active prompt within each (group, step) bucket runs at import time.
            </p>
          </div>

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
                  className={`rounded-xl border bg-card p-4 hover:shadow-sm transition-shadow ${
                    prompt.is_default ? 'border-primary/30 bg-primary/[0.02]' : 'border-border'
                  }`}
                >
                  <div className="flex items-start justify-between gap-4">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-2">
                        <h3 className="text-base font-semibold text-foreground truncate">
                          {prompt.name}
                        </h3>
                        <Badge variant="gray" className="text-xs">
                          {GROUP_LABEL[prompt.mime_group]} · {STEP_LABEL[prompt.step]}
                        </Badge>
                        {prompt.is_default && (
                          <Badge variant="green" className="text-xs">
                            Active
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
                          title="Set as active prompt"
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
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="text-sm font-medium mb-2 block">Mime group</label>
                <div className="flex flex-col gap-2">
                  {(Object.keys(GROUP_LABEL) as PromptMimeGroup[]).map((g) => (
                    <label key={g} className="flex items-center gap-2 cursor-pointer">
                      <input
                        type="radio"
                        name="new-prompt-mime-group"
                        checked={newPromptForm.mimeGroup === g}
                        onChange={() => {
                          // Reset step to the first valid one for the new group.
                          const firstStep = STEPS_BY_GROUP[g][0]!.step;
                          setNewPromptForm({ mimeGroup: g, step: firstStep });
                        }}
                        className="accent-primary"
                      />
                      <span className="text-sm">{GROUP_LABEL[g]}</span>
                    </label>
                  ))}
                </div>
              </div>
              <div>
                <label className="text-sm font-medium mb-2 block">Step</label>
                <div className="flex flex-col gap-2">
                  {STEPS_BY_GROUP[newPromptForm.mimeGroup].map(({ step, label }) => (
                    <label key={step} className="flex items-center gap-2 cursor-pointer">
                      <input
                        type="radio"
                        name="new-prompt-step"
                        checked={newPromptForm.step === step}
                        onChange={() =>
                          setNewPromptForm((prev) => ({ ...prev, step }))
                        }
                        className="accent-primary"
                      />
                      <span className="text-sm">{label}</span>
                    </label>
                  ))}
                </div>
              </div>
            </div>
            <div>
              <label className="text-sm font-medium mb-2 block">Name</label>
              <Input
                value={newPromptName}
                onChange={(e) => setNewPromptName(e.target.value)}
                placeholder="Prompt name"
              />
            </div>
            <div>
              <label className="text-sm font-medium mb-2 block">Content</label>
              <textarea
                value={newPromptContent}
                onChange={(e) => setNewPromptContent(e.target.value)}
                placeholder="Prompt content..."
                className="w-full min-h-[200px] px-3 py-2 text-sm rounded-xl border border-input bg-background focus:outline-none focus:ring-2 focus:ring-ring resize-y"
              />
              <p className="text-xs text-muted-foreground mt-1.5">
                {promptHelpText(newPromptForm.mimeGroup, newPromptForm.step)}
              </p>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setCreateDialogOpen(false)}>
              Cancel
            </Button>
            <Button
              onClick={handleCreate}
              disabled={
                saving ||
                !newPromptName.trim() ||
                !newPromptContent.trim() ||
                !isValidStep(newPromptForm.mimeGroup, newPromptForm.step)
              }
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
              <label className="text-sm font-medium mb-2 block">Type</label>
              <p className="text-sm text-muted-foreground">
                {editingPrompt
                  ? `${GROUP_LABEL[editingPrompt.mime_group]} · ${
                      STEPS_BY_GROUP[editingPrompt.mime_group].find(
                        (s) => s.step === editingPrompt.step
                      )?.label ?? editingPrompt.step
                    }`
                  : ''}
              </p>
            </div>
            <div>
              <label className="text-sm font-medium mb-2 block">Name</label>
              <Input
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
                placeholder="Prompt name"
              />
            </div>
            <div>
              <label className="text-sm font-medium mb-2 block">Content</label>
              <textarea
                value={editContent}
                onChange={(e) => setEditContent(e.target.value)}
                placeholder="Prompt content..."
                className="w-full min-h-[200px] px-3 py-2 text-sm rounded-xl border border-input bg-background focus:outline-none focus:ring-2 focus:ring-ring resize-y"
              />
              <p className="text-xs text-muted-foreground mt-1.5">
                {editingPrompt
                  ? promptHelpText(editingPrompt.mime_group, editingPrompt.step)
                  : ''}
              </p>
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
                  {' '}This is the active prompt and cannot be deleted.
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
