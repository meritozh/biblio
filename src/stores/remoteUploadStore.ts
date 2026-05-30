import { Store, useStore } from '@tanstack/react-store';
import { enqueueRemoteUpload, onRemoteUploadProgress, translateError } from '@/lib/tauri';
import { patchFile } from '@/stores/fileStore';
import type { RemoteUploadProgress } from '@/types';

interface RemoteUploadState {
  /** Append-only queue across the session. Includes pending, in-flight, and
   *  finished entries; finished rows linger until the user clears them so
   *  they remain visible while new work is enqueued. */
  uploads: RemoteUploadProgress[];
  /** Whether the bottom-anchored panel is visible at all. */
  showPanel: boolean;
  /** Collapsed state. While in-flight work exists, the user can minimize
   *  the panel (tucking it into a single-line strip) but not fully dismiss
   *  it; full dismissal is only allowed once the queue is idle. */
  minimized: boolean;
}

const initialState: RemoteUploadState = {
  uploads: [],
  showPanel: false,
  minimized: false,
};

const store = new Store<RemoteUploadState>(initialState);

// One long-lived progress listener for the whole session. The worker emits
// per-file events; we look up the matching upload by file_id and patch its
// status. Setting this up once means subsequent enqueueUpload calls don't
// race on listener setup, and finished rows stay updated even after the
// caller's enqueue promise resolves.
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
      await onRemoteUploadProgress((event) => {
        store.setState((s) => ({
          ...s,
          uploads: s.uploads.map((u) =>
            u.file_id === event.file_id
              ? { ...u, status: event.status, error: event.error, file_name: event.file_name || u.file_name }
              : u
          ),
        }));
        // On a successful upload, patch the file's row in the normalized
        // store so the card's storage badge flips to cloud without any
        // file-list refetch. The provider name isn't on the event payload,
        // so we leave `remote_provider` alone — the row already has it from
        // its initial load.
        if (event.status === 'success') {
          patchFile(event.file_id, { storage_kind: 'remote' });
        }
      });
      listenerStarted = true;
    } catch (err) {
      // Reset so a subsequent enqueueUpload can retry the listen()
      // handshake instead of awaiting the same rejected promise forever
      // (transient webview-not-ready races at app startup, etc).
      listenerPromise = null;
      throw err;
    }
  })();
  await listenerPromise;
}

export function useRemoteUploadStore(): RemoteUploadState {
  return useStore(store, (s) => s);
}

/** True when at least one upload is queued or actively running. Drives the
 *  panel's "can the user fully dismiss this?" gating. */
function hasInFlight(s: RemoteUploadState): boolean {
  return s.uploads.some((u) => u.status === 'pending' || u.status === 'uploading');
}

/** Push file IDs into the upload worker queue. Skips any IDs that already
 *  have an in-flight (pending/uploading) row to prevent duplicate queueing
 *  when the user re-selects files mid-upload. Auto-expands the panel so
 *  newly enqueued work is visible. */
export async function enqueueUpload(
  fileIds: number[],
  fileNames: Map<number, string>
): Promise<void> {
  await ensureListener();

  const inFlightIds = new Set(
    store.state.uploads
      .filter((u) => u.status === 'pending' || u.status === 'uploading')
      .map((u) => u.file_id)
  );
  const newIds = fileIds.filter((id) => !inFlightIds.has(id));
  if (newIds.length === 0) return;

  const uniqueIds = Array.from(new Set(newIds));

  const newRows: RemoteUploadProgress[] = uniqueIds.map((id) => ({
    file_id: id,
    file_name: fileNames.get(id) ?? `File ${id}`,
    status: 'pending',
  }));

  // Drop any prior terminal-state row for a file_id we're re-queuing.
  // The pending/uploading filter above already excludes in-flight ids, so
  // anything left in `newIds` is either fresh or an old error/success row
  // sitting in the panel. Without this filter a retry would produce two
  // rows with the same file_id, and the listener's `.map` matches by
  // file_id — both rows would then update in lockstep on every event,
  // indistinguishable in the UI (and a React duplicate-key collision).
  const newIdSet = new Set(uniqueIds);
  store.setState((s) => ({
    ...s,
    uploads: [...s.uploads.filter((u) => !newIdSet.has(u.file_id)), ...newRows],
    showPanel: true,
    // New work always pops the panel back to expanded so the user sees it.
    minimized: false,
  }));

  try {
    await enqueueRemoteUpload(uniqueIds);
  } catch (err) {
    // The backend rejected the enqueue (queue closed, plugin error, etc.).
    // Mark the rows we just added as errored so the user sees the failure
    // instead of a perpetually pending row.
    const ids = new Set(uniqueIds);
    const errMsg = translateError(err instanceof Error ? err.message : String(err));
    store.setState((s) => ({
      ...s,
      uploads: s.uploads.map((u) =>
        ids.has(u.file_id) && u.status === 'pending'
          ? { ...u, status: 'error', error: errMsg }
          : u
      ),
    }));
  }
}

/** Collapse the panel into the minimized strip — preserves all queue state,
 *  the worker keeps running, the user can re-expand any time. */
export function minimizePanel(): void {
  store.setState((s) => ({ ...s, minimized: true }));
}

export function expandPanel(): void {
  store.setState((s) => ({ ...s, minimized: false }));
}

/** Fully dismiss the panel. No-op while in-flight work exists — the user
 *  must wait for the queue to drain (or click `Clear completed` after) so
 *  they don't accidentally lose visibility into running tasks. */
export function dismissPanel(): void {
  store.setState((s) => {
    if (hasInFlight(s)) return s;
    return { ...s, showPanel: false, minimized: false };
  });
}

/** Drop terminal-state rows (success / error / skipped) from the queue.
 *  Pending and uploading entries are preserved so an in-flight session
 *  isn't disrupted. */
export function clearCompleted(): void {
  store.setState((s) => ({
    ...s,
    uploads: s.uploads.filter((u) => u.status === 'pending' || u.status === 'uploading'),
  }));
}

export function clearUploads(): void {
  store.setState(() => initialState);
}
