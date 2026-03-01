import { invoke } from '@tauri-apps/api/core';
import type {
  FileEntry,
  FileWithDetails,
  FileListRequest,
  FileCreateRequest,
  FileSearchRequest,
  Category,
  Tag,
  Metadata,
} from '@/types';

export async function fileList(params?: FileListRequest): Promise<{ files: FileEntry[]; total: number }> {
  return invoke('file_list', params || {});
}

export async function fileGet(id: number): Promise<FileWithDetails> {
  return invoke('file_get', { id });
}

export async function fileCreate(params: FileCreateRequest): Promise<{ id: number }> {
  return invoke('file_create', params);
}

export async function fileUpdate(id: number, params: { displayName?: string; categoryId?: number | null }): Promise<{ success: boolean }> {
  return invoke('file_update', { id, ...params });
}

export async function fileDelete(id: number): Promise<{ success: boolean }> {
  return invoke('file_delete', { id });
}

export async function fileSearch(params: FileSearchRequest): Promise<{ files: FileEntry[]; total: number }> {
  return invoke('file_search', params);
}

export async function fileCheckStatus(fileIds?: number[]): Promise<{ updated: Array<{ id: number; status: string }> }> {
  return invoke('file_check_status', { fileIds });
}

export async function categoryList(): Promise<Category[]> {
  return invoke('category_list');
}

export async function categoryGet(id: number): Promise<Category> {
  return invoke('category_get', { id });
}

export async function categoryCreate(name: string, icon?: string): Promise<{ id: number }> {
  return invoke('category_create', { name, icon: icon || null });
}

export async function categoryUpdate(id: number, name?: string, icon?: string): Promise<{ success: boolean }> {
  return invoke('category_update', { id, name, icon: icon || null });
}

export async function categoryDelete(id: number): Promise<{ success: boolean; affectedFiles: number }> {
  return invoke('category_delete', { id });
}

export async function tagList(includeUsage?: boolean): Promise<{ tags: (Tag & { usageCount: number })[] }> {
  return invoke('tag_list', { includeUsage: includeUsage || false });
}

export async function tagCreate(name: string, color?: string): Promise<{ id: number }> {
  return invoke('tag_create', { name, color: color || null });
}

export async function tagUpdate(id: number, name?: string, color?: string): Promise<{ success: boolean }> {
  return invoke('tag_update', { id, name, color: color || null });
}

export async function tagDelete(id: number): Promise<{ success: boolean; affectedFiles: number }> {
  return invoke('tag_delete', { id });
}

export async function tagAssign(fileId: number, tagIds: number[]): Promise<{ success: boolean }> {
  return invoke('tag_assign', { fileId, tagIds });
}

export async function tagUnassign(fileId: number, tagIds: number[]): Promise<{ success: boolean }> {
  return invoke('tag_unassign', { fileId, tagIds });
}

export async function metadataGet(fileId: number): Promise<{ metadata: Metadata[] }> {
  return invoke('metadata_get', { fileId });
}

export async function metadataSet(
  fileId: number,
  key: string,
  value: string,
  dataType?: string
): Promise<{ id: number }> {
  return invoke('metadata_set', { fileId, key, value, dataType: dataType || 'text' });
}

export async function metadataDelete(fileId: number, key: string): Promise<{ success: boolean }> {
  return invoke('metadata_delete', { fileId, key });
}