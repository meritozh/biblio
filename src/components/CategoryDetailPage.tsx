import { useEffect, useState, useCallback } from 'react';
import { FileList } from '@/components/FileList';
import { invoke } from '@tauri-apps/api/core';
import { useView } from '@/hooks/useView';
import type { FileEntry, Category } from '@/types';

interface CategoryDetailPageProps {
  categoryId: string;
}

export function CategoryDetailPage({ categoryId }: CategoryDetailPageProps) {
  const id = parseInt(categoryId, 10);
  const [category, setCategory] = useState<Category | null>(null);

  const fetcher = useCallback(async () => {
    return await invoke<{ files: FileEntry[]; total: number }>('file_list', {
      categoryId: id,
    });
  }, [id]);

  const { ids, total, loading } = useView(`cat-detail::${id}`, fetcher);

  useEffect(() => {
    invoke<Category>('category_get', { id })
      .then(setCategory)
      .catch((error) => console.error('Failed to load category:', error));
  }, [id]);

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
      <FileList ids={ids} total={total} onFileClick={handleFileClick} />
    </div>
  );
}
