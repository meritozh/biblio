import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
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
  FilePreparedImport,
  ProcessingProgress,
  LlmConfig,
  Prompt,
  PromptCreatePayload,
  RemoteConfig,
  RemoteUploadProgress,
  RemoteDownloadProgress,
  RemoteDeleteProgress,
} from '@/types';

export async function fileList(
  params?: FileListRequest
): Promise<{ files: FileEntry[]; total: number }> {
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
    progress: params.progress,
    coverData: params.cover_data
      ? Array.from(atob(params.cover_data), (c) => c.charCodeAt(0))
      : null,
    coverMimeType: params.cover_mime_type || null,
    stagedCoverPath: params.staged_cover_path ?? null,
  });
}

export async function fileReplace(
  existingFileId: number,
  params: FileCreateRequest
): Promise<{ id: number }> {
  return invoke('file_replace', {
    existingFileId,
    path: params.path,
    displayName: params.display_name,
    categoryId: params.category_id,
    tagIds: params.tag_ids,
    authorIds: params.author_ids,
    metadata: params.metadata,
    progress: params.progress,
    coverData: params.cover_data
      ? Array.from(atob(params.cover_data), (c) => c.charCodeAt(0))
      : null,
    coverMimeType: params.cover_mime_type || null,
    stagedCoverPath: params.staged_cover_path ?? null,
  });
}

/** Pull bytes for a cover the Phase-2 pipeline staged. Returns base64 + mime
 *  so the form can drop the bytes straight into a Blob URL and release the
 *  base64 string immediately — keeping the JS heap flat regardless of how
 *  many staged rows the user has open at once. */
export async function preparedCoverGet(
  path: string
): Promise<{ data: string; mime_type: string }> {
  return invoke('prepared_cover_get', { path });
}

/** Drop every staged cover. Called when the review dialog closes — the
 *  staged bytes have no consumer once the user walks away. */
export async function preparedCoverClear(): Promise<void> {
  return invoke('prepared_cover_clear');
}

export async function remoteConfigGet(): Promise<RemoteConfig> {
  return invoke('remote_config_get');
}

export interface RemoteLoginParams {
  app_key: string;
  access_token: string;
  expires_in_secs: number;
  app_root?: string | null;
}

export async function remoteLogin(params: RemoteLoginParams): Promise<RemoteConfig> {
  return invoke('remote_login', {
    appKey: params.app_key,
    accessToken: params.access_token,
    expiresInSecs: params.expires_in_secs,
    appRoot: params.app_root ?? null,
  });
}

export async function remoteGetAuthorizeUrl(appKey: string): Promise<string> {
  return invoke('remote_get_authorize_url', { appKey });
}

export async function remoteLogout(): Promise<void> {
  return invoke('remote_logout');
}

/** Push a batch of file IDs into the remote-upload worker queue. Returns
 *  immediately; subscribe via {@link onRemoteUploadProgress} for per-task
 *  state changes. The user can call this again while previous uploads are
 *  in flight — new IDs append to the queue. */
export async function enqueueRemoteUpload(fileIds: number[]): Promise<void> {
  return invoke('file_upload_to_remote', { fileIds });
}

export function onRemoteUploadProgress(
  callback: (progress: RemoteUploadProgress) => void
): Promise<UnlistenFn> {
  return listen<RemoteUploadProgress>('remote-upload-progress', (event) => callback(event.payload));
}

/** Push a batch of file IDs into the remote-download worker queue. Cloud
 *  copies stay in place; the worker writes each file to
 *  `<storage_path>/.cache/` and records the cache path on the row.
 *  Subscribe via {@link onRemoteDownloadProgress}. */
export async function enqueueRemoteDownload(fileIds: number[]): Promise<void> {
  return invoke('file_download_from_remote', { fileIds });
}

export function onRemoteDownloadProgress(
  callback: (progress: RemoteDownloadProgress) => void
): Promise<UnlistenFn> {
  return listen<RemoteDownloadProgress>('remote-download-progress', (event) =>
    callback(event.payload)
  );
}

/** Push a batch of file IDs into the bulk-delete worker queue. For remote
 *  files, the cloud blob is removed *strictly* (failure aborts the row's
 *  removal so the user can retry). Subscribe via {@link onRemoteDeleteProgress}. */
export async function enqueueRemoteDelete(fileIds: number[]): Promise<void> {
  return invoke('file_delete_via_worker', { fileIds });
}

export function onRemoteDeleteProgress(
  callback: (progress: RemoteDeleteProgress) => void
): Promise<UnlistenFn> {
  return listen<RemoteDeleteProgress>('remote-delete-progress', (event) =>
    callback(event.payload)
  );
}

export async function fileUpdate(
  id: number,
  params: { display_name?: string; category_id?: number | null; progress?: string | null }
): Promise<{ success: boolean }> {
  return invoke('file_update', {
    id,
    displayName: params.display_name,
    categoryId: params.category_id,
    progress: params.progress,
  });
}

export async function fileDelete(id: number): Promise<{ success: boolean }> {
  return invoke('file_delete', { id });
}

export async function fileDeleteSource(path: string): Promise<void> {
  return invoke('file_delete_source', { path });
}

export async function listFilesInFolder(path: string): Promise<string[]> {
  return invoke('list_files_in_folder', { path });
}

/** Group comic-schema files by author or by series-name prefix. Singletons
 *  are filtered out backend-side, so an empty result means there are no
 *  multi-member collections in scope — not that the call failed. */
export async function comicCollectionList(params: {
  mode: 'author' | 'name_prefix';
  category_id: number | null;
}): Promise<import('@/types').ComicCollection[]> {
  return invoke('comic_collection_list', {
    mode: params.mode,
    categoryId: params.category_id,
  });
}

/** Hydrate a set of files by id, with tags/authors/description joined. The
 *  drill-down from a comic collection card calls this so the FileList grid
 *  can resolve every file regardless of which paginated page it sits on in
 *  the main view. */
export async function fileListByIds(
  ids: number[]
): Promise<import('@/types').FileEntry[]> {
  if (ids.length === 0) return [];
  return invoke('file_list_by_ids', { ids });
}

/** Resolve an OS drag-drop path list: standalone files pass through;
 *  folders are walked the same way `listFilesInFolder` walks them. The
 *  returned `path_folder_roots` mirrors what `FilePicker` produces for
 *  folder picks, so the rest of the import flow stays uniform. */
export async function expandDropPaths(paths: string[]): Promise<{
  files: string[];
  path_folder_roots: Record<string, string>;
  empty_folders: string[];
}> {
  return invoke('expand_drop_paths', { paths });
}

/** Post-import cleanup for folder picks. Removes empty subdirectories
 *  under `folderRoot` and the root itself if empty. No-ops when
 *  `hadFolderImports` is false or when the user is in copy-mode. */
export async function importFinalize(
  folderRoot: string,
  hadFolderImports: boolean
): Promise<void> {
  return invoke('import_finalize', {
    folderRoot,
    hadFolderImports,
  });
}

export async function fileSearch(
  params: FileSearchRequest
): Promise<{ files: FileEntry[]; total: number }> {
  return invoke('file_search', {
    query: params.query,
    categoryId: params.category_id,
    tagIds: params.tag_ids,
    metadataFilters: params.metadata_filters,
    limit: params.limit,
    offset: params.offset,
  });
}

export async function fileCheckStatus(
  file_ids?: number[]
): Promise<{ updated: Array<{ id: number; status: string }> }> {
  return invoke('file_check_status', { fileIds: file_ids });
}

export async function categoryList(): Promise<Category[]> {
  return invoke('category_list');
}

export async function categoryGet(id: number): Promise<Category> {
  return invoke('category_get', { id });
}

export interface CategoryUpdateInput {
  id: number;
  name?: string;
  icon?: string;
  description?: string;
  schemaSlug?: import('@/types').SchemaSlug;
  /** Pass a JSON string to set, or omit to leave the existing value
   *  untouched. To wipe the column back to NULL (revert to schema
   *  defaults), set `clearViewConfig: true` instead. */
  viewConfig?: string;
  clearViewConfig?: boolean;
}

export async function categoryUpdate(
  input: CategoryUpdateInput
): Promise<{ success: boolean }> {
  return invoke('category_update', {
    id: input.id,
    name: input.name,
    icon: input.icon || null,
    description: input.description ?? null,
    schemaSlug: input.schemaSlug ?? null,
    viewConfig: input.viewConfig ?? null,
    clearViewConfig: input.clearViewConfig ?? null,
  });
}

export async function tagList(
  includeUsage?: boolean
): Promise<{ tags: (Tag & { usageCount: number })[] }> {
  return invoke('tag_list', { includeUsage: includeUsage || false });
}

export async function tagCreate(name: string, color?: string): Promise<{ id: number }> {
  return invoke('tag_create', { name, color: color || null });
}

export async function tagUpdate(
  id: number,
  name?: string,
  color?: string
): Promise<{ success: boolean }> {
  return invoke('tag_update', { id, name, color: color || null });
}

export async function tagDelete(id: number): Promise<{ success: boolean; affectedFiles: number }> {
  return invoke('tag_delete', { id });
}

export async function tagAssign(file_id: number, tag_ids: number[]): Promise<{ success: boolean }> {
  return invoke('tag_assign', { fileId: file_id, tagIds: tag_ids });
}

export async function tagUnassign(
  file_id: number,
  tag_ids: number[]
): Promise<{ success: boolean }> {
  return invoke('tag_unassign', { fileId: file_id, tagIds: tag_ids });
}

export async function fileListByTag(tagId: number): Promise<FileEntry[]> {
  return invoke('file_list_by_tag', { tagId });
}

export async function fileListByAuthor(authorId: number): Promise<FileEntry[]> {
  return invoke('file_list_by_author', { authorId });
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

export async function authorList(
  includeUsage?: boolean
): Promise<{ authors: (Author & { usageCount: number })[] }> {
  return invoke('author_list', { includeUsage: includeUsage || false });
}

export async function authorCreate(name: string): Promise<{ id: number }> {
  return invoke('author_create', { name });
}

export async function authorUpdate(id: number, name: string): Promise<{ success: boolean }> {
  return invoke('author_update', { id, name });
}

export async function authorDelete(
  id: number
): Promise<{ success: boolean; affectedFiles: number }> {
  return invoke('author_delete', { id });
}

export async function authorAssign(
  file_id: number,
  author_ids: number[]
): Promise<{ success: boolean }> {
  return invoke('author_assign', { fileId: file_id, authorIds: author_ids });
}

export async function authorUnassign(
  file_id: number,
  author_ids: number[]
): Promise<{ success: boolean }> {
  return invoke('author_unassign', { fileId: file_id, authorIds: author_ids });
}

export async function authorSet(
  file_id: number,
  author_ids: number[]
): Promise<{ success: boolean }> {
  return invoke('author_set', { fileId: file_id, authorIds: author_ids });
}

export async function coverSet(
  file_id: number,
  data: number[],
  mimeType?: string
): Promise<{ success: boolean }> {
  return invoke('cover_set', { fileId: file_id, data, mimeType: mimeType || null });
}

export async function coverGet(
  file_id: number
): Promise<{ data: string; mime_type: string }> {
  // The Rust struct serializes as snake_case (no `#[serde(rename_all)]`),
  // so the runtime shape is `{ data, mime_type }` — the previous TS type
  // claimed `mimeType` and silently produced `data:undefined;base64,...`
  // URLs whenever a caller used the wrong key.
  return invoke('cover_get', { fileId: file_id });
}

export async function coverDelete(file_id: number): Promise<{ success: boolean }> {
  return invoke('cover_delete', { fileId: file_id });
}

export interface RecompressCoversProgress {
  done: number;
  total: number;
  skipped: number;
}

export interface RecompressCoversResult {
  total: number;
  recompressed: number;
  skipped: number;
}

/** Walk every row in the `covers` table and re-encode it through the
 *  shared compression helper. Irreversible + lossy: existing covers
 *  lose detail. Returns a summary; subscribe via
 *  {@link onRecompressCoversProgress} for live progress while it runs. */
export async function recompressCovers(): Promise<RecompressCoversResult> {
  return invoke('db_recompress_covers');
}

export function onRecompressCoversProgress(
  callback: (progress: RecompressCoversProgress) => void
): Promise<UnlistenFn> {
  return listen<RecompressCoversProgress>('recompress-covers-progress', (event) =>
    callback(event.payload)
  );
}

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

export async function revealItemInDir(path: string): Promise<void> {
  const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
  return revealItemInDir(path);
}

export async function fileMoveCategory(
  file_id: number,
  new_category_id: number | null
): Promise<{ success: boolean }> {
  return invoke('file_move_category', { fileId: file_id, newCategoryId: new_category_id });
}

// Error code translations
const ERROR_MESSAGES: Record<string, string> = {
  STORAGE_PATH_NOT_CONFIGURED: 'Please configure a storage folder in settings first.',
  STORAGE_PATH_CHANGE_BLOCKED:
    'Cannot change storage path while files are stored. Remove all files first.',
  STORAGE_PATH_NOT_FOUND: 'The selected folder does not exist.',
  STORAGE_PATH_NOT_WRITABLE: 'Cannot write to the selected folder. Please choose another location.',
  STORAGE_PATH_SYSTEM_DIRECTORY: 'Cannot use a system directory. Please choose another location.',
  SOURCE_FILE_NOT_FOUND: 'The source file could not be found.',
  FILE_ALREADY_IN_STORAGE: 'This file is already in the managed storage.',
  FILE_NOT_IN_STORAGE: 'This file is not in managed storage.',
  FILE_NOT_FOUND: 'The file was not found.',
  CATEGORY_HAS_FILES: 'Cannot delete category with files. Move or delete files first.',
  CATEGORY_NOT_FOUND: 'The selected category was not found.',
  CATEGORY_FOLDER_NOT_SET: 'The category folder is not configured.',
  CANNOT_DELETE_DEFAULT: 'Cannot delete the default category.',
  PERMISSION_DENIED: 'Permission denied. Please check folder permissions.',
  FILE_LOCKED: 'File is in use by another application.',
  DISK_FULL: 'Not enough disk space to complete the operation.',
  NO_ACTIVE_PROMPT: 'No active prompt configured — set one in /prompts.',
  REMOTE_NOT_AUTHENTICATED: 'Baidu Pan not configured. Please sign in via Settings > Storage.',
  ACCESS_TOKEN_EXPIRED: 'Baidu Pan token expired. Re-authenticate in Settings > Storage.',
};

export function translateError(error: string): string {
  if (ERROR_MESSAGES[error]) return ERROR_MESSAGES[error];
  // Handle prefix-coded errors like "NO_ACTIVE_PROMPT: filename"
  const prefix = (error.split(':')[0] ?? '').trim();
  if (ERROR_MESSAGES[prefix]) return ERROR_MESSAGES[prefix];
  return error;
}

/** Push a batch of paths into the import worker queue. Returns immediately;
 *  subscribe via {@link listenProcessingProgress} and {@link listenFilePrepared}
 *  for per-file state changes. The user can call this again while previous
 *  analysis is in flight — new paths append to the queue. */
export async function enqueueImport(
  paths: string[],
  pathFolderRoots?: Record<string, string>
): Promise<void> {
  return invoke('enqueue_import', {
    paths,
    pathFolderRoots: pathFolderRoots && Object.keys(pathFolderRoots).length > 0
      ? pathFolderRoots
      : null,
  });
}

export async function cancelProcessing(): Promise<void> {
  return invoke('cancel_processing');
}

export function listenProcessingProgress(
  callback: (progress: ProcessingProgress) => void
): Promise<UnlistenFn> {
  return listen<ProcessingProgress>('processing-progress', (event) => {
    callback(event.payload);
  });
}

export function listenFilePrepared(
  callback: (prepared: FilePreparedImport) => void
): Promise<UnlistenFn> {
  return listen<FilePreparedImport>('file-prepared', (event) => {
    callback(event.payload);
  });
}

/**
 * Subscribe to tag/author mutation events. The callback runs when any of
 * `tag-deleted`, `tag-updated`, `author-deleted`, or `author-updated` fires.
 * The returned UnlistenFn detaches all four listeners.
 */
export function listenTagAuthorChanges(
  callback: () => void
): Promise<UnlistenFn> {
  const events = [
    'tag-deleted',
    'tag-updated',
    'author-deleted',
    'author-updated',
  ] as const;

  return Promise.all(
    events.map((name) => listen(name, () => callback()))
  ).then((unlisteners) => {
    const unlistenAll: UnlistenFn = () => {
      for (const u of unlisteners) u();
    };
    return unlistenAll;
  });
}

export async function llmConfigGet(): Promise<LlmConfig> {
  return invoke('llm_config_get');
}

export async function llmConfigSet(config: LlmConfig): Promise<void> {
  return invoke('llm_config_set', { config });
}

export async function llmTestConnection(): Promise<string> {
  return invoke('llm_test_connection');
}

export async function promptList(filter?: {
  schema_slug?: import('@/types').SchemaSlug;
  step?: string;
}): Promise<Prompt[]> {
  return invoke('prompt_list', {
    schemaSlug: filter?.schema_slug ?? null,
    step: filter?.step ?? null,
  });
}

export async function promptCreate(payload: PromptCreatePayload): Promise<Prompt> {
  return invoke('prompt_create', { payload });
}

export async function promptUpdate(id: number, payload: PromptCreatePayload): Promise<Prompt> {
  return invoke('prompt_update', { id, payload });
}

export async function promptDelete(id: number): Promise<void> {
  return invoke('prompt_delete', { id });
}

export async function promptSetDefault(id: number): Promise<Prompt> {
  return invoke('prompt_set_default', { id });
}
