import { useEffect, useState } from 'react';
import { FileList } from '@/components/FileList';
import { invoke } from '@tauri-apps/api/core';
import type { FileEntry, Category } from '@/types';

interface CategoryDetailPageProps {
  categoryId: string;
}

export function CategoryDetailPage({ categoryId }: CategoryDetailPageProps) {
  const [files, setFiles] = useState<FileEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [category, setCategory] = useState<Category | null>(null);

  useEffect(() => {
    const id = parseInt(categoryId, 10);

    const loadCategory = async () => {
      try {
        const cat = await invoke<Category>('category_get', { id });
        setCategory(cat);
      } catch (error) {
        console.error('Failed to load category:', error);
      }
    };

    const loadFiles = async () => {
      setLoading(true);
      try {
        const response = await invoke<{ files: FileEntry[]; total: number }>('file_list', {
          category_id: id,
        });
        setFiles(response.files);
        setTotal(response.total);
      } catch (error) {
        console.error('Failed to load files:', error);
      }
      setLoading(false);
    };

    loadCategory();
    loadFiles();
  }, [categoryId]);

  const handleFileClick = (file: FileEntry) => {
    console.log('File clicked:', file);
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <p className="text-muted-foreground">Loading...</p>
      </div>
    );
  }

  return (
    <div className="p-6">
      <div className="mb-6">
        <h1 className="text-2xl font-bold">
          {category?.icon && <span className="mr-2">{category.icon}</span>}
          {category?.name ?? 'Category'}
        </h1>
        <p className="text-muted-foreground">{total} files</p>
      </div>
      <FileList files={files} onFileClick={handleFileClick} />
    </div>
  );
}
