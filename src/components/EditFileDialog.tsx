import { useEffect, useState } from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { DynamicMetadataForm, type DynamicMetadataFormValues } from '@/components/DynamicMetadataForm';
import type { FileEntry, Category, Tag, Author } from '@/types';

interface EditFileDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  file: FileEntry | null;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onCategoryCreated: (category: Category) => void;
  onTagCreate: (name: string) => Promise<Tag>;
  onAuthorCreate: (name: string) => Promise<Author>;
  onSave: (fileId: number, values: DynamicMetadataFormValues) => Promise<void>;
}

export function EditFileDialog({
  open,
  onOpenChange,
  file,
  categories,
  tags,
  authors,
  onCategoryCreated,
  onTagCreate,
  onAuthorCreate,
  onSave,
}: EditFileDialogProps) {
  const [saving, setSaving] = useState(false);
  const [formValues, setFormValues] = useState<DynamicMetadataFormValues>({
    display_name: '',
    category_id: null,
    tag_ids: [],
    author_ids: [],
    metadata: [],
  });

  // Populate form when file changes
  useEffect(() => {
    if (file) {
      setFormValues({
        display_name: file.display_name,
        category_id: file.category_id,
        tag_ids: file.tags?.map((t) => t.id) ?? [],
        author_ids: file.authors?.map((a) => a.id) ?? [],
        metadata: file.metadata?.map((m) => ({
          key: m.key,
          value: m.value,
          data_type: m.data_type,
        })) ?? [],
      });
    }
  }, [file]);

  const handleSave = async () => {
    if (!file || saving) return;
    setSaving(true);
    try {
      await onSave(file.id, formValues);
      onOpenChange(false);
    } catch (error) {
      console.error('Failed to save:', error);
      alert(`Failed to save: ${error}`);
    }
    setSaving(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>Edit File</DialogTitle>
        </DialogHeader>
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
        />
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