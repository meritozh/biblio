import { memo, useCallback, useLayoutEffect, useMemo } from 'react';
import { useStore } from '@tanstack/react-store';
import { FileListContent } from '@/components/FileListContent';
import { fileStore } from '@/stores/fileStore';
import type { FileEntry } from '@/types';
import {
  conditionsOf,
  sortByOf,
  sortDescOf,
  visibleEntriesFromInputs,
} from './controller';
import { useFileListControllerContext } from './context';
import {
  useInFlightAnyIds,
  useInFlightDeleteIds,
  useInFlightDownloadIds,
  useInFlightUploadIds,
} from './workerHooks';

export const FileListContentConnected = memo(function FileListContentConnected() {
  const controller = useFileListControllerContext();
  const ids = useStore(controller.store, (s) => s.ids);
  const total = useStore(controller.store, (s) => s.total);
  const loadingMore = useStore(controller.store, (s) => s.loadingMore);
  const remoteEnabled = useStore(controller.store, (s) => s.remoteEnabled);
  const sortBy = useStore(controller.store, sortByOf);
  const sortDesc = useStore(controller.store, sortDescOf);
  const applySort = useStore(controller.store, (s) => s.applySort);
  const conditions = useStore(controller.store, conditionsOf);
  const applyConditionsClientSide = useStore(controller.store, (s) => s.applyConditionsClientSide);
  const viewMode = useStore(controller.store, (s) => s.viewMode);
  const collections = useStore(controller.store, (s) => s.collections);
  const breadcrumbLabel = useStore(controller.store, (s) => s.breadcrumbLabel);
  const selectionMode = useStore(controller.store, (s) => s.selectionMode);
  const selectedIds = useStore(controller.store, (s) => s.selectedIds);
  const byId = useStore(fileStore, (s) => s.byId);
  const inFlightUploadIds = useInFlightUploadIds();
  const inFlightDownloadIds = useInFlightDownloadIds();
  const inFlightDeleteIds = useInFlightDeleteIds();
  const inFlightAnyIds = useInFlightAnyIds(
    inFlightUploadIds,
    inFlightDownloadIds,
    inFlightDeleteIds
  );

  const { importableEntries, visibleEntries } = useMemo(
    () =>
      visibleEntriesFromInputs(
        ids,
        applyConditionsClientSide,
        conditions,
        applySort,
        sortBy,
        sortDesc,
        byId
      ),
    [ids, applyConditionsClientSide, conditions, applySort, sortBy, sortDesc, byId]
  );
  const eligibleIds = useMemo(
    () => visibleEntries.filter((f) => !inFlightAnyIds.has(f.id)).map((f) => f.id),
    [visibleEntries, inFlightAnyIds]
  );

  useLayoutEffect(() => {
    controller.setVisibleState(visibleEntries.length, eligibleIds);
  }, [controller, visibleEntries.length, eligibleIds]);

  const handleCardClick = useCallback(
    (file: FileEntry) => controller.handleCardClick(file, inFlightAnyIds),
    [controller, inFlightAnyIds]
  );
  const handleToggleSelect = useCallback(
    (id: number) => controller.toggleSelection(id),
    [controller]
  );

  return (
    <FileListContent
      scrollContainerRef={controller.scrollContainerRef}
      visibleEntries={visibleEntries}
      hasImportableEntries={importableEntries.length > 0}
      showCollections={viewMode !== 'flat' && breadcrumbLabel == null && collections != null}
      collections={collections}
      viewMode={viewMode}
      total={total}
      loadingMore={loadingMore}
      onLoadMore={controller.handleLoadMore}
      loadedCount={ids.length}
      selectionMode={selectionMode}
      selectedIds={selectedIds}
      inFlightAnyIds={inFlightAnyIds}
      inFlightUploadIds={inFlightUploadIds}
      onCardClick={handleCardClick}
      onToggleSelect={handleToggleSelect}
      onFileEdit={controller.handleFileEdit}
      onFileDelete={controller.handleFileDelete}
      onOpenCollection={controller.handleOpenCollection}
      remoteEnabled={remoteEnabled}
    />
  );
});
