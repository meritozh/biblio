import { invoke } from '@tauri-apps/api/core';
import type { FileEntry, Category } from '@/types';

export async function fetchFiles(params?: {
  category_id?: number | null;
}): Promise<{ files: FileEntry[]; total: number }> {
  try {
    return await invoke<{ files: FileEntry[]; total: number }>('file_list', {
      categoryId: params?.category_id,
    });
  } catch (error) {
    console.error('Failed to fetch files:', error);
    return { files: [], total: 0 };
  }
}

export async function fetchCategories(): Promise<Category[]> {
  try {
    return await invoke('category_list');
  } catch (error) {
    console.error('Failed to fetch categories:', error);
    return [];
  }
}
