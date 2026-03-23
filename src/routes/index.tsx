import { createFileRoute } from '@tanstack/react-router';
import { useEffect, useState, useCallback } from 'react';
import { FilePicker } from '@/components/FilePicker';
import { FileList } from '@/components/FileList';
import { CategorySidebar } from '@/components/CategorySidebar';
import { fetchFiles, fetchCategories } from '@/stores';
import { fileCreate, authorList, authorCreate, tagList, tagCreate, coverSet, storageGetPath, storageCheckAccess } from '@/lib/tauri';
import { Button } from '@/components/ui/button';
import { AlertCircle } from 'lucide-react';
import { SettingsDialog } from '@/components/SettingsDialog';
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
  const [storagePathConfigured, setStoragePathConfigured] = useState<boolean | null>(null);
  const [storagePathAccessible, setStoragePathAccessible] = useState(true);
  const [settingsOpen, setSettingsOpen] = useState(false);

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

  const loadTags = useCallback(async () => {
    const result = await tagList(true);
    setTags(result.tags);
  }, []);

  const checkStoragePath = useCallback(async () => {
    const path = await storageGetPath();
    if (path && path !== '') {
      const accessible = await storageCheckAccess();
      setStoragePathConfigured(true);
      setStoragePathAccessible(accessible);
    } else {
      setStoragePathConfigured(false);
      setStoragePathAccessible(true);
    }
  }, []);

  useEffect(() => {
    void loadCategories();
    void loadTags();
    void loadAuthors();
    void loadFiles(null);
    void checkStoragePath();
  }, [loadCategories, loadTags, loadAuthors, loadFiles, checkStoragePath]);

  useEffect(() => {
    void loadFiles(selectedCategoryId);
  }, [selectedCategoryId, loadFiles]);

  const handleSettingsOpenChange = useCallback((open: boolean) => {
    setSettingsOpen(open);
    if (!open) {
      void checkStoragePath();
    }
  }, [checkStoragePath]);

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

        if (formValues.cover_data && result.id) {
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
    <div className="flex h-screen bg-background">
      <CategorySidebar
        categories={categories}
        selectedCategoryId={selectedCategoryId}
        onCategorySelect={setSelectedCategoryId}
      />
      <main className="flex-1 p-8 overflow-auto">
        <div className="max-w-6xl mx-auto">
          <div className="flex justify-between items-start mb-8">
            <div>
              <h1 className="text-3xl font-semibold text-foreground mb-1">Library</h1>
              <p className="text-muted-foreground text-sm">{total} files</p>
            </div>
            <div className="flex items-center gap-3">
              <SettingsDialog open={settingsOpen} onOpenChange={handleSettingsOpenChange} />
              <FilePicker onFilesSelected={handleFilesSelected} disabled={storagePathConfigured === false} />
            </div>
          </div>

          {storagePathConfigured === false && (
            <div className="mb-6 p-5 bg-secondary/50 border border-border rounded-xl flex items-center gap-4">
              <div className="p-2 rounded-full bg-secondary">
                <AlertCircle className="h-5 w-5 text-accent" />
              </div>
              <div className="flex-1">
                <p className="text-sm font-medium text-foreground">
                  Storage path not configured
                </p>
                <p className="text-sm text-muted-foreground mt-0.5">
                  Please configure a storage folder before adding files.
                </p>
              </div>
              <Button variant="outline" size="sm" onClick={() => setSettingsOpen(true)}>
                Configure
              </Button>
            </div>
          )}

          {storagePathConfigured === true && !storagePathAccessible && (
            <div className="mb-6 p-5 bg-destructive/5 border border-destructive/20 rounded-xl flex items-center gap-4">
              <div className="p-2 rounded-full bg-destructive/10">
                <AlertCircle className="h-5 w-5 text-destructive" />
              </div>
              <div className="flex-1">
                <p className="text-sm font-medium text-foreground">
                  Storage path inaccessible
                </p>
                <p className="text-sm text-muted-foreground mt-0.5">
                  The configured storage folder cannot be accessed. Please check if it exists and you have permission to read/write.
                </p>
              </div>
              <Button variant="outline" size="sm" onClick={() => setSettingsOpen(true)}>
                Reconfigure
              </Button>
            </div>
          )}

          {loading ? (
            <div className="flex items-center justify-center h-64">
              <p className="text-muted-foreground">Loading...</p>
            </div>
          ) : (
            <FileList files={files} onFileClick={handleFileClick} />
          )}
        </div>
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