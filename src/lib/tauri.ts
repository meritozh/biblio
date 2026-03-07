import { invoke } from '@tauri-apps/api/core';
import type {
  FileEntry,
  FileWithDetails,
  FileListRequest,
  FileCreateRequest,
  FileSearchRequest,
  Category,
  Tag,
  Author,
  Metadata,
} from '@/types';

export async function fileList(params?: FileListRequest): Promise<{ files: FileEntry[]; total: number }> {
  return invoke('file_list', {
    categoryId: params?.category_id,
    tagIds: params?.tag_ids,
    status: params?.status,
    limit: params?.limit,
    offset: params?.offset,
  });
}

export async function fileGet(id: number): Promise<FileWithDetails> {
  return invoke('file_get', { id });
}

export async function fileCreate(params: FileCreateRequest): Promise<{ id: number }> {
  return invoke('file_create', {
    path: params.path,
    displayName: params.display_name,
    categoryId: params.category_id,
    tagIds: params.tag_ids,
    authorIds: params.author_ids,
    metadata: params.metadata,
  });
}

export async function fileUpdate(id: number, params: { display_name?: string; category_id?: number | null }): Promise<{ success: boolean }> {
  return invoke('file_update', {
    id,
    displayName: params.display_name,
    categoryId: params.category_id,
  });
}

export async function fileDelete(id: number): Promise<{ success: boolean }> {
  return invoke('file_delete', { id });
}

export async function fileSearch(params: FileSearchRequest): Promise<{ files: FileEntry[]; total: number }> {
  return invoke('file_search', {
    query: params.query,
    categoryId: params.category_id,
    tagIds: params.tag_ids,
    metadataFilters: params.metadata_filters,
    limit: params.limit,
    offset: params.offset,
  });
}

export async function fileCheckStatus(file_ids?: number[]): Promise<{ updated: Array<{ id: number; status: string }> }> {
  return invoke('file_check_status', { fileIds: file_ids });
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

export async function tagAssign(file_id: number, tag_ids: number[]): Promise<{ success: boolean }> {
  return invoke('tag_assign', { fileId: file_id, tagIds: tag_ids });
}

export async function tagUnassign(file_id: number, tag_ids: number[]): Promise<{ success: boolean }> {
  return invoke('tag_unassign', { fileId: file_id, tagIds: tag_ids });
}

export async function metadataGet(file_id: number): Promise<{ metadata: Metadata[] }> {
  return invoke('metadata_get', { fileId: file_id });
}

export async function metadataSet(
  file_id: number,
  key: string,
  value: string,
  data_type?: string
): Promise<{ id: number }> {
  return invoke('metadata_set', { fileId: file_id, key, value, dataType: data_type || 'text' });
}

export async function metadataDelete(file_id: number, key: string): Promise<{ success: boolean }> {
  return invoke('metadata_delete', { fileId: file_id, key });
}

// Author API functions
export async function authorList(includeUsage?: boolean): Promise<{ authors: (Author & { usageCount: number })[] }> {
  return invoke('author_list', { includeUsage: includeUsage || false });
}

export async function authorCreate(name: string): Promise<{ id: number }> {
  return invoke('author_create', { name });
}

export async function authorUpdate(id: number, name: string): Promise<{ success: boolean }> {
  return invoke('author_update', { id, name });
}

export async function authorDelete(id: number): Promise<{ success: boolean; affectedFiles: number }> {
  return invoke('author_delete', { id });
}

export async function authorAssign(file_id: number, author_ids: number[]): Promise<{ success: boolean }> {
  return invoke('author_assign', { fileId: file_id, authorIds: author_ids });
}

export async function authorUnassign(file_id: number, author_ids: number[]): Promise<{ success: boolean }> {
  return invoke('author_unassign', { fileId: file_id, authorIds: author_ids });
}

export async function authorSet(file_id: number, author_ids: number[]): Promise<{ success: boolean }> {
  return invoke('author_set', { fileId: file_id, authorIds: author_ids });
}

// Cover API functions
export async function coverSet(file_id: number, data: number[], mimeType?: string): Promise<{ success: boolean }> {
  return invoke('cover_set', { fileId: file_id, data, mimeType: mimeType || null });
}

export async function coverGet(file_id: number): Promise<{ data: string; mimeType: string }> {
  return invoke('cover_get', { fileId: file_id });
}

export async function coverDelete(file_id: number): Promise<{ success: boolean }> {
  return invoke('cover_delete', { fileId: file_id });
}