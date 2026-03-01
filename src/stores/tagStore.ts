import { Store, useStore } from '@tanstack/react-store';
import type { Tag } from '@/types';
import { invoke } from '@tauri-apps/api/core';

interface TagState {
  tags: Tag[];
  loading: boolean;
  error: string | null;
}

const initialState: TagState = {
  tags: [],
  loading: false,
  error: null,
};

export const tagStore = new Store<TagState>(initialState);

export function useTagStore() {
  return useStore(tagStore, (state) => state);
}

export async function fetchTags() {
  tagStore.setState((state) => ({ ...state, loading: true, error: null }));
  try {
    const response = await invoke<{ tags: (Tag & { usageCount: number })[] }>('tag_list', {
      includeUsage: true,
    });
    tagStore.setState((state) => ({ ...state, tags: response.tags, loading: false }));
  } catch (error) {
    tagStore.setState((state) => ({ ...state, error: String(error), loading: false }));
  }
}

export async function createTag(name: string, color?: string) {
  try {
    await invoke<{ id: number }>('tag_create', { name, color: color ?? null });
    await fetchTags();
  } catch (error) {
    tagStore.setState((state) => ({ ...state, error: String(error) }));
    throw error;
  }
}

export async function deleteTag(id: number) {
  try {
    await invoke('tag_delete', { id });
    await fetchTags();
  } catch (error) {
    tagStore.setState((state) => ({ ...state, error: String(error) }));
    throw error;
  }
}

export async function assignTags(fileId: number, tagIds: number[]) {
  try {
    await invoke('tag_assign', { fileId, tagIds });
  } catch (error) {
    tagStore.setState((state) => ({ ...state, error: String(error) }));
    throw error;
  }
}
