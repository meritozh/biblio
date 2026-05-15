import { Store, useStore } from '@tanstack/react-store';
import {
  enqueueRemoteDelete,
  onRemoteDeleteProgress,
  translateError,
} from '@/lib/tauri';
import { removeFile } from '@/stores/fileStore';
import type { RemoteDeleteProgress } from '@/types';

interface RemoteDeleteState {
  deletes: RemoteDeleteProgress[];
  showPanel: boolean;
  minimized: boolean;
}

const initialState: RemoteDeleteState = {
  deletes: [],
  showPanel: false,
  minimized: false,
};

const store = new Store<RemoteDeleteState>(initialState);

let listenerStarted = false;
let listenerPromise: Promise<void> | null = null;

async function ensureListener(): Promise<void> {
  if (listenerStarted) return;
  if (listenerPromise) {
    await listenerPromise;
    return;
  }
  listenerPromise = (async () => {
    await onRemoteDeleteProgress((event) => {
      store.setState((s) => ({
        ...s,
        deletes: s.deletes.map((d) =>
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
      // Drop the row from the normalized store on success so the card
      // disappears from the grid without a file-list refetch.
      if (event.status === 'success') {
        removeFile(event.file_id);
      }
    });
    listenerStarted = true;
  })();
  await listenerPromise;
}

export function useRemoteDeleteStore(): RemoteDeleteState {
  return useStore(store, (s) => s);
}

function hasInFlight(s: RemoteDeleteState): boolean {
  return s.deletes.some(
    (d) => d.status === 'pending' || d.status === 'deleting'
  );
}

export async function enqueueDelete(
  fileIds: number[],
  fileNames: Map<number, string>
): Promise<void> {
  await ensureListener();

  const inFlightIds = new Set(
    store.state.deletes
      .filter((d) => d.status === 'pending' || d.status === 'deleting')
      .map((d) => d.file_id)
  );
  const newIds = fileIds.filter((id) => !inFlightIds.has(id));
  if (newIds.length === 0) return;

  const newRows: RemoteDeleteProgress[] = newIds.map((id) => ({
    file_id: id,
    file_name: fileNames.get(id) ?? `File ${id}`,
    status: 'pending',
  }));

  store.setState((s) => ({
    ...s,
    deletes: [...s.deletes, ...newRows],
    showPanel: true,
    minimized: false,
  }));

  try {
    await enqueueRemoteDelete(newIds);
  } catch (err) {
    const ids = new Set(newIds);
    const errMsg = translateError(err instanceof Error ? err.message : String(err));
    store.setState((s) => ({
      ...s,
      deletes: s.deletes.map((d) =>
        ids.has(d.file_id) && d.status === 'pending'
          ? { ...d, status: 'error', error: errMsg }
          : d
      ),
    }));
  }
}

export function minimizeDeletePanel(): void {
  store.setState((s) => ({ ...s, minimized: true }));
}

export function expandDeletePanel(): void {
  store.setState((s) => ({ ...s, minimized: false }));
}

export function dismissDeletePanel(): void {
  store.setState((s) => {
    if (hasInFlight(s)) return s;
    return { ...s, showPanel: false, minimized: false };
  });
}

export function clearCompletedDeletes(): void {
  store.setState((s) => ({
    ...s,
    deletes: s.deletes.filter(
      (d) => d.status === 'pending' || d.status === 'deleting'
    ),
  }));
}
