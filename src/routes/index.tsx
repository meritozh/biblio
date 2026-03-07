import { createFileRoute } from '@tanstack/react-router';
import { useEffect, useState, useCallback } from 'react';
import { FilePicker } from '@/components/FilePicker';
import { FileList } from '@/components/FileList';
import { CategorySidebar } from '@/components/CategorySidebar';
import { fetchFiles, fetchCategories } from '@/stores';
import { fileCreate, authorList, authorCreate, tagCreate, coverSet } from '@/lib/tauri';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import {
  DynamicMetadataForm,
  type DynamicMetadataFormValues,
} from '@/components/DynamicMetadataForm';
import type { FileEntry, Category, Tag, Author } from '@/types';

export const Route = createFileRoute('/')({
  component: HomePage,
});

function HomePage() {
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [selectedCategoryId, setSelectedCategoryId] = useState<number | null>(null);
  const [categories, setCategories] = useState<Category[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [authors, setAuthors] = useState<Author[]>([]);
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);
  const [formValues, setFormValues] = useState<DynamicMetadataFormValues>({
    display_name: '',
    category_id: null,
    tag_ids: [],
    author_ids: [],
    metadata: [],
  });

  const loadFiles = useCallback(async (categoryId: number | null) => {
    setLoading(true);
    const result = await fetchFiles({ category_id: categoryId });
    setFiles(result.files);
    setTotal(result.total);
    setLoading(false);
  }, []);

  const loadCategories = useCallback(async () => {
    const result = await fetchCategories();
    setCategories(result);
  }, []);

  const loadAuthors = useCallback(async () => {
    const result = await authorList(true);
    setAuthors(result.authors);
  }, []);

  // Initial data fetch on mount
  useEffect(() => {
    void loadCategories();
    void loadAuthors();
    void loadFiles(null);
  }, [loadCategories, loadAuthors, loadFiles]);

  // Reload files when category changes
  useEffect(() => {
    void loadFiles(selectedCategoryId);
  }, [selectedCategoryId, loadFiles]);

  const handleFilesSelected = (paths: string[]) => {
    setSelectedFiles(paths);
    if (paths.length === 1 && paths[0]) {
      const path = paths[0];
      const fileName = path.substring(Math.max(0, path.lastIndexOf('/') + 1)) || path;
      setFormValues({
        display_name: fileName,
        category_id: null,
        tag_ids: [],
        author_ids: [],
        metadata: [],
      });
    } else {
      setFormValues({
        display_name: '',
        category_id: null,
        tag_ids: [],
        author_ids: [],
        metadata: [],
      });
    }
    setAddDialogOpen(true);
  };

  const handleAddFile = async () => {
    if (saving) return;
    setSaving(true);
    try {
      for (const path of selectedFiles) {
        const defaultName = path.substring(Math.max(0, path.lastIndexOf('/') + 1)) || path;
        const display_name = selectedFiles.length === 1 ? formValues.display_name : defaultName;

        const result = await fileCreate({
          path,
          display_name,
          category_id: formValues.category_id,
          tag_ids: formValues.tag_ids,
          author_ids: formValues.author_ids,
          metadata: formValues.metadata,
        });

        // Upload cover image if provided
        if (formValues.cover_data && result.id) {
          // Convert base64 to byte array
          const binaryString = atob(formValues.cover_data);
          const bytes = new Uint8Array(binaryString.length);
          for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
          }
          await coverSet(result.id, Array.from(bytes));
        }
      }
      setAddDialogOpen(false);
      setSelectedFiles([]);
      setFormValues({
        display_name: '',
        category_id: null,
        tag_ids: [],
        author_ids: [],
        metadata: [],
      });
      void loadFiles(selectedCategoryId);
    } catch (error) {
      console.error('Failed to add file:', error);
      alert(`Failed to add file: ${error}`);
    }
    setSaving(false);
  };

  const handleFileClick = (file: FileEntry) => {
    console.log('File clicked:', file);
  };

  const handleCategoryCreated = (newCategory: Category) => {
    setCategories((prev) => [...prev, newCategory]);
  };

  const handleTagCreate = async (name: string): Promise<Tag> => {
    const result = await tagCreate(name);
    const newTag: Tag = {
      id: result.id,
      name,
      color: null,
      created_at: new Date().toISOString(),
    };
    setTags((prev) => [...prev, newTag]);
    return newTag;
  };

  const handleAuthorCreate = async (name: string): Promise<Author> => {
    const result = await authorCreate(name);
    const newAuthor: Author = {
      id: result.id,
      name,
      created_at: new Date().toISOString(),
    };
    setAuthors((prev) => [...prev, newAuthor]);
    return newAuthor;
  };

  return (
    <div className="flex h-screen">
      <CategorySidebar
        categories={categories}
        selectedCategoryId={selectedCategoryId}
        onCategorySelect={setSelectedCategoryId}
      />
      <main className="flex-1 p-6 overflow-auto">
        <div className="flex justify-between items-center mb-6">
          <div>
            <h1 className="text-2xl font-bold">Library</h1>
            <p className="text-muted-foreground">{total} files</p>
          </div>
          <FilePicker onFilesSelected={handleFilesSelected} />
        </div>
        {loading ? (
          <div className="flex items-center justify-center h-64">
            <p className="text-muted-foreground">Loading...</p>
          </div>
        ) : (
          <FileList files={files} onFileClick={handleFileClick} />
        )}
      </main>

      <Dialog open={addDialogOpen} onOpenChange={setAddDialogOpen}>
        <DialogContent className="max-w-md max-h-[90vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>Add Files to Library</DialogTitle>
          </DialogHeader>
          {selectedFiles.length === 1 ? (
            <DynamicMetadataForm
              values={formValues}
              onChange={setFormValues}
              categories={categories}
              tags={tags}
              authors={authors}
              onCategoryCreated={handleCategoryCreated}
              onTagCreate={handleTagCreate}
              onAuthorCreate={handleAuthorCreate}
            />
          ) : (
            <div className="py-4">
              <p className="text-sm text-muted-foreground">
                Adding {selectedFiles.length} files. Each file will use its filename as the display name.
              </p>
              <div className="mt-4 space-y-2">
                <label className="text-sm font-medium">Category</label>
                <DynamicMetadataForm
                  values={formValues}
                  onChange={setFormValues}
                  categories={categories}
                  tags={tags}
                  authors={authors}
                  onCategoryCreated={handleCategoryCreated}
                  onTagCreate={handleTagCreate}
                  onAuthorCreate={handleAuthorCreate}
                />
              </div>
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setAddDialogOpen(false)}>
              Cancel
            </Button>
            <Button onClick={handleAddFile} disabled={saving}>
              {saving ? 'Adding...' : 'Add to Library'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}