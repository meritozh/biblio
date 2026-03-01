import { Store } from '@tanstack/react-store';
import type { Category } from '@/types';
import { invoke } from '@tauri-apps/api/core';

interface CategoryState {
  categories: Category[];
  loading: boolean;
  error: string | null;
}

const initialState: CategoryState = {
  categories: [],
  loading: false,
  error: null,
};

export const categoryStore = new Store<CategoryState>(initialState);

export async function fetchCategories() {
  categoryStore.setState((state) => ({ ...state, loading: true, error: null }));
  try {
    const categories = await invoke<Category[]>('category_list');
    categoryStore.setState((state) => ({ ...state, categories, loading: false }));
  } catch (error) {
    categoryStore.setState((state) => ({ ...state, error: String(error), loading: false }));
  }
}
