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

// Settings API functions
export async function settingsGet(key: string): Promise<string | null> {
  return invoke('settings_get', { key });
}

export async function settingsSet(key: string, value: string): Promise<void> {
  return invoke('settings_set', { key, value });
}

export async function storageGetPath(): Promise<string | null> {
  return invoke('storage_get_path');
}

export async function storageCheckAccess(): Promise<boolean> {
  return invoke('storage_check_access');
}

// File category move function
export async function fileMoveCategory(file_id: number, new_category_id: number | null): Promise<{ success: boolean }> {
  return invoke('file_move_category', { fileId: file_id, newCategoryId: new_category_id });
}

// Error code translations
const ERROR_MESSAGES: Record<string, string> = {
  'STORAGE_PATH_NOT_CONFIGURED': 'Please configure a storage folder in settings first.',
  'STORAGE_PATH_CHANGE_BLOCKED': 'Cannot change storage path while files are stored. Remove all files first.',
  'STORAGE_PATH_NOT_FOUND': 'The selected folder does not exist.',
  'STORAGE_PATH_NOT_WRITABLE': 'Cannot write to the selected folder. Please choose another location.',
  'STORAGE_PATH_SYSTEM_DIRECTORY': 'Cannot use a system directory. Please choose another location.',
  'SOURCE_FILE_NOT_FOUND': 'The source file could not be found.',
  'FILE_ALREADY_IN_STORAGE': 'This file is already in the managed storage.',
  'FILE_NOT_IN_STORAGE': 'This file is not in managed storage.',
  'FILE_NOT_FOUND': 'The file was not found.',
  'CATEGORY_HAS_FILES': 'Cannot delete category with files. Move or delete files first.',
  'CATEGORY_NOT_FOUND': 'The selected category was not found.',
  'CATEGORY_FOLDER_NOT_SET': 'The category folder is not configured.',
  'CANNOT_DELETE_DEFAULT': 'Cannot delete the default category.',
  'PERMISSION_DENIED': 'Permission denied. Please check folder permissions.',
  'FILE_LOCKED': 'File is in use by another application.',
  'DISK_FULL': 'Not enough disk space to complete the operation.',
};

export function translateError(error: string): string {
  return ERROR_MESSAGES[error] || error;
}