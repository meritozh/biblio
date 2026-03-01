export type FileStatus = 'available' | 'missing' | 'moved';
export type MetadataType = 'text' | 'number' | 'date' | 'boolean';

export interface Category {
  id: number;
  name: string;
  icon: string | null;
  isDefault: boolean;
  createdAt: string;
}

export interface FileEntry {
  id: number;
  path: string;
  displayName: string;
  categoryId: number | null;
  fileStatus: FileStatus;
  createdAt: string;
  updatedAt: string;
  category?: Category | null;
  tags?: Tag[];
  metadata?: Metadata[];
}

export interface Tag {
  id: number;
  name: string;
  color: string | null;
  createdAt: string;
}

export interface Metadata {
  id: number;
  fileId: number;
  key: string;
  value: string;
  dataType: MetadataType;
}

export interface FileWithDetails {
  id: number;
  path: string;
  displayName: string;
  categoryId: number | null;
  fileStatus: FileStatus;
  createdAt: string;
  updatedAt: string;
  category: Category | null;
  tags: Tag[];
  metadata: Metadata[];
}

export interface FileListRequest extends Record<string, unknown> {
  categoryId?: number | null;
  tagIds?: number[];
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
  displayName: string;
  categoryId?: number | null;
  tagIds?: number[];
  metadata?: Array<{ key: string; value: string; dataType: MetadataType }>;
}

export interface FileSearchRequest extends Record<string, unknown> {
  query: string;
  categoryId?: number | null;
  tagIds?: number[];
  metadataFilters?: Array<{ key: string; value: string }>;
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
