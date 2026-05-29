import { Store, useStore } from '@tanstack/react-store';
import {
  enqueueRemoteDownload,
  onRemoteDownloadProgress,
  translateError,
} from '@/lib/tauri';
import { patchFile } from '@/stores/fileStore';
import type { RemoteDownloadProgress } from '@/types';

interface RemoteDownloadState {
  /** Append-only queue across the session. Same shape as the upload store
   *  so the matching panel UI is interchangeable. */
  downloads: RemoteDownloadProgress[];
  showPanel: boolean;
  minimized: boolean;
}

const initialState: RemoteDownloadState = {
  downloads: [],
  showPanel: false,
  minimized: false,
};

const store = new Store<RemoteDownloadState>(initialState);

let listenerStarted = false;
let listenerPromise: Promise<void> | null = null;

async function ensureListener(): Promise<void> {
  if (listenerStarted) return;
  if (listenerPromise) {
    await listenerPromise;
    return;
  }
  listenerPromise = (async () => {
    try {
      await onRemoteDownloadProgress((event) => {
        store.setState((s) => ({
          ...s,
          downloads: s.downloads.map((d) =>
            d.file_id === event.file_id
              ? {
                  ...d,
                  status: event.status,
                  error: event.error,
                  file_name: event.file_name || d.file_name,
                  // Carry the absolute cache path forward when the success
                  // event arrives. The Rust struct uses
                  // `skip_serializing_if = "Option::is_none"`, so non-success
                  // events omit the field on the wire — `?? d.local_cache_path`
                  // preserves whatever the row already had.
                  local_cache_path:
                    event.local_cache_path ?? d.local_cache_path,
                }
              : d
          ),
        }));
        // On success, patch the normalized file-store with the REAL absolute
        // cache path so "Show in Finder" / Open resolve to a real fs path
        // without a list refetch. NO sentinel fallback: the backend's
        // success branch always sets `local_cache_path`, so a missing field
        // means the worker is buggy — leave the existing value untouched
        // rather than write a fake "cached" string that would re-introduce
        // the silent reveal-in-Finder failure mode this PR fixed.
        if (event.status === 'success' && event.local_cache_path) {
          patchFile(event.file_id, {
            local_cache_path: event.local_cache_path,
          });
        }
      });
      listenerStarted = true;
    } catch (err) {
      // Reset so a subsequent enqueueDownload can retry the listen()
      // handshake instead of awaiting the same rejected promise forever
      // (transient webview-not-ready races at app startup, etc).
      listenerPromise = null;
      throw err;
    }
  })();
  await listenerPromise;
}

export function useRemoteDownloadStore(): RemoteDownloadState {
  return useStore(store, (s) => s);
}

function hasInFlight(s: RemoteDownloadState): boolean {
  return s.downloads.some(
    (d) => d.status === 'pending' || d.status === 'downloading'
  );
}

export async function enqueueDownload(
  fileIds: number[],
  fileNames: Map<number, string>
): Promise<void> {
  await ensureListener();

  const inFlightIds = new Set(
    store.state.downloads
      .filter((d) => d.status === 'pending' || d.status === 'downloading')
      .map((d) => d.file_id)
  );
  const newIds = fileIds.filter((id) => !inFlightIds.has(id));
  if (newIds.length === 0) return;

  const newRows: RemoteDownloadProgress[] = newIds.map((id) => ({
    file_id: id,
    file_name: fileNames.get(id) ?? `File ${id}`,
    status: 'pending',
  }));

  // Drop any prior terminal-state row for a file_id we're re-queuing.
  // The pending/downloading filter at line 87 already excludes in-flight
  // ids, so anything left in `newIds` is either fresh or an old error/
  // success row sitting in the panel. Without this filter a retry would
  // produce two rows with the same file_id, and the listener's `.map`
  // matches by file_id — both rows would then update in lockstep on every
  // event, indistinguishable in the UI.
  const newIdSet = new Set(newIds);
  store.setState((s) => ({
    ...s,
    downloads: [
      ...s.downloads.filter((d) => !newIdSet.has(d.file_id)),
      ...newRows,
    ],
    showPanel: true,
    minimized: false,
  }));

  try {
    await enqueueRemoteDownload(newIds);
  } catch (err) {
    const ids = new Set(newIds);
    const errMsg = translateError(err instanceof Error ? err.message : String(err));
    store.setState((s) => ({
      ...s,
      downloads: s.downloads.map((d) =>
        ids.has(d.file_id) && d.status === 'pending'
          ? { ...d, status: 'error', error: errMsg }
          : d
      ),
    }));
  }
}

export function minimizeDownloadPanel(): void {
  store.setState((s) => ({ ...s, minimized: true }));
}

export function expandDownloadPanel(): void {
  store.setState((s) => ({ ...s, minimized: false }));
}

export function dismissDownloadPanel(): void {
  store.setState((s) => {
    if (hasInFlight(s)) return s;
    return { ...s, showPanel: false, minimized: false };
  });
}

export function clearCompletedDownloads(): void {
  store.setState((s) => ({
    ...s,
    downloads: s.downloads.filter(
      (d) => d.status === 'pending' || d.status === 'downloading'
    ),
  }));
}
