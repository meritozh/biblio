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

/** Group files by author or by series-name prefix within one schema
 *  family. `schemaSlug` filters to that family — 'novel' covers every
 *  novel-schema category, 'comic' covers every comic-schema category
 *  (the schema-slug column does the routing). Singletons are filtered
 *  out backend-side, so an empty result means there are no multi-member
 *  collections in scope — not that the call failed. */
export async function collectionList(params: {
  mode: 'author' | 'name_prefix';
  schemaSlug: import('@/types').SchemaSlug;
  category_id: number | null;
}): Promise<import('@/types').Collection[]> {
  return invoke('collection_list', {
    mode: params.mode,
    schemaSlug: params.schemaSlug,
    categoryId: params.category_id,
  });
}

/** Result of `file_reanalyze_missing_tags`. Per-file failures live in
 *  `errors` so the UI can surface them inline; the run keeps going past
 *  individual file failures. */
export interface ReanalyzeError {
  file_id: number;
  display_name: string;
  message: string;
}

export interface ReanalyzeResponse {
  processed: number;
  succeeded: number;
  failed: number;
  errors: ReanalyzeError[];
}

/** Count novels with zero tags. Cheap query — feeds the "N files
 *  affected" badge on the /cleanup Debug card without committing to
 *  the LLM run. */
export async function fileCountNovelsMissingTags(): Promise<number> {
  return invoke('file_count_novels_missing_tags');
}

/** Re-run the import-time LLM content extraction against every novel
 *  file that currently has zero tags assigned. Applies returned tags
 *  (creating unknown ones) and the new category (when it differs from
 *  the file's current). Blocking IPC — at a few seconds per file × N
 *  files this can take minutes; the caller should disable its trigger
 *  and show a spinner. */
export async function fileReanalyzeMissingTags(): Promise<ReanalyzeResponse> {
  return invoke('file_reanalyze_missing_tags');
}

/** Result of `file_reanalyze_for_category` — same error-collection shape
 *  as ReanalyzeResponse, but the success vs. no-op split is explicit
 *  (moved vs. skipped) because the LLM frequently picks a non-target
 *  category and we don't want that to look like a failure. */
export interface ReclassifyResponse {
  processed: number;
  moved: number;
  skipped: number;
  failed: number;
  errors: ReanalyzeError[];
}

/** Count novel-schema files that could be re-classified into the target
 *  category — i.e. not already in it. `sourceCategoryId` scopes the
 *  candidate set to one source category when set; `null` means "every
 *  novel-schema category except the target". */
export async function fileCountForCategoryReanalyze(
  targetCategoryId: number,
  sourceCategoryId: number | null
): Promise<number> {
  return invoke('file_count_for_category_reanalyze', {
    targetCategoryId,
    sourceCategoryId,
  });
}

/** Run the import-time content LLM on each candidate. Only moves a file
 *  into `targetCategoryId` when the LLM's category pick matches the
 *  target's name (NFC + case-insensitive). Tags returned by the LLM are
 *  ignored on this path — use `fileReanalyzeMissingTags` for tag fills. */
export async function fileReanalyzeForCategory(
  targetCategoryId: number,
  sourceCategoryId: number | null
): Promise<ReclassifyResponse> {
  return invoke('file_reanalyze_for_category', {
    targetCategoryId,
    sourceCategoryId,
  });
}

/** Count files with no `file_authors` row, optionally scoped to one
 *  category. Pass `null` to count library-wide. Feeds the affected-count
 *  badge on /cleanup's "Assign author" Debug card. */
export async function fileCountAuthorlessInCategory(
  categoryId: number | null
): Promise<number> {
  return invoke('file_count_authorless_in_category', { categoryId });
}

/** Result of `file_regenerate_missing_covers`. `regenerated` is the
 *  success count; `skipped` covers the recoverable cases (file missing
 *  on disk, remote without local cache) that the user can address and
 *  retry; `failed` covers the structural cases (archive unreadable,
 *  decode failure) that a re-run won't fix. */
export interface RegenerateCoversResponse {
  processed: number;
  regenerated: number;
  skipped: number;
  failed: number;
  errors: ReanalyzeError[];
}

/** Count comic-schema files with no row in the `covers` table —
 *  victims of the pre-2936908 cancel/clear race or genuine new
 *  imports that haven't been analyzed. Feeds the count badge on
 *  /cleanup's "Regenerate missing comic covers" card. */
export async function fileCountComicsMissingCovers(): Promise<number> {
  return invoke('file_count_comics_missing_covers');
}

/** Re-extract the first cover image (basename heuristic — no LLM) from
 *  every comic archive whose `covers` row is missing, compress, and
 *  store. Idempotent — running twice just replaces the freshly-stored
 *  bytes with another extraction of the same archive. */
export async function fileRegenerateMissingCovers(): Promise<RegenerateCoversResponse> {
  return invoke('file_regenerate_missing_covers');
}

/** Bulk-assign `authorId` to every file with no current author, optionally
 *  scoped to one category. Single-transaction `INSERT OR IGNORE`. Emits
 *  one bulk `author-updated` event (sentinel id 0). Returns the number
 *  of join rows actually inserted. */
export async function fileAssignAuthorToAuthorless(
  categoryId: number | null,
  authorId: number
): Promise<{ assigned: number }> {
  return invoke('file_assign_author_to_authorless', { categoryId, authorId });
}

/** Find groups of files whose `display_name`s share a long prefix. Used
 *  by /cleanup. Returns groups of size ≥ 2 with hydrated rows so the
 *  card can render storage badges without a follow-up IPC. Defaults
 *  match the backend's: 3 chars min, 0.5 ratio. `categoryId` scopes
 *  the scan to one category; null/undefined searches everything. */
export async function fileDuplicateGroups(params?: {
  minPrefixChars?: number;
  prefixRatio?: number;
  categoryId?: number | null;
}): Promise<import('@/types').DuplicateGroup[]> {
  return invoke('file_duplicate_groups', {
    minPrefixChars: params?.minPrefixChars ?? null,
    prefixRatio: params?.prefixRatio ?? null,
    categoryId: params?.categoryId ?? null,
  });
}

/** Hydrate a set of files by id, with tags/authors joined. The drill-down
 *  from a comic collection card calls this so the FileList grid can resolve
 *  every file regardless of which paginated page it sits on in the main
 *  view. */
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

/** Result of `category_merge`. `skipped_duplicates` lists the basenames
 *  the merge refused to overwrite (same filename already at target). When
 *  `deleted_source` is false, the source category was kept so the user
 *  can resolve those conflicts and re-run the merge. */
export interface CategoryMergeResponse {
  moved: number;
  skipped_duplicates: string[];
  deleted_source: boolean;
}

/** Merge every file from `sourceId` into `targetId`, on disk and in the
 *  DB. Same-schema only — the command rejects INCOMPATIBLE_SCHEMAS. The
 *  source category row + folder are deleted iff every file moved cleanly
 *  (no duplicates skipped). Irreversible — call from a confirm dialog. */
export async function categoryMerge(
  sourceId: number,
  targetId: number
): Promise<CategoryMergeResponse> {
  return invoke('category_merge', { sourceId, targetId });
}

export async function tagList(params?: {
  includeUsage?: boolean;
  limit?: number;
  offset?: number;
  /** Case-insensitive substring match against `tags.name`. Trimmed
   *  empty strings are treated as "no filter" both here and in Rust. */
  nameQuery?: string;
}): Promise<{ tags: (Tag & { usageCount: number })[] }> {
  return invoke('tag_list', {
    includeUsage: params?.includeUsage || false,
    limit: params?.limit ?? null,
    offset: params?.offset ?? null,
    nameQuery: params?.nameQuery ?? null,
  });
}

export async function tagCount(params?: { nameQuery?: string }): Promise<number> {
  return invoke('tag_count', { nameQuery: params?.nameQuery ?? null });
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

export async function tagDelete(
  id: number
): Promise<{ success: boolean; affected_files: number }> {
  return invoke('tag_delete', { id });
}

/** Bulk-delete every tag with no `file_tags` row referencing it. Emits a
 *  single `tag-deleted` event (sentinel id `0`) at the end so the existing
 *  `listenTagAuthorChanges` listener re-fetches once, not N times. */
export async function tagDeleteUnused(): Promise<{ deleted: number }> {
  return invoke('tag_delete_unused');
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

export async function authorList(params?: {
  includeUsage?: boolean;
  limit?: number;
  offset?: number;
  /** Case-insensitive substring match against `authors.name`. */
  nameQuery?: string;
}): Promise<{ authors: (Author & { usageCount: number })[] }> {
  return invoke('author_list', {
    includeUsage: params?.includeUsage || false,
    limit: params?.limit ?? null,
    offset: params?.offset ?? null,
    nameQuery: params?.nameQuery ?? null,
  });
}

export async function authorCount(params?: { nameQuery?: string }): Promise<number> {
  return invoke('author_count', { nameQuery: params?.nameQuery ?? null });
}

export async function authorCreate(name: string): Promise<{ id: number }> {
  return invoke('author_create', { name });
}

export async function authorUpdate(id: number, name: string): Promise<{ success: boolean }> {
  return invoke('author_update', { id, name });
}

export async function authorDelete(
  id: number
): Promise<{ success: boolean; affected_files: number }> {
  return invoke('author_delete', { id });
}

/** Bulk-delete every author with no `file_authors` row referencing it. See
 *  `tagDeleteUnused` for the single-event rationale. */
export async function authorDeleteUnused(): Promise<{ deleted: number }> {
  return invoke('author_delete_unused');
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

/** Open a file's on-disk copy with the system default app. Resolves the
 *  cache path for remote rows and the regular `path` for local rows
 *  inside one backend call so callers don't need to branch on
 *  `storage_kind`. Errors `CACHE_NOT_FOUND` for remote rows without a
 *  cached copy. */
export async function cacheOpen(file_id: number): Promise<{ success: boolean }> {
  return invoke('cache_open', { fileId: file_id });
}

/** Delete the local cache copy for a remote file. Strict: disk delete
 *  first, DB column cleared second. `NotFound` on disk is treated as
 *  success. Idempotent on already-cleared rows. */
export async function cacheClear(file_id: number): Promise<{ success: boolean }> {
  return invoke('cache_clear', { fileId: file_id });
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
