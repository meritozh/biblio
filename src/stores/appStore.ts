import { Store, useStore } from '@tanstack/react-store';
import type { Category } from '@/types';
import { fetchCategories as invokeFetchCategories } from '@/stores';

interface AppState {
  categories: Category[];
  selectedCategoryId: number | null;
  settingsOpen: boolean;
}

const initialState: AppState = {
  categories: [],
  selectedCategoryId: null,
  settingsOpen: false,
};

export const appStore = new Store<AppState>(initialState);

// Reload the category list and snap the active selection to a sane value:
//   - empty list  → null
//   - prior pick  → keep it if still present, otherwise fall back to the first
export async function loadCategories(): Promise<void> {
  const categories = await invokeFetchCategories();
  appStore.setState((state) => {
    let selected: number | null = state.selectedCategoryId;
    if (categories.length === 0) {
      selected = null;
    } else if (!categories.some((c) => c.id === selected)) {
      selected = categories[0]!.id;
    }
    return { ...state, categories, selectedCategoryId: selected };
  });
}

export function setSelectedCategoryId(id: number | null): void {
  appStore.setState((s) => ({ ...s, selectedCategoryId: id }));
}

export function setSettingsOpen(open: boolean): void {
  appStore.setState((s) => ({ ...s, settingsOpen: open }));
}

export function useAppState<T>(selector: (s: AppState) => T): T {
  return useStore(appStore, selector);
}
