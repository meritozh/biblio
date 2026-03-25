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
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <main className="flex-1 flex flex-col overflow-hidden">
        <div className="flex items-center justify-between px-8 pt-14 pb-4 border-b border-border" data-tauri-drag-region>
          <div>
            <h1 className="text-xl font-semibold text-foreground">Library</h1>
            <p className="text-xs text-muted-foreground mt-0.5">{total} files</p>
          </div>
          <FilePicker onFilesSelected={handleFilesSelected} disabled={storagePathConfigured === false} />
        </div>

        <div className="flex-1 overflow-auto px-8 py-6">
          {storagePathConfigured === false && (
            <div className="mb-6 p-4 bg-secondary/50 rounded flex items-center gap-3">
              <AlertCircle className="h-4 w-4 text-muted-foreground shrink-0" />
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium text-foreground">
                  Storage path not configured
                </p>
                <p className="text-xs text-muted-foreground mt-0.5">
                  Select a storage folder to start adding files.
                </p>
              </div>
              <Button variant="secondary" size="sm" onClick={() => setSettingsOpen(true)}>
                Configure
              </Button>
            </div>
          )}

          {storagePathConfigured === true && !storagePathAccessible && (
            <div className="mb-6 p-4 bg-destructive/5 rounded flex items-center gap-3">
              <AlertCircle className="h-4 w-4 text-destructive shrink-0" />
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium text-foreground">
                  Storage path inaccessible
                </p>
                <p className="text-xs text-muted-foreground mt-0.5">
                  The storage folder cannot be accessed.
                </p>
              </div>
              <Button variant="secondary" size="sm" onClick={() => setSettingsOpen(true)}>
                Reconfigure
              </Button>
            </div>
          )}

          {loading ? (
            <div className="flex items-center justify-center h-32">
              <p className="text-sm text-muted-foreground">Loading...</p>
            </div>
          ) : (
            <FileList files={files} onFileClick={handleFileClick} />
          )}
        </div>
      </main>

      <SettingsDialog open={settingsOpen} onOpenChange={handleSettingsOpenChange} />

      <Dialog open={addDialogOpen} onOpenChange={setAddDialogOpen}>
        <DialogContent className="max-w-md max-h-[90vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>Add Files</DialogTitle>
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
                Adding {selectedFiles.length} files.
              </p>
              <div className="mt-4">
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
              {saving ? 'Adding...' : 'Add'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
