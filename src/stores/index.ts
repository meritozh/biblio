import { invoke } from '@tauri-apps/api/core';
import type { FileEntry } from '@/types';

export async function fetchFiles(params?: {
  categoryId?: number | null;
}): Promise<{ files: FileEntry[]; total: number }> {
  try {
    return await invoke<{ files: FileEntry[]; total: number }>('file_list', params ?? {});
  } catch (error) {
    console.error('Failed to fetch files:', error);
    return { files: [], total: 0 };
  }
}

export async function fetchCategories(): Promise<
  { id: number; name: string; icon: string | null; isDefault: boolean; createdAt: string }[]
> {
  try {
    return await invoke('category_list');
  } catch (error) {
    console.error('Failed to fetch categories:', error);
    return [];
  }
}
