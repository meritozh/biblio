import type { Category, Tag, Metadata, FileStatus } from './index';

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
