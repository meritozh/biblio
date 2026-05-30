import { useCallback, useEffect, useRef } from 'react';
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

  // Per-key sequence counters so out-of-order reloads of the same key can't
  // clobber a newer in-flight result. Each call claims a token and only writes
  // when it is still the latest one for its key.
  const seqRef = useRef<Map<string, number>>(new Map());

  // Mirror the live view so `reload` can read the prior slice on error without
  // depending on `view` (which would recreate the callback on every store
  // change and retrigger the fetch effect).
  const viewRef = useRef(view);
  viewRef.current = view;

  const reload = useCallback(async () => {
    const seq = (seqRef.current.get(key) ?? 0) + 1;
    seqRef.current.set(key, seq);
    const isLatest = () => seqRef.current.get(key) === seq;

    setViewLoading(key, true);
    try {
      const result = await fetcher();
      if (isLatest()) {
        setView(key, result.files, result.total);
      }
    } catch (error) {
      console.error('useView fetch failed:', error);
      if (isLatest()) {
        // Keep the prior rows on a transient failure; only fall back to an
        // empty view when there is genuinely nothing to preserve.
        if (viewRef.current.ids.length === 0) {
          setView(key, [], 0);
        } else {
          setViewLoading(key, false);
        }
      }
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
