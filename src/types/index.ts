export type FileStatus = 'available' | 'missing' | 'moved';
export type MetadataType = 'text' | 'number' | 'date' | 'boolean';

/** Built-in schema slug. Each value picks a `CategorySchema` from
 *  `lib/categorySchema.ts` that decides which form sections, card
 *  fields, and prompts apply to files in this category. */
export type SchemaSlug = 'novel' | 'comic' | 'galgame';

export interface Category {
  id: number;
  name: string;
  description: string | null;
  icon: string | null;
  is_default: boolean;
  folder_name: string | null;
  /** Drives form layout, card layout, and prompt resolution. Defaults
   *  to `'novel'` for legacy rows; the migration v7 backfills 'comic'
   *  for the seeded comic category. */
  schema_slug: SchemaSlug;
  /** JSON-serialized `CategoryViewConfig` (see `lib/categorySchema.ts`).
   *  `null` means the file list should use the schema-slug defaults from
   *  the frontend REGISTRY. The shape is owned by the frontend; the
   *  backend only validates that this is parseable JSON. */
  view_config: string | null;
  created_at: string;
}

export interface FileEntry {
  id: number;
  path: string;
  display_name: string;
  category_id: number | null;
  file_status: FileStatus;
  in_storage: boolean;
  original_path: string | null;
  progress?: string | null;
  created_at: string;
  updated_at: string;
  category?: Category | null;
  tags?: Tag[];
  authors?: Author[];
  metadata?: Metadata[];
  storage_kind?: StorageKind;
  remote_provider?: string | null;
  /** Set when a remote file has been pulled to the local cache directory.
   *  Drives the "cached locally" badge and the cleanup path on delete. */
  local_cache_path?: string | null;
  is_favorite: boolean;
}

export interface Tag {
  id: number;
  name: string;
  color: string | null;
  created_at: string;
}

export interface TagWithUsage extends Tag {
  usageCount: number;
}

export interface Author {
  id: number;
  name: string;
  created_at: string;
}

export interface AuthorWithUsage extends Author {
  usageCount: number;
}

export interface Metadata {
  id: number;
  file_id: number;
  key: string;
  value: string;
  data_type: MetadataType;
}

export interface FileWithDetails {
  id: number;
  path: string;
  display_name: string;
  category_id: number | null;
  file_status: FileStatus;
  in_storage: boolean;
  original_path: string | null;
  progress?: string | null;
  storage_kind?: StorageKind;
  remote_provider?: string | null;
  local_cache_path?: string | null;
  is_favorite: boolean;
  created_at: string;
  updated_at: string;
  category: Category | null;
  tags: Tag[];
  authors: Author[];
  metadata: Metadata[];
}

export interface FileListRequest extends Record<string, unknown> {
  category_id?: number | null;
  tag_ids?: number[];
  status?: FileStatus;
  limit?: number;
  offset?: number;
}

/** A group of files whose `display_name`s share a long prefix. Returned
 *  by `file_duplicate_groups`. `prefix` is the case-original LCP across
 *  the whole group, used as the card label on /cleanup. */
export interface DuplicateGroup {
  prefix: string;
  files: FileEntry[];
}

export interface FileListResponse {
  files: FileEntry[];
  total: number;
}

export type StorageKind = 'local' | 'remote';

/** Grouping axis surfaced by the FileList view-mode toggle. `'flat'`
 *  keeps the existing per-file grid; `'author'` and `'name_prefix'`
 *  collapse the grid into collection cards (one card per author or per
 *  derived series name). Available for both novel and comic schemas. */
export type ViewMode = 'flat' | 'author' | 'name_prefix';

/** One collection card. `mode` matches the request that produced it;
 *  `key` is unique within the mode (stringified author id, or the
 *  series-name root). `cover_file_id` is the member rendered as the
 *  card's preview cover. `schema_slug` tells the card which renderer
 *  to use — comic → stored cover bytes via `coverGet`; novel → procedural
 *  NovelCover from the cover file's tags + display_name. */
export interface Collection {
  mode: 'author' | 'name_prefix';
  key: string;
  title: string;
  file_ids: number[];
  cover_file_id: number | null;
  schema_slug: SchemaSlug;
}

export interface FileCreateRequest extends Record<string, unknown> {
  path: string;
  display_name: string;
  category_id?: number | null;
  tag_ids?: number[];
  author_ids?: number[];
  metadata?: Array<{ key: string; value: string; data_type: MetadataType }>;
  progress?: string;
  is_favorite?: boolean;
  cover_data?: string;
  cover_mime_type?: string;
  /** Token (the original import path) for a cover staged by the Phase-2
   *  pipeline. The backend pulls bytes from its in-memory cache and writes
   *  the cover row server-side, so the base64 never crosses IPC. Inline
   *  `cover_data` wins when both are present (user uploaded a replacement). */
  staged_cover_path?: string;
}

export interface FileSearchRequest extends Record<string, unknown> {
  query: string;
  category_id?: number | null;
  tag_ids?: number[];
  metadata_filters?: Array<{ key: string; value: string }>;
  limit?: number;
  offset?: number;
}

export interface CategoryCreateRequest extends Record<string, unknown> {
  name: string;
  icon?: string | null;
}

export interface TagCreateRequest extends Record<string, unknown> {
  name: string;
  color?: string | null;
}

export interface AuthorCreateRequest extends Record<string, unknown> {
  name: string;
}

// Dynamic form field types
export type FieldType =
  | 'text'
  | 'authors'
  | 'number'
  | 'date'
  | 'boolean'
  | 'select'
  | 'tags'
  | 'image';

export interface FieldConfig {
  key: string;
  label: string;
  type: FieldType;
  required?: boolean;
  placeholder?: string;
  options?: string[];
  defaultValue?: string;
}

export interface ExtractedField {
  key: string;
  value: string;
  data_type: string;
}

export type DuplicateAction = 'Replace' | 'Delete' | 'ImportAnyway';

export interface DuplicateInfo {
  existing_file_id: number;
  existing_display_name: string;
  existing_progress: string | null;
  /** Byte length of the existing file on disk. `null` when the file is
   *  missing / unreadable / uncached remote — the comparison panel
   *  renders that as "—". */
  existing_size: number | null;
  /** Byte length of the file being imported. `null` for folder-to-zip
   *  imports (no single file to measure) or unreadable paths. */
  new_size: number | null;
  /** Author names attached to the existing row. Empty array when none.
   *  Denormalized into this DTO so the dupe compare panel can render
   *  side-by-side authors without an extra IPC round-trip. */
  existing_author_names: string[];
  recommendation: DuplicateAction;
}

export interface FilePreparedImport {
  path: string;
  file_name: string;
  display_name: string;
  category_id: number | null;
  tag_ids: number[];
  author_ids: number[];
  metadata: ExtractedField[];
  unresolved_author_names: string[];
  /** Present when the Phase-2 pipeline staged a cover for this import. The
   *  bytes themselves live in the Rust-side `PreparedCoverCache`; fetch
   *  them on demand with `preparedCoverGet(path)`. */
  cover_mime_type?: string;
  progress: string | null;
  suggested_tags: string[];
  duplicate_of: DuplicateInfo | null;
  batch_duplicate_group: string | null;
  /** True when the picked source is a folder of images. The import flow
   *  will package it into a `.zip` on commit; the review UI shows a
   *  "Folder → .zip" hint so the user knows what's about to land. */
  source_is_directory: boolean;
}

export interface ProcessingProgress {
  current: number;
  total: number;
  current_file: string;
  status: string;
}

export type FileAnalysisStatus =
  | 'pending'
  | 'extracting_name'
  | 'analyzing_content'
  | 'ready'
  | 'partial'
  | 'error';

export interface LlmConfig {
  enabled: boolean;
  base_url: string;
  api_key: string;
  model: string;
  analyze_content: boolean;
}

export interface RemoteConfig {
  enabled: boolean;
  app_key: string;
  access_token: string;
  access_token_expires_at: number;
  app_root: string;
}

export interface RemoteUploadProgress {
  file_id: number;
  file_name: string;
  /** `pending` is the queued-but-not-yet-running state in the producer-consumer
   *  worker. The backend emits `uploading` once the worker picks the job up. */
  status: 'pending' | 'uploading' | 'success' | 'error' | 'skipped';
  error?: string;
  /** Bytes done so far / total for this file. Present on progress ticks; the
   *  panel derives percent + speed from them. Omitted on status-only events,
   *  so the store preserves the last known values. */
  uploaded_bytes?: number;
  total_bytes?: number;
  /** Which long phase the byte counts belong to. The `status` stays
   *  `uploading` for all three (same spinner); this drives the row label so
   *  the user sees the multi-minute encrypt/hash phases instead of a blank
   *  "Uploading…". Omitted on status-only events. */
  phase?: 'encrypting' | 'hashing' | 'uploading';
}

/** Mirrors the upload progress shape; emitted by the download_worker as it
 *  drains its queue. `pending` is the optimistic local-side state inserted
 *  by `enqueueDownload`; the backend emits `downloading` → `success` | `error`. */
export interface RemoteDownloadProgress {
  file_id: number;
  file_name: string;
  status: 'pending' | 'downloading' | 'success' | 'error';
  error?: string;
  /** Absolute path of the freshly-written cache file. Carried on the
   *  terminal `success` event so the file-store can be patched with the
   *  real path (not a sentinel) — "Show in Finder" calls
   *  `revealItemInDir(local_cache_path)` and needs a real fs path. */
  local_cache_path?: string | null;
}

/** Bulk-delete progress, emitted by the delete_worker. Single-row deletes
 *  go through the existing `file_delete` and don't fire these events. */
export interface RemoteDeleteProgress {
  file_id: number;
  file_name: string;
  status: 'pending' | 'deleting' | 'success' | 'error';
  error?: string;
}

/** Re-encrypt progress, emitted by the reencrypt_worker as it back-fills
 *  legacy raw remote files into the encrypted container. The two-stage
 *  pipeline emits `downloading` (download leg) then `uploading` (encrypt +
 *  upload leg) before a terminal `success` | `error`. `skipped` is the
 *  terminal state for a queued row that turned ineligible before processing
 *  (already encrypted / no longer remote) — counted as done by the UI. */
export interface RemoteReencryptProgress {
  file_id: number;
  file_name: string;
  status: 'downloading' | 'uploading' | 'success' | 'error' | 'skipped';
  error?: string;
}

/** @deprecated kept for migration / one-release back-compat. New code
 *  must read `schema_slug` from prompts and categories instead. */
export type PromptMimeGroup = 'text' | 'archive' | 'image_folder';

/** Pipeline step a prompt feeds.
 *  - novel: `filename`, `content`
 *  - comic: `filename` (archive source), `cover_pick`, `filename_folder`
 *    (image-folder source — different rules: folder name doesn't carry
 *    the author, the parent folder does). The `(schema_slug, step)`
 *    pair is the unique discriminator. */
export type PromptStep =
  | 'filename'
  | 'content'
  | 'cover_pick'
  | 'filename_folder';

/** Legacy free-text label kept on rows for back-compat readers. New code
 *  should switch on `schema_slug` + `step` instead. */
export type PromptCategory = string;

export interface Prompt {
  id: number;
  name: string;
  content: string;
  /** Legacy label from before v3 — kept for back-compat. */
  category: PromptCategory | null;
  /** @deprecated read `schema_slug` instead. */
  mime_group: PromptMimeGroup;
  schema_slug: SchemaSlug | null;
  step: PromptStep;
  is_default: boolean;
  created_at: string;
  updated_at: string;
}

export interface PromptCreatePayload {
  name: string;
  content: string;
  schema_slug: SchemaSlug;
  step: PromptStep;
}
