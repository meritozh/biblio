import { useCallback, useEffect } from 'react';
import {
  appendToView,
  setView,
  setViewLoading,
  useView as useViewState,
  useRefreshEpoch,
} from '@/stores/fileStore';
import type { FileEntry } from '@/types';

export interface ViewFetcherResult {
  files: FileEntry[];
  total: number;
}

/**
 * Owns route-level fetching for a `<FileList>` view. The route passes a
 * stable `key` and a `fetcher`; the hook fetches on mount, on key change,
 * and on every `refreshActiveView()` bump (e.g. tag rename), writing the
 * result into the normalized `fileStore`.
 *
 * The returned view state is read from the store, so single-row patches
 * (`patchFile`, `removeFile`) flow through to the UI without re-fetching.
 */
export function useView(
  key: string,
  fetcher: () => Promise<ViewFetcherResult>
): {
  ids: number[];
  total: number;
  loading: boolean;
  reload: () => Promise<void>;
  appendMore: (more: ViewFetcherResult) => void;
} {
  const view = useViewState(key);
  const epoch = useRefreshEpoch();

  const reload = useCallback(async () => {
    setViewLoading(key, true);
    try {
      const result = await fetcher();
      setView(key, result.files, result.total);
    } catch (error) {
      console.error('useView fetch failed:', error);
      setView(key, [], 0);
    }
  }, [key, fetcher]);

  useEffect(() => {
    void reload();
  }, [reload, epoch]);

  const appendMore = useCallback(
    (more: ViewFetcherResult) => appendToView(key, more.files, more.total),
    [key]
  );

  return {
    ids: view.ids,
    total: view.total,
    loading: view.loading,
    reload,
    appendMore,
  };
}
