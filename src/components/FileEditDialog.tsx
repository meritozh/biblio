import { useState, useEffect } from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { DynamicMetadataForm, type DynamicMetadataFormValues } from '@/components/DynamicMetadataForm';
import { fileUpdate, authorSet, tagAssign, fileMoveCategory, translateError } from '@/lib/tauri';
import type { FileEntry, Category, Tag, Author } from '@/types';

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
  onCategoryCreated?: (category: Category) => void;
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
  onCategoryCreated,
}: FileEditDialogProps) {
  const [formValues, setFormValues] = useState<DynamicMetadataFormValues>({
    display_name: '',
    category_id: null,
    tag_ids: [],
    author_ids: [],
    metadata: [],
  });
  const [saving, setSaving] = useState(false);
  const [moveError, setMoveError] = useState<string | null>(null);

  // Reset form when file changes
  useEffect(() => {
    if (file && open) {
      setFormValues({
        display_name: file.display_name,
        category_id: file.category_id,
        tag_ids: file.tags?.map((t) => t.id) || [],
        author_ids: file.authors?.map((a) => a.id) || [],
        metadata: (file.metadata || []).map((m) => ({
          key: m.key,
          value: m.value,
          data_type: m.data_type,
        })),
      });
      setMoveError(null);
    }
  }, [file, open]);

  const handleSave = async () => {
    if (!file) return;

    setSaving(true);
    try {
      // Update basic file info
      await fileUpdate(file.id, {
        display_name: formValues.display_name,
        category_id: formValues.category_id,
      });

      // Update tags
      await tagAssign(file.id, formValues.tag_ids);

      // Update authors
      await authorSet(file.id, formValues.author_ids);

      await onFileUpdated();
      onOpenChange(false);
    } catch (error) {
      console.error('Failed to save file:', error);
      alert(`Failed to save file: ${error}`);
    }
    setSaving(false);
  };

  const handleCategoryChange = async (newCategoryId: number | null) => {
    if (!file) return;
    setMoveError(null);
    try {
      await fileMoveCategory(file.id, newCategoryId);
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      setMoveError(translateError(errorMsg));
      throw error; // Re-throw to let the form know it failed
    }
  };

  if (!file) return null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Edit File</DialogTitle>
        </DialogHeader>
        <div className="space-y-4 py-4">
          <DynamicMetadataForm
            values={formValues}
            onChange={setFormValues}
            categories={categories}
            tags={tags}
            authors={authors}
            onCategoryCreated={onCategoryCreated}
            onTagCreate={onTagCreate}
            onAuthorCreate={onAuthorCreate}
            fileId={file?.id}
            inStorage={file?.in_storage}
            onCategoryChange={handleCategoryChange}
          />
          {moveError && (
            <p className="text-sm text-red-500 mt-2">{moveError}</p>
          )}
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