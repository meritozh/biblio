import { createFileRoute } from '@tanstack/react-router';
import { useEffect, useState, useCallback } from 'react';
import { FilePicker } from '@/components/FilePicker';
import { FileList } from '@/components/FileList';
import { CategorySidebar } from '@/components/CategorySidebar';
import { fetchFiles, fetchCategories } from '@/stores';
import { fileCreate } from '@/lib/tauri';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { CategorySelect } from '@/components/CategorySelect';
import type { FileEntry, Category } from '@/types';

export const Route = createFileRoute('/')({
  component: HomePage,
});

function HomePage() {
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [selectedCategoryId, setSelectedCategoryId] = useState<number | null>(null);
  const [categories, setCategories] = useState<Category[]>([]);
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [selectedFiles, setSelectedFiles] = useState<string[]>([]);
  const [newFileName, setNewFileName] = useState('');
  const [newFileCategory, setNewFileCategory] = useState<number | null>(null);

  const loadFiles = useCallback(async (categoryId: number | null) => {
    setLoading(true);
    const result = await fetchFiles({ categoryId });
    setFiles(result.files);
    setTotal(result.total);
    setLoading(false);
  }, []);

  const loadCategories = useCallback(async () => {
    const result = await fetchCategories();
    setCategories(result);
  }, []);

  useEffect(() => {
    loadCategories();
    loadFiles(null);
  }, [loadCategories, loadFiles]);

  useEffect(() => {
    loadFiles(selectedCategoryId);
  }, [selectedCategoryId, loadFiles]);

  const handleFilesSelected = (paths: string[]) => {
    setSelectedFiles(paths);
    if (paths.length === 1 && paths[0]) {
      const path = paths[0];
      const fileName = path.substring(Math.max(0, path.lastIndexOf('/') + 1)) || path;
      setNewFileName(fileName);
    }
    setAddDialogOpen(true);
  };

  const handleAddFile = async () => {
    for (const path of selectedFiles) {
      const defaultName = path.substring(Math.max(0, path.lastIndexOf('/') + 1)) || path;
      const displayName = selectedFiles.length === 1 ? newFileName : defaultName;
      await fileCreate({
        path,
        displayName,
        categoryId: newFileCategory,
      });
    }
    setAddDialogOpen(false);
    setSelectedFiles([]);
    setNewFileName('');
    setNewFileCategory(null);
    loadFiles(selectedCategoryId);
  };

  const handleFileClick = (file: FileEntry) => {
    console.log('File clicked:', file);
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
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Add Files to Library</DialogTitle>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <p className="text-sm text-muted-foreground">
              Adding {selectedFiles.length} file{selectedFiles.length !== 1 ? 's' : ''}
            </p>
            {selectedFiles.length === 1 && (
              <>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Display Name</label>
                  <Input value={newFileName} onChange={(e) => setNewFileName(e.target.value)} />
                </div>
                <div className="space-y-2">
                  <label className="text-sm font-medium">Category</label>
                  <CategorySelect
                    categories={categories}
                    value={newFileCategory}
                    onValueChange={setNewFileCategory}
                  />
                </div>
              </>
            )}
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setAddDialogOpen(false)}>
              Cancel
            </Button>
            <Button onClick={handleAddFile}>Add to Library</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
