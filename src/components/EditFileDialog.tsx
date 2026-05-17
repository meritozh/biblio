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
// No schema prop: the form resolves the schema from
// `formValues.category_id` so the section list reactively follows the
// user's category choice. When the file's existing category isn't in
// the list (rare race), the form falls back to the default schema.
import { fileGet } from '@/lib/tauri';
import type { FileEntry, Category, Tag, Author } from '@/types';

interface EditFileDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  file: FileEntry | null;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onTagCreate: (name: string) => Promise<Tag>;
  onAuthorCreate: (name: string) => Promise<Author>;
  onSave: (fileId: number, values: DynamicMetadataFormValues) => Promise<void>;
  onCategoryChange?: (newCategoryId: number | null) => Promise<void>;
}

export function EditFileDialog({
  open,
  onOpenChange,
  file,
  categories,
  tags,
  authors,
  onTagCreate,
  onAuthorCreate,
  onSave,
  onCategoryChange,
}: EditFileDialogProps) {
  const [saving, setSaving] = useState(false);
  const [formValues, setFormValues] = useState<DynamicMetadataFormValues>({
    display_name: '',
    category_id: null,
    tag_ids: [],
    author_ids: [],
    metadata: [],
    progress: '',
  });

  // Populate form when file changes or dialog opens.
  // Fetch full details (including the metadata array) because the file list
  // only ships a flat `description` field — other metadata keys (e.g. comics
  // `volume`) are only available via file_get.
  useEffect(() => {
    if (!file || !open) return;

    let cancelled = false;

    // Seed immediately with what we already know so the dialog doesn't
    // flash empty while the fetch is in flight.
    setFormValues({
      display_name: file.display_name,
      category_id: file.category_id,
      tag_ids: file.tags?.map((t) => t.id) ?? [],
      author_ids: file.authors?.map((a) => a.id) ?? [],
      metadata:
        file.description != null && file.description !== ''
          ? [{ key: 'description', value: file.description, data_type: 'text' }]
          : [],
      progress: file.progress ?? '',
    });

    // Cover is no longer loaded here — DynamicMetadataForm's
    // `ExistingCoverPreview` self-fetches via fileId, mirroring the grid
    // card's pattern. This keeps "user hasn't touched the cover" distinct
    // from "user removed the cover" without overloading cover_data.
    void fileGet(file.id)
      .then((details) => {
        if (cancelled) return;
        setFormValues({
          display_name: details.display_name,
          category_id: details.category_id,
          tag_ids: details.tags.map((t) => t.id),
          author_ids: details.authors.map((a) => a.id),
          metadata: details.metadata.map((m) => ({
            key: m.key,
            value: m.value,
            data_type: m.data_type,
          })),
          progress: details.progress ?? '',
        });
      })
      .catch((error) => {
        console.error('Failed to load file details:', error);
      });

    return () => {
      cancelled = true;
    };
  }, [file, open]);

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
      <DialogContent
        className="max-w-md max-h-[90vh] overflow-y-auto"
        onInteractOutside={(e) => e.preventDefault()}
        onEscapeKeyDown={(e) => e.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>Edit File</DialogTitle>
        </DialogHeader>
        <DynamicMetadataForm
          values={formValues}
          onChange={setFormValues}
          categories={categories}
          tags={tags}
          authors={authors}
          onTagCreate={onTagCreate}
          onAuthorCreate={onAuthorCreate}
          fileId={file?.id}
          inStorage={file?.in_storage}
          onCategoryChange={onCategoryChange}
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