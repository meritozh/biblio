export type FileStatus = 'available' | 'missing' | 'moved';
export type MetadataType = 'text' | 'number' | 'date' | 'boolean';

export interface Category {
  id: number;
  name: string;
  description: string | null;
  icon: string | null;
  is_default: boolean;
  folder_name: string | null;
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
  description?: string | null;
  created_at: string;
  updated_at: string;
  category?: Category | null;
  tags?: Tag[];
  authors?: Author[];
  metadata?: Metadata[];
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

export interface FileListResponse {
  files: FileEntry[];
  total: number;
}

export type StorageKind = 'local' | 'remote';

export interface FileCreateRequest extends Record<string, unknown> {
  path: string;
  display_name: string;
  category_id?: number | null;
  tag_ids?: number[];
  author_ids?: number[];
  metadata?: Array<{ key: string; value: string; data_type: MetadataType }>;
  progress?: string;
  cover_data?: string;
  cover_mime_type?: string;
  storage_kind?: StorageKind;
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
  cover_data?: string;
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

export type RemoteAuthMode = 'openlist_proxy' | 'self_app';

export interface RemoteConfig {
  enabled: boolean;
  auth_mode: RemoteAuthMode;
  refresh_token: string;
  client_id: string | null;
  client_secret: string | null;
  access_token: string;
  access_token_expires_at: number;
  app_root: string;
}

/** Mime-type group a prompt applies to. */
export type PromptMimeGroup = 'text' | 'archive' | 'image_folder';

/** Step within a mime group. text supports filename + content; archive supports
 *  filename + cover_pick; image_folder supports filename only (cover detection
 *  reuses the archive prompt). The (group, step) pair is the unique discriminator. */
export type PromptStep = 'filename' | 'content' | 'cover_pick';

/** Legacy free-text label kept on rows for back-compat readers. New code
 *  should switch on `mime_group` + `step` instead. */
export type PromptCategory = string;

export interface Prompt {
  id: number;
  name: string;
  content: string;
  /** Legacy label from before v3 — kept for back-compat. */
  category: PromptCategory | null;
  mime_group: PromptMimeGroup;
  step: PromptStep;
  is_default: boolean;
  created_at: string;
  updated_at: string;
}

export interface PromptCreatePayload {
  name: string;
  content: string;
  mime_group: PromptMimeGroup;
  step: PromptStep;
}
