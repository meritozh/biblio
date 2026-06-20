import { useMemo } from 'react';
import { useRemoteDeleteStore } from '@/stores/remoteDeleteStore';
import { useRemoteDownloadStore } from '@/stores/remoteDownloadStore';
import { useRemoteUploadStore } from '@/stores/remoteUploadStore';

export function useInFlightUploadIds(): Set<number> {
  const uploadState = useRemoteUploadStore();
  return useMemo(
    () =>
      new Set(
        uploadState.uploads
          .filter((u) => u.status === 'pending' || u.status === 'uploading')
          .map((u) => u.file_id)
      ),
    [uploadState.uploads]
  );
}

export function useInFlightDownloadIds(): Set<number> {
  const downloadState = useRemoteDownloadStore();
  return useMemo(
    () =>
      new Set(
        downloadState.downloads
          .filter((d) => d.status === 'pending' || d.status === 'downloading')
          .map((d) => d.file_id)
      ),
    [downloadState.downloads]
  );
}

export function useInFlightDeleteIds(): Set<number> {
  const deleteState = useRemoteDeleteStore();
  return useMemo(
    () =>
      new Set(
        deleteState.deletes
          .filter((d) => d.status === 'pending' || d.status === 'deleting')
          .map((d) => d.file_id)
      ),
    [deleteState.deletes]
  );
}

export function useInFlightAnyIds(
  inFlightUploadIds: Set<number>,
  inFlightDownloadIds: Set<number>,
  inFlightDeleteIds: Set<number>
): Set<number> {
  return useMemo(() => {
    const s = new Set<number>();
    for (const id of inFlightUploadIds) s.add(id);
    for (const id of inFlightDownloadIds) s.add(id);
    for (const id of inFlightDeleteIds) s.add(id);
    return s;
  }, [inFlightUploadIds, inFlightDownloadIds, inFlightDeleteIds]);
}
