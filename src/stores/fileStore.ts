import { Store } from '@tanstack/react-store';
import type { FileEntry } from '@/types';
import { invoke } from '@tauri-apps/api/core';

interface ImportProgress {
  current: number;
  total: number;
  currentFile: string;
  isImporting: boolean;
}

interface FileState {
  files: FileEntry[];
  total: number;
  loading: boolean;
  error: string | null;
  selectedCategoryId: number | null;
  importProgress: ImportProgress;
}

const initialState: FileState = {
  files: [],
  total: 0,
  loading: false,
  error: null,
  selectedCategoryId: null,
  importProgress: {
    current: 0,
    total: 0,
    currentFile: '',
    isImporting: false,
  },
};

export const fileStore = new Store<FileState>(initialState);

export async function fetchFiles(params?: { categoryId?: number | null }) {
  fileStore.setState((state) => ({ ...state, loading: true, error: null }));
  try {
    const response = await invoke<{ files: FileEntry[]; total: number }>('file_list', params ?? {});
    fileStore.setState((state) => ({ ...state, ...response, loading: false }));
  } catch (error) {
    fileStore.setState((state) => ({ ...state, error: String(error), loading: false }));
  }
}

export function setSelectedCategory(categoryId: number | null) {
  fileStore.setState((state) => ({ ...state, selectedCategoryId: categoryId }));
}

export async function importFiles(
  paths: string[],
  options?: {
    categoryId?: number | null;
    onProgress?: (current: number, total: number, file: string) => void;
  }
): Promise<{ succeeded: number; failed: number; errors: string[] }> {
  const total = paths.length;
  let succeeded = 0;
  let failed = 0;
  const errors: string[] = [];

  fileStore.setState((state) => ({
    ...state,
    importProgress: { current: 0, total, currentFile: '', isImporting: true },
  }));

  for (let i = 0; i < paths.length; i++) {
    const path = paths[i]!;
    const fileName = path.split(/[/\\]/).pop() || path;

    fileStore.setState((state) => ({
      ...state,
      importProgress: { ...state.importProgress, current: i, currentFile: fileName },
    }));

    options?.onProgress?.(i, total, fileName);

    try {
      await invoke('file_create', {
        path,
        displayName: fileName,
        categoryId: options?.categoryId ?? null,
      });
      succeeded++;
    } catch (error) {
      failed++;
      errors.push(`${fileName}: ${String(error)}`);
    }
  }

  fileStore.setState((state) => ({
    ...state,
    importProgress: { current: total, total, currentFile: '', isImporting: false },
  }));

  await fetchFiles({ categoryId: fileStore.state.selectedCategoryId });

  return { succeeded, failed, errors };
}

export function cancelImport() {
  fileStore.setState((state) => ({
    ...state,
    importProgress: { current: 0, total: 0, currentFile: '', isImporting: false },
  }));
}
