import { invoke } from '@tauri-apps/api/core';
import type { FileEntry, Category } from '@/types';

export async function fetchFiles(params?: {
  category_id?: number | null;
  /** Free-text search query. When non-empty, routes through the FTS-backed
   *  file_search command; otherwise the default paginated file_list. */
  query?: string;
  limit?: number;
  offset?: number;
}): Promise<{ files: FileEntry[]; total: number }> {
  const query = params?.query?.trim();
  try {
    if (query) {
      return await invoke<{ files: FileEntry[]; total: number }>('file_search', {
        query,
        categoryId: params?.category_id,
        limit: params?.limit,
        offset: params?.offset,
      });
    }
    return await invoke<{ files: FileEntry[]; total: number }>('file_list', {
      categoryId: params?.category_id,
      limit: params?.limit,
      offset: params?.offset,
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
