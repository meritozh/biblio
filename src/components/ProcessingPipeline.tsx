import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { UnlistenFn } from '@tauri-apps/api/event';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import {
  Tabs,
  TabsList,
  TabsTrigger,
  TabsContent,
} from '@/components/ui/tabs';
import { Badge } from '@/components/ui/badge';
import {
  DynamicMetadataForm,
  type DynamicMetadataFormValues,
} from '@/components/DynamicMetadataForm';
import { SuggestedTagChip } from '@/components/SuggestedTagChip';
import { DuplicateWarning } from '@/components/DuplicateWarning';
import {
  fileCreate,
  fileReplace,
  fileDeleteSource,
  cancelProcessing,
  listenProcessingProgress,
  listenFilePrepared,
  settingsGet,
  importFinalize,
} from '@/lib/tauri';
import {
  REGISTRY,
  defaultCategoryIdForSchema,
  defaultSchema,
  schemaForCategoryId,
  schemaForPath,
} from '@/lib/categorySchema';
import {
  ChevronDown,
  ChevronRight,
  Loader2,
  CheckCircle2,
  AlertCircle,
  AlertTriangle,
  FileText,
  FolderArchive,
} from 'lucide-react';
import type {
  Category,
  Tag,
  Author,
  FilePreparedImport,
  MetadataType,
  DuplicateAction,
  FileAnalysisStatus,
  StorageKind,
} from '@/types';

type FileStatus = FileAnalysisStatus;

/**
 * Which panel a file belongs in:
 *   - `processing`: still being analyzed — not yet in any tab, shown only as a
 *     header-level progress counter.
 *   - `review`: analysis complete but needs human attention (duplicate detected
 *     OR partial metadata extraction).
 *   - `ready`: analysis complete, all signals healthy, safe to batch-import.
 *   - `failed`: analysis errored. Cannot be imported.
 */
type Bucket = 'processing' | 'review' | 'ready' | 'failed';

interface FileItemState {
  path: string;
  fileName: string;
  status: FileStatus;
  selected: boolean;
  preparedImport?: FilePreparedImport;
  formValues: DynamicMetadataFormValues;
  error?: string;
  userEdited: boolean;
  suggestedTags: string[];
  duplicateAction: DuplicateAction | null;
  /** Where the file ends up on commit. Comic archives default to remote
   *  (Baidu Pan); everything else defaults to local storage. */
  storageKind: StorageKind;
}

function defaultStorageKind(path: string, remoteEnabled: boolean): StorageKind {
  // Folder imports (no file extension) are auto-zipped into comics on
  // commit, so they take the comic schema's storage default rather than
  // falling through to `local`.
  const isFolderImport = !path.includes('.') || path.endsWith('/');
  const schemaDefault =
    schemaForPath(path)?.defaultStorage ??
    (isFolderImport ? REGISTRY.comic.defaultStorage : 'local');
  if (schemaDefault === 'remote' && !remoteEnabled) return 'local';
  return schemaDefault;
}

function StorageKindToggle({
  value,
  disabled,
  onChange,
}: {
  value: StorageKind;
  disabled: boolean;
  onChange: (kind: StorageKind) => void;
}) {
  return (
    <div
      role="radiogroup"
      aria-label="Destination"
      className={`shrink-0 inline-flex rounded-full border bg-secondary/40 text-xs ${
        disabled ? 'opacity-40 pointer-events-none' : ''
      }`}
      onClick={(e) => e.stopPropagation()}
    >
      <StorageKindOption
        label="Local"
        selected={value === 'local'}
        onClick={() => onChange('local')}
      />
      <StorageKindOption
        label="Remote"
        selected={value === 'remote'}
        onClick={() => onChange('remote')}
      />
    </div>
  );
}

function StorageKindOption({
  label,
  selected,
  onClick,
}: {
  label: string;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      onClick={onClick}
      className={`px-2.5 py-0.5 rounded-full transition-colors ${
        selected
          ? 'bg-primary text-primary-foreground font-medium'
          : 'text-muted-foreground hover:text-foreground'
      }`}
    >
      {label}
    </button>
  );
}

interface ProcessingPipelineProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  paths: string[];
  /** Per-path map of source folder. Keys are entries in `paths`; values
   *  are the folders the user picked. Empty for non-folder picks. The
   *  set of unique values gives the list of roots to clean up after
   *  import; per-path values let the backend group comics by root for
   *  the parent-dir author hint. */
  pathFolderRoots?: Record<string, string>;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onCategoryCreated: (category: Category) => void;
  onTagCreate: (name: string) => Promise<Tag>;
  onAuthorCreate: (name: string) => Promise<Author>;
  onImportComplete: () => void;
}

const EMPTY_FORM_VALUES: DynamicMetadataFormValues = {
  display_name: '',
  category_id: null,
  tag_ids: [],
  author_ids: [],
  metadata: [],
  progress: '',
};

function bucketOf(item: FileItemState): Bucket {
  if (item.status === 'error') return 'failed';
  if (
    item.status === 'pending' ||
    item.status === 'extracting_name' ||
    item.status === 'analyzing_content'
  ) {
    return 'processing';
  }
  // status is 'ready' or 'partial' — decide between review and ready
  if (item.status === 'partial' || item.preparedImport?.duplicate_of) {
    return 'review';
  }
  return 'ready';
}

type TabKey = 'review' | 'ready' | 'failed';

export function ProcessingPipeline({
  open,
  onOpenChange,
  paths,
  pathFolderRoots,
  categories,
  tags,
  authors,
  onCategoryCreated,
  onTagCreate,
  onAuthorCreate,
  onImportComplete,
}: ProcessingPipelineProps) {
  const [fileItems, setFileItems] = useState<FileItemState[]>([]);
  const [importing, setImporting] = useState(false);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [activeTab, setActiveTab] = useState<TabKey>('review');

  // Refs so the long-lived listener callbacks always read the latest props
  // / context without re-subscribing on every parent re-render.
  const remoteEnabledRef = useRef<boolean>(true);
  const categoriesRef = useRef(categories);
  const onAuthorCreateRef = useRef(onAuthorCreate);
  const createdAuthorIdsRef = useRef<Record<string, number>>({});
  const listenersRef = useRef<{
    progress?: UnlistenFn;
    prepared?: UnlistenFn;
  }>({});

  useEffect(() => {
    categoriesRef.current = categories;
  }, [categories]);
  useEffect(() => {
    onAuthorCreateRef.current = onAuthorCreate;
  }, [onAuthorCreate]);

  // Listener-setup effect: runs on dialog open, tears down on close. The
  // worker emits per-file events for everything the route enqueues; we
  // listen once and route by path.
  useEffect(() => {
    if (!open) {
      listenersRef.current.progress?.();
      listenersRef.current.prepared?.();
      listenersRef.current = {};
      // Cancel any queued analysis so the worker doesn't burn LLM tokens
      // on files the user is no longer reviewing. Mirrors the original
      // pre-queue behavior where closing the dialog ended the batch.
      void cancelProcessing();
      setFileItems([]);
      setExpandedIds(new Set());
      createdAuthorIdsRef.current = {};
      return;
    }

    let cancelled = false;

    void (async () => {
      // Comic schema defaults to remote, but the user may have disabled
      // remote uploads in Debug Settings. Honor that override at import
      // time so dropped/picked comics start out as `local`. Fetched once
      // per dialog open; cached in a ref so the path-diff effect can read
      // it synchronously.
      try {
        const remoteRaw = await settingsGet('debug_remote_upload_enabled');
        remoteEnabledRef.current = remoteRaw !== 'false';
      } catch {
        // Default true on settings read failure.
        remoteEnabledRef.current = true;
      }

      if (cancelled) return;

      try {
        const unsub = await listenProcessingProgress((p) => {
          setFileItems((prev) =>
            prev.map((item) =>
              item.path === p.current_file
                ? { ...item, status: p.status as FileStatus }
                : item
            )
          );
        });
        if (cancelled) {
          unsub();
        } else {
          listenersRef.current.progress = unsub;
        }
      } catch (error) {
        console.error('Failed to listen for progress:', error);
      }

      try {
        const unsub = await listenFilePrepared(async (result) => {
          // Auto-create unresolved authors. The cache is dialog-scoped via
          // the ref so multiple enqueues within one open share dedup.
          for (const name of result.unresolved_author_names) {
            if (!createdAuthorIdsRef.current[name]) {
              try {
                const newAuthor = await onAuthorCreateRef.current(name);
                createdAuthorIdsRef.current[name] = newAuthor.id;
              } catch {
                // Already exists or creation failed — ignore
              }
            }
          }

          const allAuthorIds = [...result.author_ids];
          for (const name of result.unresolved_author_names) {
            const id = createdAuthorIdsRef.current[name];
            if (id && !allAuthorIds.includes(id)) {
              allAuthorIds.push(id);
            }
          }

          setFileItems((prev) =>
            prev.map((item) => {
              if (item.path !== result.path) return item;

              // Folder imports always become comics on commit; everything
              // else routes by extension.
              const itemSchemaSlug = result.source_is_directory
                ? 'comic'
                : (schemaForPath(item.path)?.slug ?? null);
              const resolvedCategoryId =
                result.category_id ??
                (itemSchemaSlug
                  ? defaultCategoryIdForSchema(itemSchemaSlug, categoriesRef.current)
                  : null);
              const formValues: DynamicMetadataFormValues = item.userEdited
                ? item.formValues
                : {
                    display_name: result.display_name || result.file_name,
                    category_id: resolvedCategoryId,
                    tag_ids: result.tag_ids,
                    author_ids: allAuthorIds,
                    metadata: result.metadata.map((m) => ({
                      key: m.key,
                      value: m.value,
                      data_type: m.data_type as MetadataType,
                    })),
                    progress: result.progress ?? '',
                    cover_data: result.cover_data,
                    cover_mime_type: result.cover_mime_type,
                  };
              return {
                ...item,
                status:
                  item.status === 'partial' || item.status === 'error'
                    ? item.status
                    : ('ready' as FileStatus),
                preparedImport: result,
                formValues,
                suggestedTags: result.suggested_tags ?? [],
                duplicateAction:
                  item.duplicateAction ??
                  result.duplicate_of?.recommendation ??
                  null,
              };
            })
          );

          if (result.duplicate_of) {
            setExpandedIds((prev) => {
              const next = new Set(prev);
              next.add(result.path);
              return next;
            });
          }
        });
        if (cancelled) {
          unsub();
        } else {
          listenersRef.current.prepared = unsub;
        }
      } catch (error) {
        console.error('Failed to listen for file-prepared:', error);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [open]);

  // Path-diff effect: append placeholder items for paths the dialog hasn't
  // seen yet. Existing items keep their state (analysis results, user
  // edits) so an enqueue mid-review doesn't clobber the user's work.
  useEffect(() => {
    if (!open || paths.length === 0) return;
    setFileItems((prev) => {
      const known = new Set(prev.map((i) => i.path));
      const additions: FileItemState[] = [];
      for (const path of paths) {
        if (known.has(path)) continue;
        const fileName =
          path.substring(Math.max(0, path.lastIndexOf('/') + 1)) || path;
        additions.push({
          path,
          fileName,
          status: 'pending' as FileStatus,
          selected: true,
          formValues: { ...EMPTY_FORM_VALUES, display_name: fileName },
          userEdited: false,
          suggestedTags: [],
          duplicateAction: null,
          storageKind: defaultStorageKind(fileName, remoteEnabledRef.current),
        });
      }
      return additions.length === 0 ? prev : [...prev, ...additions];
    });
    setImporting(false);
  }, [open, paths]);

  // `analyzing` is derived from item statuses now that the queue model has
  // no batch boundary — true while any item is in a non-terminal state.
  const analyzing = useMemo(
    () =>
      fileItems.some(
        (i) =>
          i.status === 'pending' ||
          i.status === 'extracting_name' ||
          i.status === 'analyzing_content'
      ),
    [fileItems]
  );

  // Auto-deselect items that transitioned to error after initial analysis.
  // (The streaming path can mark a file ready → we then import it and it
  //  fails → status becomes error; in that case leave selected untouched so
  //  the error stays visible in Failed tab without surprising re-check.)
  useEffect(() => {
    setFileItems((prev) => {
      let changed = false;
      const next = prev.map((item) => {
        if (item.status === 'error' && item.selected) {
          changed = true;
          return { ...item, selected: false };
        }
        return item;
      });
      return changed ? next : prev;
    });
  }, [fileItems.length]);

  const handleCancelAnalysis = useCallback(async () => {
    await cancelProcessing();
    // Mid-flight items roll back to `pending` for visibility; the dialog
    // close clears state via the listener-setup effect's cleanup branch.
    setFileItems((prev) =>
      prev.map((item) =>
        item.status === 'extracting_name' || item.status === 'analyzing_content'
          ? { ...item, status: 'pending' as FileStatus }
          : item
      )
    );
    onOpenChange(false);
  }, [onOpenChange]);

  const handleToggleExpand = useCallback((path: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  const handleToggleSelected = useCallback((path: string) => {
    setFileItems((prev) =>
      prev.map((item) =>
        item.path === path ? { ...item, selected: !item.selected } : item
      )
    );
  }, []);

  const handleFormChange = useCallback(
    (path: string, values: DynamicMetadataFormValues) => {
      setFileItems((prev) =>
        prev.map((item) =>
          item.path === path
            ? { ...item, formValues: values, userEdited: true }
            : item
        )
      );
    },
    []
  );

  const handleApproveSuggestedTag = useCallback(
    async (path: string, tagName: string) => {
      try {
        const newTag = await onTagCreate(tagName);
        setFileItems((prev) =>
          prev.map((item) => {
            if (item.path !== path) return item;
            return {
              ...item,
              suggestedTags: item.suggestedTags.filter((t) => t !== tagName),
              formValues: {
                ...item.formValues,
                tag_ids: [...item.formValues.tag_ids, newTag.id],
              },
            };
          })
        );
      } catch (error) {
        console.error('Failed to create tag:', error);
      }
    },
    [onTagCreate]
  );

  const handleDismissSuggestedTag = useCallback((path: string, tagName: string) => {
    setFileItems((prev) =>
      prev.map((item) =>
        item.path === path
          ? { ...item, suggestedTags: item.suggestedTags.filter((t) => t !== tagName) }
          : item
      )
    );
  }, []);

  const handleDuplicateAction = useCallback((path: string, action: DuplicateAction) => {
    setFileItems((prev) =>
      prev.map((item) =>
        item.path === path ? { ...item, duplicateAction: action } : item
      )
    );
  }, []);

  const handleStorageKindChange = useCallback(
    (path: string, kind: StorageKind) => {
      setFileItems((prev) =>
        prev.map((item) => (item.path === path ? { ...item, storageKind: kind } : item))
      );
    },
    []
  );

  // Bucket all items once per render.
  const buckets = useMemo(() => {
    const b: Record<Bucket, FileItemState[]> = {
      processing: [],
      review: [],
      ready: [],
      failed: [],
    };
    for (const item of fileItems) {
      b[bucketOf(item)].push(item);
    }
    return b;
  }, [fileItems]);

  // Auto-switch to the first non-empty tab once processing settles.
  // Prefer Review (needs attention), then Ready, then Failed.
  useEffect(() => {
    if (analyzing) return;
    if (buckets.review.length > 0) setActiveTab('review');
    else if (buckets.ready.length > 0) setActiveTab('ready');
    else if (buckets.failed.length > 0) setActiveTab('failed');
  }, [analyzing, buckets.review.length, buckets.ready.length, buckets.failed.length]);

  const handleToggleAllInTab = useCallback(
    (tab: TabKey, value: boolean) => {
      setFileItems((prev) =>
        prev.map((item) => {
          const itemTab: TabKey | null =
            bucketOf(item) === 'review'
              ? 'review'
              : bucketOf(item) === 'ready'
                ? 'ready'
                : bucketOf(item) === 'failed'
                  ? 'failed'
                  : null;
          if (itemTab === tab) {
            // Never auto-re-select Failed items (they have no data to import).
            if (tab === 'failed') return item;
            return { ...item, selected: value };
          }
          return item;
        })
      );
    },
    []
  );

  const handleImport = useCallback(async () => {
    if (importing) return;

    // Only process non-failed items that the user has checked.
    const toProcess = fileItems.filter(
      (item) =>
        item.selected &&
        (item.status === 'ready' || item.status === 'partial')
    );
    if (toProcess.length === 0) return;

    // Clicking Import is a commitment to the current snapshot — stop the
    // backend from chewing through any still-queued LLM calls. The cancel
    // flag is cooperative (see processing.rs) so the currently in-flight
    // file may still finish, but the queue past it is halted immediately.
    if (analyzing) {
      try {
        await cancelProcessing();
      } catch (error) {
        console.error('Failed to cancel queued analysis:', error);
      }
    }

    setImporting(true);
    const errors: string[] = [];

    try {
      for (const item of toProcess) {
        if (item.duplicateAction === 'Delete') {
          try {
            await fileDeleteSource(item.path);
          } catch (error) {
            console.error(`Failed to delete source ${item.fileName}:`, error);
            errors.push(`${item.fileName} (delete): ${String(error)}`);
          }
          continue;
        }

        try {
          const createParams = {
            path: item.path,
            display_name: item.formValues.display_name,
            category_id: item.formValues.category_id,
            tag_ids: item.formValues.tag_ids,
            author_ids: item.formValues.author_ids,
            metadata: item.formValues.metadata,
            progress: item.formValues.progress,
            cover_data: item.formValues.cover_data,
            cover_mime_type: item.formValues.cover_mime_type,
            storage_kind: item.storageKind,
          };

          if (
            item.duplicateAction === 'Replace' &&
            item.preparedImport?.duplicate_of
          ) {
            await fileReplace(
              item.preparedImport.duplicate_of.existing_file_id,
              createParams
            );
          } else {
            await fileCreate(createParams);
          }
        } catch (error) {
          console.error(`Failed to import ${item.fileName}:`, error);
          errors.push(`${item.fileName}: ${String(error)}`);
          setFileItems((prev) =>
            prev.map((f) =>
              f.path === item.path
                ? {
                    ...f,
                    status: 'error' as FileStatus,
                    error: String(error),
                    selected: false,
                  }
                : f
            )
          );
        }
      }

      if (errors.length === 0) {
        // Best-effort cleanup of empty subdirs left behind by
        // folder-to-zip imports. One finalize call per picked root;
        // backend gates on copy-mode and `had_folder_imports`. We scope
        // `had_folder_imports` per root so a root with only archive
        // files (and no auto-zip) keeps its source tree untouched.
        if (pathFolderRoots && Object.keys(pathFolderRoots).length > 0) {
          const importedDirsByRoot = new Map<string, boolean>();
          for (const item of toProcess) {
            const root = pathFolderRoots[item.path];
            if (!root) continue;
            if (item.preparedImport?.source_is_directory) {
              importedDirsByRoot.set(root, true);
            } else if (!importedDirsByRoot.has(root)) {
              importedDirsByRoot.set(root, false);
            }
          }
          await Promise.all(
            Array.from(importedDirsByRoot.entries()).map(async ([root, hadFolderImports]) => {
              try {
                await importFinalize(root, hadFolderImports);
              } catch (error) {
                console.error(`Import finalize failed for ${root}:`, error);
              }
            })
          );
        }
        onOpenChange(false);
        onImportComplete();
      } else {
        alert(`Some files failed to import:\n${errors.join('\n')}`);
      }
    } finally {
      setImporting(false);
    }
  }, [fileItems, importing, analyzing, pathFolderRoots, onOpenChange, onImportComplete]);

  const processingCount = buckets.processing.length;
  const totalFiles = fileItems.length;
  const analyzedCount = totalFiles - processingCount;

  const selectedToImport = fileItems.filter(
    (item) =>
      item.selected &&
      (item.status === 'ready' || item.status === 'partial') &&
      item.duplicateAction !== 'Delete'
  ).length;
  const selectedToDelete = fileItems.filter(
    (item) =>
      item.selected &&
      (item.status === 'ready' || item.status === 'partial') &&
      item.duplicateAction === 'Delete'
  ).length;

  return (
    <Dialog open={open} onOpenChange={onOpenChange} modal={false}>
      <DialogContent
        className="max-w-3xl max-h-[90vh] flex flex-col overflow-hidden"
        onInteractOutside={(e) => e.preventDefault()}
        onEscapeKeyDown={(e) => e.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle className="flex items-baseline gap-3">
            <span>Import</span>
            <span className="font-serif-italic text-sm text-muted-foreground">
              — {totalFiles} {totalFiles === 1 ? 'file' : 'files'}
            </span>
          </DialogTitle>
        </DialogHeader>

        {/* Streaming progress strip: visible while any file is in flight */}
        {processingCount > 0 && (
          <div className="flex items-center gap-2 px-1 pb-2 text-xs text-muted-foreground">
            <Loader2 className="h-3.5 w-3.5 animate-spin text-primary" />
            <span>
              Analyzing {analyzedCount + 1} of {totalFiles}…
            </span>
            <div className="flex-1 h-[3px] rounded-full bg-muted overflow-hidden ml-2">
              <div
                className="h-full bg-primary transition-[width] duration-300"
                style={{ width: `${(analyzedCount / Math.max(1, totalFiles)) * 100}%` }}
              />
            </div>
          </div>
        )}

        <Tabs
          value={activeTab}
          onValueChange={(v) => setActiveTab(v as TabKey)}
          className="flex-1 flex flex-col min-h-0"
        >
          <TabsList className="self-start">
            <TabsTrigger value="review" className="gap-2">
              Review
              <CountBadge count={buckets.review.length} tone="warning" />
            </TabsTrigger>
            <TabsTrigger value="ready" className="gap-2">
              Ready
              <CountBadge count={buckets.ready.length} tone="success" />
            </TabsTrigger>
            <TabsTrigger value="failed" className="gap-2">
              Failed
              <CountBadge count={buckets.failed.length} tone="destructive" />
            </TabsTrigger>
          </TabsList>

          <TabPanel
            tabKey="review"
            isActive={activeTab === 'review'}
            items={buckets.review}
            emptyLabel="No files need review."
            expandedIds={expandedIds}
            onToggleExpand={handleToggleExpand}
            onToggleSelected={handleToggleSelected}
            onToggleAll={handleToggleAllInTab}
            onFormChange={handleFormChange}
            onApproveSuggestedTag={handleApproveSuggestedTag}
            onDismissSuggestedTag={handleDismissSuggestedTag}
            onDuplicateAction={handleDuplicateAction}
            onStorageKindChange={handleStorageKindChange}
            categories={categories}
            tags={tags}
            authors={authors}
            onCategoryCreated={onCategoryCreated}
            onTagCreate={onTagCreate}
            onAuthorCreate={onAuthorCreate}
          />

          <TabPanel
            tabKey="ready"
            isActive={activeTab === 'ready'}
            items={buckets.ready}
            emptyLabel={
              analyzing
                ? 'Waiting for analysis to finish…'
                : 'No files are ready yet.'
            }
            expandedIds={expandedIds}
            onToggleExpand={handleToggleExpand}
            onToggleSelected={handleToggleSelected}
            onToggleAll={handleToggleAllInTab}
            onFormChange={handleFormChange}
            onApproveSuggestedTag={handleApproveSuggestedTag}
            onDismissSuggestedTag={handleDismissSuggestedTag}
            onDuplicateAction={handleDuplicateAction}
            onStorageKindChange={handleStorageKindChange}
            categories={categories}
            tags={tags}
            authors={authors}
            onCategoryCreated={onCategoryCreated}
            onTagCreate={onTagCreate}
            onAuthorCreate={onAuthorCreate}
          />

          <TabPanel
            tabKey="failed"
            isActive={activeTab === 'failed'}
            items={buckets.failed}
            emptyLabel="Nothing failed."
            expandedIds={expandedIds}
            onToggleExpand={handleToggleExpand}
            onToggleSelected={handleToggleSelected}
            onToggleAll={handleToggleAllInTab}
            onFormChange={handleFormChange}
            onApproveSuggestedTag={handleApproveSuggestedTag}
            onDismissSuggestedTag={handleDismissSuggestedTag}
            onDuplicateAction={handleDuplicateAction}
            onStorageKindChange={handleStorageKindChange}
            categories={categories}
            tags={tags}
            authors={authors}
            onCategoryCreated={onCategoryCreated}
            onTagCreate={onTagCreate}
            onAuthorCreate={onAuthorCreate}
          />
        </Tabs>

        <DialogFooter className="flex-col gap-3 sm:flex-row sm:justify-between">
          <div className="text-xs text-muted-foreground">
            {selectedToImport > 0 && (
              <span>
                {selectedToImport} selected to import
                {selectedToDelete > 0 && (
                  <span className="ml-2">· {selectedToDelete} to delete</span>
                )}
              </span>
            )}
            {selectedToImport === 0 && selectedToDelete > 0 && (
              <span>{selectedToDelete} to delete</span>
            )}
            {selectedToImport === 0 && selectedToDelete === 0 && (
              <span>Nothing selected.</span>
            )}
          </div>

          <div className="flex gap-2">
            <Button
              variant="outline"
              onClick={analyzing ? handleCancelAnalysis : () => onOpenChange(false)}
            >
              {analyzing ? 'Cancel' : 'Close'}
            </Button>
            <Button
              onClick={handleImport}
              disabled={
                importing ||
                (selectedToImport === 0 && selectedToDelete === 0)
              }
            >
              {importing ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Importing…
                </>
              ) : selectedToImport > 0 ? (
                `Import ${selectedToImport}${
                  selectedToDelete > 0 ? ` (${selectedToDelete} delete)` : ''
                }`
              ) : (
                `Delete ${selectedToDelete} file${selectedToDelete !== 1 ? 's' : ''}`
              )}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/* ---------------- Internal helpers ---------------- */

function CountBadge({
  count,
  tone,
}: {
  count: number;
  tone: 'warning' | 'success' | 'destructive';
}) {
  if (count === 0) {
    return (
      <span className="text-xs text-muted-foreground/60 tabular-nums">0</span>
    );
  }
  const variant =
    tone === 'warning' ? 'orange' : tone === 'success' ? 'green' : 'destructive';
  return (
    <Badge
      variant={variant as 'orange' | 'green' | 'destructive'}
      className="px-1.5 py-0 h-5 text-[11px] tabular-nums"
    >
      {count}
    </Badge>
  );
}

interface TabPanelProps {
  tabKey: TabKey;
  isActive: boolean;
  items: FileItemState[];
  emptyLabel: string;
  expandedIds: Set<string>;
  onToggleExpand: (path: string) => void;
  onToggleSelected: (path: string) => void;
  onToggleAll: (tab: TabKey, value: boolean) => void;
  onFormChange: (path: string, values: DynamicMetadataFormValues) => void;
  onApproveSuggestedTag: (path: string, tagName: string) => void;
  onDismissSuggestedTag: (path: string, tagName: string) => void;
  onDuplicateAction: (path: string, action: DuplicateAction) => void;
  onStorageKindChange: (path: string, kind: StorageKind) => void;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onCategoryCreated: (category: Category) => void;
  onTagCreate: (name: string) => Promise<Tag>;
  onAuthorCreate: (name: string) => Promise<Author>;
}

function TabPanel({
  tabKey,
  isActive,
  items,
  emptyLabel,
  expandedIds,
  onToggleExpand,
  onToggleSelected,
  onToggleAll,
  onFormChange,
  onApproveSuggestedTag,
  onDismissSuggestedTag,
  onDuplicateAction,
  onStorageKindChange,
  categories,
  tags,
  authors,
  onCategoryCreated,
  onTagCreate,
  onAuthorCreate,
}: TabPanelProps) {
  const selectableItems = tabKey === 'failed' ? [] : items;
  const selectedCount = items.filter((i) => i.selected).length;
  const allSelected =
    selectableItems.length > 0 && selectedCount === selectableItems.length;

  const parentRef = useRef<HTMLDivElement>(null);

  // `enabled: isActive` is load-bearing: the scroll element sits inside a
  // `data-[state=inactive]:hidden` ancestor, so its ResizeObserver rect stays
  // at 0×0 while inactive. Toggling enabled on activation forces the
  // virtualizer to re-subscribe the observer against the now-visible element.
  const virtualizer = useVirtualizer({
    count: items.length,
    enabled: isActive,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 72,
    overscan: 5,
    getItemKey: (index) => items[index]!.path,
  });

  return (
    <TabsContent
      value={tabKey}
      forceMount
      className="flex-1 min-h-0 flex flex-col data-[state=inactive]:hidden"
    >
      {items.length > 0 && tabKey !== 'failed' && (
        <div className="flex items-center justify-between px-1 pt-1 pb-2 text-xs text-muted-foreground shrink-0">
          <span>
            {selectedCount} of {items.length} selected
          </span>
          <button
            type="button"
            onClick={() => onToggleAll(tabKey, !allSelected)}
            className="text-primary hover:underline"
          >
            {allSelected ? 'Deselect all' : 'Select all'}
          </button>
        </div>
      )}
      <div ref={parentRef} className="flex-1 min-h-0 -mx-6 overflow-y-auto">
        {items.length === 0 ? (
          <div className="py-10 text-center text-sm text-muted-foreground font-serif-italic">
            {emptyLabel}
          </div>
        ) : (
          <div
            style={{
              height: virtualizer.getTotalSize(),
              width: '100%',
              position: 'relative',
            }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const item = items[virtualRow.index]!;
              return (
                <div
                  key={virtualRow.key}
                  data-index={virtualRow.index}
                  ref={virtualizer.measureElement}
                  className="px-6 pb-2"
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    width: '100%',
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                >
                  <FileCardRow
                    item={item}
                    tabKey={tabKey}
                    expanded={expandedIds.has(item.path)}
                    onToggleExpand={onToggleExpand}
                    onToggleSelected={onToggleSelected}
                    onFormChange={onFormChange}
                    onApproveSuggestedTag={onApproveSuggestedTag}
                    onDismissSuggestedTag={onDismissSuggestedTag}
                    onDuplicateAction={onDuplicateAction}
                    onStorageKindChange={onStorageKindChange}
                    categories={categories}
                    tags={tags}
                    authors={authors}
                    onCategoryCreated={onCategoryCreated}
                    onTagCreate={onTagCreate}
                    onAuthorCreate={onAuthorCreate}
                  />
                </div>
              );
            })}
          </div>
        )}
      </div>
    </TabsContent>
  );
}

interface FileCardRowProps {
  item: FileItemState;
  tabKey: TabKey;
  expanded: boolean;
  onToggleExpand: (path: string) => void;
  onToggleSelected: (path: string) => void;
  onFormChange: (path: string, values: DynamicMetadataFormValues) => void;
  onApproveSuggestedTag: (path: string, tagName: string) => void;
  onDismissSuggestedTag: (path: string, tagName: string) => void;
  onDuplicateAction: (path: string, action: DuplicateAction) => void;
  onStorageKindChange: (path: string, kind: StorageKind) => void;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onCategoryCreated: (category: Category) => void;
  onTagCreate: (name: string) => Promise<Tag>;
  onAuthorCreate: (name: string) => Promise<Author>;
}

function FileCardRow({
  item,
  tabKey,
  expanded,
  onToggleExpand,
  onToggleSelected,
  onFormChange,
  onApproveSuggestedTag,
  onDismissSuggestedTag,
  onDuplicateAction,
  onStorageKindChange,
  categories,
  tags,
  authors,
  onCategoryCreated,
  onTagCreate,
  onAuthorCreate,
}: FileCardRowProps) {
  const canExpand =
    tabKey !== 'failed' && (item.status === 'ready' || item.status === 'partial');
  const checkboxDisabled = tabKey === 'failed';
  const toggleDisabled = tabKey === 'failed' || !item.selected;

  return (
    <Card
      className={`transition-all duration-200 ${
        !item.selected && tabKey !== 'failed' ? 'opacity-60' : ''
      }`}
    >
      <CardContent className="p-3">
        <div className="flex items-center gap-3">
          {/* Checkbox */}
          <input
            type="checkbox"
            checked={item.selected}
            disabled={checkboxDisabled}
            onChange={() => onToggleSelected(item.path)}
            onClick={(e) => e.stopPropagation()}
            className="h-4 w-4 shrink-0 accent-primary cursor-pointer disabled:cursor-not-allowed"
            aria-label={`Include ${item.fileName}`}
          />

          {/* Clickable header — toggles expand (only when there's something to show) */}
          <div
            className={`flex-1 min-w-0 flex items-center gap-3 ${
              canExpand ? 'cursor-pointer hover:text-primary/90' : ''
            }`}
            onClick={canExpand ? () => onToggleExpand(item.path) : undefined}
          >
            <StatusIcon status={item.status} />

            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2 min-w-0">
                <p className="text-sm font-medium truncate">{item.fileName}</p>
                {item.preparedImport?.source_is_directory && <FolderToZipHint />}
              </div>
              <StatusSubtitle item={item} />
            </div>

            <StorageKindToggle
              value={item.storageKind}
              disabled={toggleDisabled}
              onChange={(kind) => onStorageKindChange(item.path, kind)}
            />

            {canExpand && (
              <div className="shrink-0 text-muted-foreground">
                {expanded ? (
                  <ChevronDown className="h-4 w-4" />
                ) : (
                  <ChevronRight className="h-4 w-4" />
                )}
              </div>
            )}
          </div>
        </div>

        {/* Expandable form */}
        {expanded && canExpand && (
          <div className="mt-4 pt-4 border-t border-border space-y-4">
            {item.preparedImport?.duplicate_of && (
              <DuplicateWarning
                duplicateInfo={item.preparedImport.duplicate_of}
                newProgress={item.formValues.progress ?? null}
                selectedAction={
                  item.duplicateAction ?? item.preparedImport.duplicate_of.recommendation
                }
                onActionChange={(action) => onDuplicateAction(item.path, action)}
              />
            )}

            {item.suggestedTags.length > 0 && (
              <div className="space-y-2">
                <p className="text-xs text-muted-foreground">Suggested new tags:</p>
                <div className="flex flex-wrap gap-1.5">
                  {item.suggestedTags.map((tag) => (
                    <SuggestedTagChip
                      key={tag}
                      name={tag}
                      onApprove={(name) => onApproveSuggestedTag(item.path, name)}
                      onDismiss={(name) => onDismissSuggestedTag(item.path, name)}
                    />
                  ))}
                </div>
              </div>
            )}

            <DynamicMetadataForm
              values={item.formValues}
              onChange={(values) => onFormChange(item.path, values)}
              schema={
                // Prefer the schema picked by the user's selected
                // category, since that's what the file will end up
                // tagged with. Fall back to the path-based schema for
                // the moment between drop and category resolution; for
                // folder imports (which auto-zip into comics) use the
                // comic schema directly.
                item.formValues.category_id != null
                  ? schemaForCategoryId(item.formValues.category_id, categories)
                  : schemaForPath(item.path) ??
                    (item.preparedImport?.source_is_directory
                      ? REGISTRY.comic
                      : defaultSchema())
              }
              categories={categories}
              tags={tags}
              authors={authors}
              onCategoryCreated={onCategoryCreated}
              onTagCreate={onTagCreate}
              onAuthorCreate={onAuthorCreate}
            />
          </div>
        )}

        {/* Error message */}
        {item.status === 'error' && item.error && (
          <div className="mt-2 ml-7 text-xs text-destructive">{item.error}</div>
        )}
      </CardContent>
    </Card>
  );
}

function FolderToZipHint() {
  return (
    <span
      className="shrink-0 inline-flex items-center gap-1 rounded-full border border-border/60 bg-secondary/40 px-1.5 py-0 text-[10px] text-muted-foreground"
      title="This folder of images will be packaged as a .zip on import"
    >
      <FolderArchive className="h-3 w-3" aria-hidden="true" />
      Folder → .zip
    </span>
  );
}

function StatusIcon({ status }: { status: FileStatus }) {
  return (
    <div className="shrink-0">
      {status === 'extracting_name' || status === 'analyzing_content' ? (
        <Loader2 className="h-4 w-4 animate-spin text-primary" />
      ) : status === 'ready' ? (
        <CheckCircle2 className="h-4 w-4 text-notion-green" />
      ) : status === 'partial' ? (
        <AlertTriangle className="h-4 w-4 text-notion-orange" />
      ) : status === 'error' ? (
        <AlertCircle className="h-4 w-4 text-destructive" />
      ) : (
        <FileText className="h-4 w-4 text-muted-foreground" />
      )}
    </div>
  );
}

function StatusSubtitle({ item }: { item: FileItemState }) {
  if (item.status === 'extracting_name') {
    return <p className="text-xs text-muted-foreground">Extracting name…</p>;
  }
  if (item.status === 'analyzing_content') {
    return <p className="text-xs text-muted-foreground">Analyzing content…</p>;
  }
  if (item.status === 'partial') {
    return (
      <p className="text-xs text-notion-orange">
        Partial extraction — please fill missing fields
      </p>
    );
  }
  if (item.preparedImport?.duplicate_of) {
    const d = item.preparedImport.duplicate_of;
    return (
      <p className="text-xs text-muted-foreground">
        Duplicate of{' '}
        <span className="font-serif-italic">{d.existing_display_name}</span>
        {d.existing_progress ? ` (${d.existing_progress})` : ''}
      </p>
    );
  }
  return null;
}
