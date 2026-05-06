import { Store, useStore } from '@tanstack/react-store';
import { fileUploadToRemote, onRemoteUploadProgress, translateError } from '@/lib/tauri';
import type { RemoteUploadProgress } from '@/types';
import type { UnlistenFn } from '@tauri-apps/api/event';

interface RemoteUploadState {
  uploads: RemoteUploadProgress[];
  isUploading: boolean;
  showPanel: boolean;
}

const initialState: RemoteUploadState = {
  uploads: [],
  isUploading: false,
  showPanel: false,
};

const store = new Store<RemoteUploadState>(initialState);

export function useRemoteUploadStore(): RemoteUploadState {
  return useStore(store, (s) => s);
}

export async function startUpload(fileIds: number[], fileNames: Map<number, string>) {
  const uploads: RemoteUploadProgress[] = fileIds.map((id) => ({
    file_id: id,
    file_name: fileNames.get(id) ?? `File ${id}`,
    status: 'uploading' as const,
    current: 0,
    total: fileIds.length,
  }));

  store.setState((s) => ({
    ...s,
    uploads,
    isUploading: true,
    showPanel: true,
  }));

  let unlisten: UnlistenFn | undefined;

  try {
    unlisten = await onRemoteUploadProgress((event) => {
      store.setState((s) => ({
        ...s,
        uploads: s.uploads.map((u) =>
          u.file_id === event.file_id ? { ...event } : u
        ),
      }));

      const allDone = store.state.uploads.every(
        (u) => u.status !== 'uploading'
      );
      if (allDone) {
        store.setState((s) => ({ ...s, isUploading: false }));
      }
    });

    await fileUploadToRemote(fileIds);
  } catch (err) {
    store.setState((s) => ({
      ...s,
      isUploading: false,
      uploads: s.uploads.map((u) =>
        u.status === 'uploading'
          ? { ...u, status: 'error' as const, error: translateError(err instanceof Error ? err.message : String(err)) }
          : u
      ),
    }));
  } finally {
    unlisten?.();
  }
}

export function dismissPanel() {
  store.setState((s) => ({ ...s, showPanel: false }));
}

export function clearUploads() {
  store.setState(() => initialState);
}
