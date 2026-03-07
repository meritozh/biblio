import { useState, useEffect } from 'react';
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
import { AuthorManager } from '@/components/AuthorManager';
import { MetadataEditor } from '@/components/MetadataEditor';
import { fileUpdate, authorSet, tagAssign, metadataSet, metadataDelete } from '@/lib/tauri';
import type { FileEntry, Category, Tag, Author, Metadata, MetadataType } from '@/types';

interface FileEditDialogProps {
  file: FileEntry | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onFileUpdated: () => Promise<void>;
  onTagCreate?: (name: string) => Promise<Tag>;
  onAuthorCreate?: (name: string) => Promise<Author>;
}

export function FileEditDialog({
  file,
  open,
  onOpenChange,
  categories,
  tags,
  authors,
  onFileUpdated,
  onTagCreate,
  onAuthorCreate,
}: FileEditDialogProps) {
  const [display_name, setDisplayName] = useState('');
  const [category_id, setCategoryId] = useState<number | null>(null);
  const [selectedTagIds, setSelectedTagIds] = useState<number[]>([]);
  const [selectedAuthorIds, setSelectedAuthorIds] = useState<number[]>([]);
  const [metadata, setMetadata] = useState<Metadata[]>([]);
  const [saving, setSaving] = useState(false);

  // Reset form when file changes
  useEffect(() => {
    if (file && open) {
      setDisplayName(file.display_name);
      setCategoryId(file.category_id);
      setSelectedTagIds(file.tags?.map((t) => t.id) || []);
      setSelectedAuthorIds(file.authors?.map((a) => a.id) || []);
      setMetadata(file.metadata || []);
    }
  }, [file, open]);

  const handleSave = async () => {
    if (!file) return;

    setSaving(true);
    try {
      // Update basic file info
      await fileUpdate(file.id, {
        display_name,
        category_id,
      });

      // Update tags
      await tagAssign(file.id, selectedTagIds);

      // Update authors
      await authorSet(file.id, selectedAuthorIds);

      await onFileUpdated();
      onOpenChange(false);
    } catch (error) {
      console.error('Failed to save file:', error);
      alert(`Failed to save file: ${error}`);
    }
    setSaving(false);
  };

  const handleMetadataUpdate = async (key: string, value: string, dataType?: string) => {
    if (!file) return;
    const typedDataType = (dataType || 'text') as MetadataType;
    await metadataSet(file.id, key, value, typedDataType);
    // Refresh metadata
    const updatedMeta = metadata.map((m) =>
      m.key === key ? { ...m, value, data_type: typedDataType } : m
    );
    if (!metadata.find((m) => m.key === key)) {
      updatedMeta.push({
        id: -1,
        file_id: file.id,
        key,
        value,
        data_type: typedDataType,
      });
    }
    setMetadata(updatedMeta);
  };

  const handleMetadataDelete = async (key: string) => {
    if (!file) return;
    await metadataDelete(file.id, key);
    setMetadata(metadata.filter((m) => m.key !== key));
  };

  if (!file) return null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md max-h-[90vh] overflow-y-auto">
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
            <label className="text-sm font-medium">Authors</label>
            <AuthorManager
              authors={authors}
              selectedAuthorIds={selectedAuthorIds}
              onAuthorAssign={setSelectedAuthorIds}
              onAuthorCreate={onAuthorCreate}
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">Tags</label>
            <TagManager
              tags={tags}
              selectedTagIds={selectedTagIds}
              onTagAssign={setSelectedTagIds}
              onTagCreate={onTagCreate}
            />
          </div>
          <MetadataEditor
            metadata={metadata}
            onUpdate={handleMetadataUpdate}
            onDelete={handleMetadataDelete}
          />
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