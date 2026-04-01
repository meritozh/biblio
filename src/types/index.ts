export type FileStatus = 'available' | 'missing' | 'moved';
export type MetadataType = 'text' | 'number' | 'date' | 'boolean';

export interface Category {
  id: number;
  name: string;
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

export interface FileCreateRequest extends Record<string, unknown> {
  path: string;
  display_name: string;
  category_id?: number | null;
  tag_ids?: number[];
  author_ids?: number[];
  metadata?: Array<{ key: string; value: string; data_type: MetadataType }>;
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

export interface FilePreparedImport {
  path: string;
  file_name: string;
  display_name: string;
  category_id: number | null;
  tag_ids: number[];
  author_ids: number[];
  metadata: ExtractedField[];
  unresolved_author_names: string[];
}

export interface ProcessingProgress {
  current: number;
  total: number;
  current_file: string;
  status: string;
}
