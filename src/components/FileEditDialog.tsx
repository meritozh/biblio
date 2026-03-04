import { useState } from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { CategorySelect } from '@/components/CategorySelect';
import { TagManager } from '@/components/TagManager';
import type { FileEntry, Category, Tag } from '@/types';

interface FileEditDialogProps {
  file: FileEntry | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  categories: Category[];
  tags: Tag[];
  onSave: (file: FileEntry) => Promise<void>;
}

export function FileEditDialog({
  file,
  open,
  onOpenChange,
  categories,
  tags,
  onSave,
}: FileEditDialogProps) {
  const [display_name, setDisplayName] = useState('');
  const [category_id, setCategoryId] = useState<number | null>(null);
  const [selectedTagIds, setSelectedTagIds] = useState<number[]>([]);
  const [saving, setSaving] = useState(false);

  useState(() => {
    if (file) {
      setDisplayName(file.display_name);
      setCategoryId(file.category_id);
      setSelectedTagIds(file.tags?.map((t) => t.id) || []);
    }
  });

  const handleSave = async () => {
    if (!file) return;

    setSaving(true);
    try {
      await onSave({
        ...file,
        display_name,
        category_id,
        tags: tags.filter((t) => selectedTagIds.includes(t.id)),
      });
      onOpenChange(false);
    } catch (error) {
      console.error('Failed to save file:', error);
    }
    setSaving(false);
  };

  if (!file) return null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Edit File</DialogTitle>
        </DialogHeader>
        <div className="space-y-4 py-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">Display Name</label>
            <Input
              value={display_name}
              onChange={(e) => setDisplayName(e.target.value)}
              placeholder="File name"
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Category</label>
            <CategorySelect
              categories={categories}
              value={category_id}
              onValueChange={setCategoryId}
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Tags</label>
            <TagManager
              tags={tags}
              selectedTagIds={selectedTagIds}
              onTagAssign={setSelectedTagIds}
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={saving}>
            {saving ? 'Saving...' : 'Save'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
