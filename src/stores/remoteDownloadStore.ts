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
              }
            : d
        ),
      }));
      // Once a download succeeds, the row gains a local cache. Patching
      // the normalized file store flips the "cached locally" badge on the
      // card without a file-list refetch. The exact path isn't on the
      // event payload — the card just needs a truthy `local_cache_path`,
      // so we set a sentinel; subsequent loads of the row will replace it
      // with the real path from the DB.
      if (event.status === 'success') {
        patchFile(event.file_id, { local_cache_path: 'cached' });
      }
    });
    listenerStarted = true;
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

  store.setState((s) => ({
    ...s,
    downloads: [...s.downloads, ...newRows],
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
