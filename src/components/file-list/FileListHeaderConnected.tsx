import { memo, useCallback, useMemo } from 'react';
import { useStore } from '@tanstack/react-store';
import { FileListHeader } from '@/components/FileListHeader';
import { fileStore } from '@/stores/fileStore';
import type { Author, Tag } from '@/types';
import {
  conditionsOf,
  showCollectionsOf,
  sortByOf,
  sortDescOf,
} from './controller';
import { useFileListControllerContext } from './context';
import {
  useInFlightDeleteIds,
  useInFlightDownloadIds,
  useInFlightUploadIds,
} from './workerHooks';

export const FileListHeaderConnected = memo(function FileListHeaderConnected() {
  const controller = useFileListControllerContext();
  const showCollections = useStore(controller.store, showCollectionsOf);
  const viewMode = useStore(controller.store, (s) => s.viewMode);
  const viewModeAvailable = useStore(controller.store, (s) => s.viewModeAvailable);
  const hasViewModeChange = useStore(controller.store, (s) => s.hasViewModeChange);
  const sortBy = useStore(controller.store, sortByOf);
  const sortDesc = useStore(controller.store, sortDescOf);
  const conditions = useStore(controller.store, conditionsOf);
  const filterOpen = useStore(controller.store, (s) => s.filterOpen);
  const availableTags = useStore(controller.store, (s) => s.availableTags);
  const availableAuthors = useStore(controller.store, (s) => s.availableAuthors);
  const selectionMode = useStore(controller.store, (s) => s.selectionMode);
  const selectedIds = useStore(controller.store, (s) => s.selectedIds);
  const visibleCount = useStore(controller.store, (s) => s.visibleCount);
  const remoteEnabled = useStore(controller.store, (s) => s.remoteEnabled);
  const canBulkDownload = useStore(controller.store, (s) => s.canBulkDownload);
  const canBulkDelete = useStore(controller.store, (s) => s.canBulkDelete);
  const canBulkClearCache = useStore(controller.store, (s) => s.canBulkClearCache);
  const breadcrumbLabel = useStore(controller.store, (s) => s.breadcrumbLabel);
  const inFlightUploadIds = useInFlightUploadIds();
  const inFlightDownloadIds = useInFlightDownloadIds();
  const inFlightDeleteIds = useInFlightDeleteIds();
  const hasCacheableSelection = useStore(fileStore, (s) => {
    for (const id of selectedIds) {
      const f = s.byId.get(id);
      if (f && f.local_cache_path != null && f.local_cache_path !== '') {
        return true;
      }
    }
    return false;
  });

  const tagsById = useMemo(() => {
    const m = new Map<number, Tag>();
    for (const t of availableTags) m.set(t.id, t);
    return m;
  }, [availableTags]);
  const authorsById = useMemo(() => {
    const m = new Map<number, Author>();
    for (const a of availableAuthors) m.set(a.id, a);
    return m;
  }, [availableAuthors]);

  const onUpload = useCallback(() => {
    controller.handleBulkUpload(inFlightUploadIds);
  }, [controller, inFlightUploadIds]);
  const onDownload = useCallback(() => {
    controller.handleBulkDownload(inFlightDownloadIds);
  }, [controller, inFlightDownloadIds]);
  const onDelete = useCallback(() => {
    controller.handleBulkDelete(inFlightDeleteIds);
  }, [controller, inFlightDeleteIds]);

  return (
    <FileListHeader
      showCollections={showCollections}
      view={{
        viewMode,
        onViewModeChange: hasViewModeChange ? controller.setViewMode : undefined,
        available: viewModeAvailable,
      }}
      sort={{
        sortBy,
        sortDesc,
        setSortBy: controller.setSortBy,
        setSortDesc: controller.setSortDesc,
      }}
      filter={{
        conditions,
        setConditions: controller.setConditions,
        filterOpen,
        setFilterOpen: controller.setFilterOpen,
        removeCondition: controller.removeCondition,
        availableTags,
        availableAuthors,
        tagsById,
        authorsById,
      }}
      selection={{
        selectionMode,
        selectedCount: selectedIds.size,
        visibleCount,
        enterSelectionMode: controller.enterSelectionMode,
        exitSelectionMode: controller.exitSelectionMode,
        clearSelection: controller.clearSelection,
        selectFirstN: controller.selectFirstN,
      }}
      bulk={{
        remoteEnabled,
        canDownload: canBulkDownload,
        canDelete: canBulkDelete,
        canClearCache: canBulkClearCache,
        hasCacheableSelection,
        onUpload,
        onDownload,
        onDelete,
        onClearCache: controller.handleBulkClearCache,
      }}
      breadcrumb={
        breadcrumbLabel ? { label: breadcrumbLabel, onBack: controller.handleBack } : null
      }
    />
  );
});
