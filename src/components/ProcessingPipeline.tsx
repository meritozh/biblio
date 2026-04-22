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
  filePrepareImport,
  fileCreate,
  fileReplace,
  fileDeleteSource,
  cancelProcessing,
  listenProcessingProgress,
  listenFilePrepared,
} from '@/lib/tauri';
import {
  ChevronDown,
  ChevronRight,
  Loader2,
  CheckCircle2,
  AlertCircle,
  AlertTriangle,
  FileText,
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

function defaultStorageKind(fileName: string): StorageKind {
  const lower = fileName.toLowerCase();
  if (lower.endsWith('.cbz') || lower.endsWith('.zip')) return 'remote';
  return 'local';
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
  categories,
  tags,
  authors,
  onCategoryCreated,
  onTagCreate,
  onAuthorCreate,
  onImportComplete,
}: ProcessingPipelineProps) {
  const [fileItems, setFileItems] = useState<FileItemState[]>([]);
  const [analyzing, setAnalyzing] = useState(false);
  const [importing, setImporting] = useState(false);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const [activeTab, setActiveTab] = useState<TabKey>('review');
  const analysisStarted = useRef(false);

  useEffect(() => {
    if (!open || paths.length === 0) {
      analysisStarted.current = false;
      return;
    }

    if (analysisStarted.current) return;
    analysisStarted.current = true;

    const initialItems: FileItemState[] = paths.map((path) => {
      const fileName = path.substring(Math.max(0, path.lastIndexOf('/') + 1)) || path;
      return {
        path,
        fileName,
        status: 'pending' as FileStatus,
        selected: true, // default-include; Failed items will be auto-unchecked on transition
        formValues: { ...EMPTY_FORM_VALUES, display_name: fileName },
        userEdited: false,
        suggestedTags: [],
        duplicateAction: null,
        storageKind: defaultStorageKind(fileName),
      };
    });
    setFileItems(initialItems);
    setImporting(false);
    setExpandedIds(new Set()); // start all collapsed; Review items auto-expand on arrival

    const createdAuthorIds: Record<string, number> = {};

    const runAnalysis = async () => {
      setAnalyzing(true);

      let unlistenProgress: UnlistenFn | null = null;
      let unlistenPrepared: UnlistenFn | null = null;

      try {
        unlistenProgress = await listenProcessingProgress((p) => {
          setFileItems((prev) =>
            prev.map((item) =>
              item.path === p.current_file
                ? { ...item, status: p.status as FileStatus }
                : item
            )
          );
        });
      } catch (error) {
        console.error('Failed to listen for progress:', error);
      }

      try {
        unlistenPrepared = await listenFilePrepared(async (result) => {
          // Auto-create any unresolved authors for THIS file (deduped across batch)
          for (const name of result.unresolved_author_names) {
            if (!createdAuthorIds[name]) {
              try {
                const newAuthor = await onAuthorCreate(name);
                createdAuthorIds[name] = newAuthor.id;
              } catch {
                // Already exists or creation failed — ignore
              }
            }
          }

          const allAuthorIds = [...result.author_ids];
          for (const name of result.unresolved_author_names) {
            const id = createdAuthorIds[name];
            if (id && !allAuthorIds.includes(id)) {
              allAuthorIds.push(id);
            }
          }

          setFileItems((prev) =>
            prev.map((item) => {
              if (item.path !== result.path) return item;

              const formValues: DynamicMetadataFormValues = item.userEdited
                ? item.formValues
                : {
                    display_name: result.display_name || result.file_name,
                    category_id: result.category_id,
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

          // Review items auto-expand so duplicate & partial signals are visible;
          // Ready items stay collapsed to keep the list dense.
          const needsReview =
            result.duplicate_of !== null && result.duplicate_of !== undefined;
          if (needsReview) {
            setExpandedIds((prev) => {
              const next = new Set(prev);
              next.add(result.path);
              return next;
            });
          }
        });
      } catch (error) {
        console.error('Failed to listen for file-prepared:', error);
      }

      try {
        const results = await filePrepareImport(paths);

        // Final sync: backfill anything the streaming events missed + mark
        // items that never produced a result as errored.
        setFileItems((prev) => {
          const resultsByPath = new Map(results.map((r) => [r.path, r] as const));
          return prev.map((item) => {
            const r = resultsByPath.get(item.path);
            if (!r) {
              return {
                ...item,
                status: 'error' as FileStatus,
                error: 'Analysis failed',
                selected: false,
              };
            }
            return {
              ...item,
              preparedImport: r,
              duplicateAction:
                item.duplicateAction ?? r.duplicate_of?.recommendation ?? null,
            };
          });
        });
      } catch (error) {
        console.error('Analysis failed:', error);
        setFileItems((prev) =>
          prev.map((item) => ({
            ...item,
            status: 'error' as FileStatus,
            error: String(error),
            selected: false,
          }))
        );
      } finally {
        if (unlistenProgress) unlistenProgress();
        if (unlistenPrepared) unlistenPrepared();
        setAnalyzing(false);
      }
    };

    runAnalysis();
  }, [open, paths, onAuthorCreate]);

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
    analysisStarted.current = false;
    setAnalyzing(false);
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
        onOpenChange(false);
        onImportComplete();
      } else {
        alert(`Some files failed to import:\n${errors.join('\n')}`);
      }
    } finally {
      setImporting(false);
    }
  }, [fileItems, importing, analyzing, onOpenChange, onImportComplete]);

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
    <Dialog open={open} onOpenChange={onOpenChange}>
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

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 72,
    overscan: 5,
    getItemKey: (index) => items[index]!.path,
  });

  return (
    <TabsContent
      value={tabKey}
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
              <p className="text-sm font-medium truncate">{item.fileName}</p>
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
