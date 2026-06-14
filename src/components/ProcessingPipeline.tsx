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
import { Input } from '@/components/ui/input';
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
  authorList,
  coverGet,
  fileCreate,
  fileGet,
  fileReplace,
  fileDeleteSource,
  cancelProcessing,
  preparedCoverClear,
  listenProcessingProgress,
  listenFilePrepared,
  importFinalize,
  tagList,
  vndbSearch,
  vndbFetchCover,
  type VndbCandidate,
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
  ChevronUp,
  Loader2,
  CheckCircle2,
  AlertCircle,
  AlertTriangle,
  FileText,
  FolderArchive,
  Minus,
} from 'lucide-react';
import type {
  Category,
  Tag,
  Author,
  FileCreateRequest,
  FilePreparedImport,
  MetadataType,
  DuplicateAction,
  FileAnalysisStatus,
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
  /** LLM-extracted author names that didn't resolve against the existing
   *  catalog. Surface as chips — the user adopts (find-or-create on the
   *  authors snapshot) or dismisses. Authors are never auto-created. */
  suggestedAuthors: string[];
  duplicateAction: DuplicateAction | null;
}

interface ProcessingPipelineProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Minimize collapses the modal into a small floating pill while
   *  leaving every internal listener and the per-file state intact —
   *  the worker keeps analyzing in the background and the user can
   *  re-open later to commit. Mirrors the RemoteUploadProgressPanel
   *  minimize/expand convention so the two panels feel symmetric. */
  minimized: boolean;
  onMinimize: () => void;
  onExpand: () => void;
  paths: string[];
  /** Per-path map of source folder. Keys are entries in `paths`; values
   *  are the folders the user picked. Empty for non-folder picks. The
   *  set of unique values gives the list of roots to clean up after
   *  import; per-path values let the backend group comics by root for
   *  the parent-dir author hint. */
  pathFolderRoots?: Record<string, string>;
  /** Category-first import: the category the user is importing INTO. Drives
   *  the default category for every reviewed file (the backend already
   *  validated the input against this category's schema). Falls back to
   *  extension-based schema inference when null (legacy / no selection). */
  targetCategoryId?: number | null;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
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

/** For each empty field in the new-file's import params, fall back to the
 *  existing row's value. Called only when the user picks `Replace` on a
 *  duplicate — that action's natural reading is "keep what was there +
 *  override what I changed", not "wipe and rebuild from my new file
 *  alone." The most common case this addresses: LLM extraction missed a
 *  field on the new file, so it's blank; without the merge the
 *  user-curated tags/authors/cover on the existing row would be lost.
 *
 *  Trade-off: the merge can't tell "extraction missed" apart from
 *  "user explicitly cleared in the form" — both look empty. Users who
 *  deliberately clear and then pick Replace will see the existing value
 *  re-populate; they have to clear again on the resulting row. Rare
 *  enough vs. the common case to be the right default.
 *
 *  Cover handling: file_replace cascades-deletes the existing row's
 *  cover. If the new params have neither inline `cover_data` nor a
 *  pipeline-staged path, we fetch the existing cover via `coverGet` and
 *  stash it as `cover_data` so the new row inherits the cover bytes. The
 *  one exception is when the user explicitly removed the cover in the form
 *  (`cover_removed`): that's a deliberate clear, not a missed extraction,
 *  so we skip the inherit and let the new row land without a cover. */
async function mergeReplaceParams<T extends FileCreateRequest>(
  newParams: T,
  existingId: number,
): Promise<T> {
  const existing = await fileGet(existingId);
  const merged: T = { ...newParams };

  if (!newParams.display_name?.trim()) {
    merged.display_name = existing.display_name;
  }
  if (newParams.category_id == null) {
    merged.category_id = existing.category_id;
  }
  if (!newParams.tag_ids || newParams.tag_ids.length === 0) {
    merged.tag_ids = existing.tags.map((t) => t.id);
  }
  if (!newParams.author_ids || newParams.author_ids.length === 0) {
    merged.author_ids = existing.authors.map((a) => a.id);
  }
  if (!newParams.metadata || newParams.metadata.length === 0) {
    // Strip id + file_id off the existing metadata rows — FileCreateRequest
    // takes the writable subset only.
    merged.metadata = existing.metadata.map((m) => ({
      key: m.key,
      value: m.value,
      data_type: m.data_type,
    }));
  }
  if (!newParams.progress || !newParams.progress.trim()) {
    merged.progress = existing.progress ?? undefined;
  }
  if (
    !newParams.cover_removed &&
    !newParams.cover_data &&
    !newParams.staged_cover_path
  ) {
    try {
      const c = await coverGet(existingId);
      merged.cover_data = c.data;
      merged.cover_mime_type = c.mime_type;
    } catch {
      // No existing cover row — leave merged.cover_data undefined; the
      // new row simply won't have a cover, same as today.
    }
  }
  return merged;
}

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

function needsDuplicateDecision(item: FileItemState): boolean {
  return !!item.preparedImport?.duplicate_of && item.duplicateAction == null;
}

type TabKey = 'review' | 'ready' | 'failed';

export function ProcessingPipeline({
  open,
  onOpenChange,
  minimized,
  onMinimize,
  onExpand,
  paths,
  pathFolderRoots,
  targetCategoryId,
  categories,
  tags,
  authors,
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
  const categoriesRef = useRef(categories);
  // Category-first import: the category being imported into. Kept in a ref so
  // the long-lived `file-prepared` listener defaults each item to it without
  // re-subscribing.
  const targetCategoryIdRef = useRef(targetCategoryId);
  // Snapshots of the parent's catalog used by the adopt handlers to do an
  // in-memory case-insensitive lookup before falling back to *_create. The
  // parent passes the canonical authors/tags arrays (loaded via LIMIT -1
  // by useFileActions), so the local refs stay authoritative across re-renders.
  const authorsRef = useRef(authors);
  const tagsRef = useRef(tags);
  const listenersRef = useRef<{
    progress?: UnlistenFn;
    prepared?: UnlistenFn;
  }>({});
  // Flips true on each open transition and is consumed (reset to false) by
  // the path-diff effect's first run for that open. Lets us clear the
  // `importing` re-entrancy guard only on a genuine fresh open, not on every
  // `paths` delta — otherwise adding files mid-import would unlock Import.
  const justOpenedRef = useRef(false);

  useEffect(() => {
    categoriesRef.current = categories;
  }, [categories]);

  useEffect(() => {
    targetCategoryIdRef.current = targetCategoryId;
  }, [targetCategoryId]);
  useEffect(() => {
    authorsRef.current = authors;
  }, [authors]);
  useEffect(() => {
    tagsRef.current = tags;
  }, [tags]);

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
      // Also drop any staged cover bytes — nothing downstream will read
      // them now. (The commit-button caller of `cancelProcessing` at the
      // bottom of this file deliberately skips the cache-clear: it's
      // about to read those bytes.)
      void cancelProcessing();
      void preparedCoverClear();
      setFileItems([]);
      setExpandedIds(new Set());
      return;
    }

    // Fresh open — the path-diff effect will clear the `importing` guard
    // exactly once for this session, then reset the flag.
    justOpenedRef.current = true;

    let cancelled = false;

    void (async () => {
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
          // LLM-suggested author names are NOT auto-created anymore. They
          // arrive as `unresolved_author_names`, get surfaced as chips, and
          // only land in the DB when the user clicks Approve (via the
          // find-or-create handler below).
          setFileItems((prev) =>
            prev.map((item) => {
              if (item.path !== result.path) return item;

              // Category-first import: default to the category the user is
              // importing into (the backend already validated the input
              // against its schema). Fall back to the legacy extension guess
              // only when no target category was supplied.
              const itemSchemaSlug = result.source_is_directory
                ? 'comic'
                : (schemaForPath(item.path)?.slug ?? null);
              const resolvedCategoryId =
                result.category_id ??
                targetCategoryIdRef.current ??
                (itemSchemaSlug
                  ? defaultCategoryIdForSchema(itemSchemaSlug, categoriesRef.current)
                  : null);
              const formValues: DynamicMetadataFormValues = item.userEdited
                ? item.formValues
                : {
                    display_name: result.display_name || result.file_name,
                    category_id: resolvedCategoryId,
                    tag_ids: result.tag_ids,
                    author_ids: result.author_ids,
                    metadata: result.metadata.map((m) => ({
                      key: m.key,
                      value: m.value,
                      data_type: m.data_type as MetadataType,
                    })),
                    progress: result.progress ?? '',
                    // Cover bytes stay in the Rust-side PreparedCoverCache.
                    // `cover_mime_type` is the "there's a staged cover" flag;
                    // when set, hand the import path through as the token so
                    // the preview can lazy-fetch and the commit can claim
                    // the bytes server-side.
                    cover_mime_type: result.cover_mime_type,
                    staged_cover_path: result.cover_mime_type
                      ? result.path
                      : undefined,
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
                suggestedAuthors: result.unresolved_author_names ?? [],
                // Backend recommendations are hints, not consent. Leave
                // duplicate rows undecided until the user picks Delete,
                // Replace, or Import anyway in the review panel.
                duplicateAction: item.duplicateAction,
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
          suggestedAuthors: [],
          duplicateAction: null,
        });
      }
      return additions.length === 0 ? prev : [...prev, ...additions];
    });
    // Only clear the re-entrancy guard on a genuine fresh open. A `paths`
    // delta while an import is in flight (user adds more files mid-import)
    // must not unlock the Import button under the running loop.
    if (justOpenedRef.current) {
      justOpenedRef.current = false;
      setImporting(false);
    }
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
  //
  // Depend on a signature of the error+selected paths, not `fileItems.length`:
  // a status→error transition without a count change (the common case — a
  // ready file that fails to import) wouldn't re-run a length-keyed effect,
  // leaving the errored item checked.
  const erroredSelectedKey = useMemo(
    () =>
      fileItems
        .filter((i) => i.status === 'error' && i.selected)
        .map((i) => i.path)
        .join(' '),
    [fileItems]
  );
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
  }, [erroredSelectedKey]);

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

  // Find an existing row whose name matches `name` under the same key the
  // Rust pipeline's resolve nodes use (NFC + trim + lowercase). When the
  // user adopts an LLM suggestion that happens to already exist (or that
  // a different casing / Unicode form of already exists), we reuse the
  // row instead of creating a duplicate.
  const findExistingId = useCallback(
    (name: string, snapshot: ReadonlyArray<{ id: number; name: string }>): number | null => {
      const key = name.normalize('NFC').trim().toLowerCase();
      if (!key) return null;
      const hit = snapshot.find(
        (row) => row.name.normalize('NFC').trim().toLowerCase() === key
      );
      return hit?.id ?? null;
    },
    []
  );

  const handleApproveSuggestedTag = useCallback(
    async (path: string, tagName: string) => {
      // Try the in-memory catalog first. If the LLM suggested "Action" but
      // the DB already has "action", reuse the existing row.
      let id = findExistingId(tagName, tagsRef.current);
      if (id == null) {
        try {
          const created = await onTagCreate(tagName);
          id = created.id;
          // Eager push so a rapid second adopt of the same name (e.g. the
          // same LLM-suggested tag across multiple files in the batch)
          // resolves via the snapshot on the next click instead of taking
          // the TAG_EXISTS → refetch detour. The parent's setTags re-render
          // will overwrite this via the syncing useEffect.
          tagsRef.current = [
            ...tagsRef.current,
            { id: created.id, name: created.name, color: created.color, created_at: created.created_at },
          ];
        } catch (error) {
          // TAG_EXISTS = the snapshot was stale (another window inserted).
          // Refetch once, look up the canonical row, reuse it.
          if (String(error).includes('TAG_EXISTS')) {
            try {
              const { tags: fresh } = await tagList({ includeUsage: true });
              tagsRef.current = fresh;
              id = findExistingId(tagName, fresh);
            } catch (refetchErr) {
              console.error('Failed to refetch tags:', refetchErr);
            }
          }
          if (id == null) {
            console.error('Failed to adopt tag:', error);
            return;
          }
        }
      }
      const resolvedId = id;
      setFileItems((prev) =>
        prev.map((item) => {
          if (item.path !== path) return item;
          const alreadyHas = item.formValues.tag_ids.includes(resolvedId);
          return {
            ...item,
            suggestedTags: item.suggestedTags.filter((t) => t !== tagName),
            formValues: alreadyHas
              ? item.formValues
              : {
                  ...item.formValues,
                  tag_ids: [...item.formValues.tag_ids, resolvedId],
                },
          };
        })
      );
    },
    [onTagCreate, findExistingId]
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

  const handleApproveSuggestedAuthor = useCallback(
    async (path: string, authorName: string) => {
      let id = findExistingId(authorName, authorsRef.current);
      if (id == null) {
        try {
          const created = await onAuthorCreate(authorName);
          id = created.id;
          // Eager push — see the matching note in handleApproveSuggestedTag.
          authorsRef.current = [
            ...authorsRef.current,
            { id: created.id, name: created.name, created_at: created.created_at },
          ];
        } catch (error) {
          // AUTHOR_EXISTS = snapshot stale; refetch + reuse.
          if (String(error).includes('AUTHOR_EXISTS')) {
            try {
              const { authors: fresh } = await authorList({ includeUsage: true });
              authorsRef.current = fresh;
              id = findExistingId(authorName, fresh);
            } catch (refetchErr) {
              console.error('Failed to refetch authors:', refetchErr);
            }
          }
          if (id == null) {
            console.error('Failed to adopt author:', error);
            return;
          }
        }
      }
      const resolvedId = id;
      setFileItems((prev) =>
        prev.map((item) => {
          if (item.path !== path) return item;
          const alreadyHas = item.formValues.author_ids.includes(resolvedId);
          return {
            ...item,
            suggestedAuthors: item.suggestedAuthors.filter((n) => n !== authorName),
            formValues: alreadyHas
              ? item.formValues
              : {
                  ...item.formValues,
                  author_ids: [...item.formValues.author_ids, resolvedId],
                },
          };
        })
      );
    },
    [onAuthorCreate, findExistingId]
  );

  const handleDismissSuggestedAuthor = useCallback(
    (path: string, authorName: string) => {
      setFileItems((prev) =>
        prev.map((item) =>
          item.path === path
            ? {
                ...item,
                suggestedAuthors: item.suggestedAuthors.filter((n) => n !== authorName),
              }
            : item
        )
      );
    },
    []
  );

  const handleDuplicateAction = useCallback((path: string, action: DuplicateAction) => {
    setFileItems((prev) =>
      prev.map((item) =>
        item.path === path ? { ...item, duplicateAction: action } : item
      )
    );
  }, []);

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
        (item.status === 'ready' || item.status === 'partial') &&
        !needsDuplicateDecision(item)
    );
    if (
      fileItems.some(
        (item) =>
          item.selected &&
          (item.status === 'ready' || item.status === 'partial') &&
          needsDuplicateDecision(item)
      )
    ) {
      return;
    }
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
          let createParams = {
            path: item.path,
            display_name: item.formValues.display_name,
            category_id: item.formValues.category_id,
            tag_ids: item.formValues.tag_ids,
            author_ids: item.formValues.author_ids,
            metadata: item.formValues.metadata,
            progress: item.formValues.progress,
            cover_data: item.formValues.cover_data,
            cover_mime_type: item.formValues.cover_mime_type,
            staged_cover_path: item.formValues.staged_cover_path,
            // A user "Remove cover" sets cover_removed on the form values;
            // carry it through so a fresh create lands without a cover and
            // the Replace merge below skips inheriting the existing one.
            cover_removed: item.formValues.cover_removed,
          };

          if (
            item.duplicateAction === 'Replace' &&
            item.preparedImport?.duplicate_of
          ) {
            // Inherit-on-empty: where the new file's form left a field
            // blank (typically because LLM extraction missed it, not
            // because the user explicitly cleared it), fall back to the
            // existing row's value. Replace then carries forward the
            // metadata the user already curated on the existing file
            // instead of resetting it to whatever the LLM extracted.
            const existingId =
              item.preparedImport.duplicate_of.existing_file_id;
            createParams = await mergeReplaceParams(createParams, existingId);
            await fileReplace(existingId, createParams);
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
      !needsDuplicateDecision(item) &&
      item.duplicateAction !== 'Delete'
  ).length;
  const selectedToDelete = fileItems.filter(
    (item) =>
      item.selected &&
      (item.status === 'ready' || item.status === 'partial') &&
      item.duplicateAction === 'Delete'
  ).length;
  const selectedNeedingDecision = fileItems.filter(
    (item) =>
      item.selected &&
      (item.status === 'ready' || item.status === 'partial') &&
      needsDuplicateDecision(item)
  ).length;

  // Minimized pill — analysis keeps running in the background (the
  // listener-setup effect still depends on `open`, not on `minimized`,
  // so events continue to flow into state). Clicking the pill re-opens
  // the modal with everything intact.
  if (open && minimized) {
    return (
      <MinimizedPipelinePill
        totalFiles={totalFiles}
        readyCount={buckets.ready.length}
        reviewCount={buckets.review.length}
        failedCount={buckets.failed.length}
        processingCount={processingCount}
        analyzing={analyzing}
        onExpand={onExpand}
      />
    );
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange} modal={false}>
      <DialogContent
        className="max-w-3xl max-h-[90vh] flex flex-col overflow-hidden"
        onInteractOutside={(e) => e.preventDefault()}
        onEscapeKeyDown={(e) => e.preventDefault()}
      >
        {/* Minimize sits absolutely to the left of the shadcn Dialog's
            built-in close (the X at `right-4 top-4` in dialog.tsx) so
            both share a baseline and don't overlap. `right-12` slots in
            the gap reserved by the close's padding. Pulling it out of
            the DialogHeader flex also stops the title row from being
            squeezed by a sibling at narrow widths. */}
        <button
          type="button"
          onClick={onMinimize}
          className="absolute right-12 top-4 text-muted-foreground hover:text-foreground hover:bg-secondary p-1 rounded-xl opacity-60 hover:opacity-100 transition-opacity"
          aria-label="Minimize import dialog"
          title="Minimize — analysis keeps running"
        >
          <Minus className="h-4 w-4" />
        </button>
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
            onApproveSuggestedAuthor={handleApproveSuggestedAuthor}
            onDismissSuggestedAuthor={handleDismissSuggestedAuthor}
            onDuplicateAction={handleDuplicateAction}
            categories={categories}
            tags={tags}
            authors={authors}
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
            onApproveSuggestedAuthor={handleApproveSuggestedAuthor}
            onDismissSuggestedAuthor={handleDismissSuggestedAuthor}
            onDuplicateAction={handleDuplicateAction}
            categories={categories}
            tags={tags}
            authors={authors}
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
            onApproveSuggestedAuthor={handleApproveSuggestedAuthor}
            onDismissSuggestedAuthor={handleDismissSuggestedAuthor}
            onDuplicateAction={handleDuplicateAction}
            categories={categories}
            tags={tags}
            authors={authors}
            onTagCreate={onTagCreate}
            onAuthorCreate={onAuthorCreate}
          />
        </Tabs>

        <DialogFooter className="flex-col gap-3 sm:flex-row sm:justify-between">
          <div className="text-xs text-muted-foreground">
            {selectedNeedingDecision > 0 && (
              <span>
                {selectedNeedingDecision} duplicate decision
                {selectedNeedingDecision !== 1 ? 's' : ''} needed
              </span>
            )}
            {selectedNeedingDecision === 0 && selectedToImport > 0 && (
              <span>
                {selectedToImport} selected to import
                {selectedToDelete > 0 && (
                  <span className="ml-2">· {selectedToDelete} to delete</span>
                )}
              </span>
            )}
            {selectedNeedingDecision === 0 && selectedToImport === 0 && selectedToDelete > 0 && (
              <span>{selectedToDelete} to delete</span>
            )}
            {selectedNeedingDecision === 0 && selectedToImport === 0 && selectedToDelete === 0 && (
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
                selectedNeedingDecision > 0 ||
                (selectedToImport === 0 && selectedToDelete === 0)
              }
            >
              {importing ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Importing…
                </>
              ) : selectedNeedingDecision > 0 ? (
                `Choose ${selectedNeedingDecision} duplicate action${
                  selectedNeedingDecision !== 1 ? 's' : ''
                }`
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
  onApproveSuggestedAuthor: (path: string, authorName: string) => void;
  onDismissSuggestedAuthor: (path: string, authorName: string) => void;
  onDuplicateAction: (path: string, action: DuplicateAction) => void;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
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
  onApproveSuggestedAuthor,
  onDismissSuggestedAuthor,
  onDuplicateAction,
  categories,
  tags,
  authors,
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
                    onApproveSuggestedAuthor={onApproveSuggestedAuthor}
                    onDismissSuggestedAuthor={onDismissSuggestedAuthor}
                    onDuplicateAction={onDuplicateAction}
                    categories={categories}
                    tags={tags}
                    authors={authors}
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

/** Renders a VNDB cover thumbnail by fetching its bytes through the Rust
 *  `vndb_fetch_cover` command and showing a local data URL. We never put the
 *  remote `t.vndb.org` URL in an <img src> — the webview CSP is `self`-only,
 *  so a cross-origin image would be blocked. Falls back to a dashed
 *  placeholder while loading, on error, or when there's no cover. */
function VndbThumb({ url }: { url: string | null }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (!url) {
      setSrc(null);
      return;
    }
    let cancelled = false;
    vndbFetchCover(url)
      .then(({ data, mime_type }) => {
        if (!cancelled) setSrc(`data:${mime_type};base64,${data}`);
      })
      .catch(() => {
        if (!cancelled) setSrc(null);
      });
    return () => {
      cancelled = true;
    };
  }, [url]);
  return src ? (
    <img
      src={src}
      alt=""
      className="h-14 w-10 object-cover rounded border shrink-0"
    />
  ) : (
    <div className="h-14 w-10 rounded border border-dashed shrink-0" />
  );
}

/** Galgame-only VNDB match panel. Auto-searches on mount using the cleaned
 *  display name (the filename-LLM result, or the raw file name when the LLM is
 *  off), lets the user pick a candidate or re-search, and on pick autofills
 *  the form: origin title (alttitle → title), cover (fetched + dropped into
 *  `cover_data`), and developer (routed through the existing author-adopt
 *  handler so it reuses find-or-create). Failures degrade to manual entry. */
function GalgameVndbPanel({
  item,
  onFormChange,
  onApproveSuggestedAuthor,
}: {
  item: FileItemState;
  onFormChange: (path: string, values: DynamicMetadataFormValues) => void;
  onApproveSuggestedAuthor: (path: string, authorName: string) => void;
}) {
  const [query, setQuery] = useState(
    () => item.formValues.display_name || item.preparedImport?.file_name || ''
  );
  const [candidates, setCandidates] = useState<VndbCandidate[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [applyingId, setApplyingId] = useState<string | null>(null);

  const runSearch = useCallback(async (q: string) => {
    const trimmed = q.trim();
    if (!trimmed) {
      setCandidates([]);
      return;
    }
    setSearching(true);
    setError(null);
    try {
      setCandidates(await vndbSearch(trimmed));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setCandidates([]);
    } finally {
      setSearching(false);
    }
  }, []);

  // Auto-search once on mount with the cleaned name. Re-runs only via the
  // manual search box afterward so we don't spam the API on every re-render.
  const didAutoSearch = useRef(false);
  useEffect(() => {
    if (didAutoSearch.current) return;
    didAutoSearch.current = true;
    void runSearch(query);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const applyCandidate = async (c: VndbCandidate) => {
    setApplyingId(c.id);
    try {
      const title = c.alttitle?.trim() || c.title.trim();
      let next: DynamicMetadataFormValues = {
        ...item.formValues,
        display_name: title || item.formValues.display_name,
      };
      // Fetch the cover and inline it so the normal commit path stores it.
      if (c.image_url) {
        try {
          const { data, mime_type } = await vndbFetchCover(c.image_url);
          next = {
            ...next,
            cover_data: data,
            cover_mime_type: mime_type,
            cover_removed: false,
            staged_cover_path: undefined,
          };
        } catch (err) {
          console.error('VNDB cover fetch failed:', err);
        }
      }
      onFormChange(item.path, next);
      // Developer → author through the existing adopt handler (find-or-create
      // against the catalog snapshot). Only the first developer is adopted.
      const dev = c.developers[0]?.trim();
      if (dev) onApproveSuggestedAuthor(item.path, dev);
    } finally {
      setApplyingId(null);
    }
  };

  return (
    <div className="space-y-2">
      <p className="text-xs text-muted-foreground">VNDB match</p>
      <div className="flex items-center gap-2">
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void runSearch(query);
            }
          }}
          placeholder="Search VNDB by title…"
          className="h-8 text-sm"
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-8 shrink-0"
          onClick={() => void runSearch(query)}
          disabled={searching}
        >
          {searching ? <Loader2 className="h-4 w-4 animate-spin" /> : 'Search'}
        </Button>
      </div>

      {error && <p className="text-xs text-destructive">VNDB error: {error}</p>}

      {candidates != null && candidates.length === 0 && !searching && !error && (
        <p className="text-xs text-muted-foreground">
          No matches — edit the title and search again, or fill metadata
          manually below.
        </p>
      )}

      {candidates != null && candidates.length > 0 && (
        <ul className="space-y-1.5">
          {candidates.map((c) => (
            <li key={c.id}>
              <button
                type="button"
                onClick={() => void applyCandidate(c)}
                disabled={applyingId != null}
                className="w-full flex items-center gap-3 rounded-md border p-2 text-left hover:bg-muted/50 transition-colors disabled:opacity-50"
              >
                <VndbThumb url={c.thumbnail ?? c.image_url} />
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium truncate">
                    {c.alttitle?.trim() || c.title}
                  </p>
                  {c.alttitle?.trim() && (
                    <p className="text-xs text-muted-foreground truncate">
                      {c.title}
                    </p>
                  )}
                  <p className="text-xs text-muted-foreground truncate">
                    {[c.released, c.developers[0]]
                      .filter(Boolean)
                      .join(' · ') || '—'}
                  </p>
                </div>
                {applyingId === c.id && (
                  <Loader2 className="h-4 w-4 animate-spin shrink-0" />
                )}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
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
  onApproveSuggestedAuthor: (path: string, authorName: string) => void;
  onDismissSuggestedAuthor: (path: string, authorName: string) => void;
  onDuplicateAction: (path: string, action: DuplicateAction) => void;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
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
  onApproveSuggestedAuthor,
  onDismissSuggestedAuthor,
  onDuplicateAction,
  categories,
  tags,
  authors,
  onTagCreate,
  onAuthorCreate,
}: FileCardRowProps) {
  const canExpand =
    tabKey !== 'failed' && (item.status === 'ready' || item.status === 'partial');
  const checkboxDisabled = tabKey === 'failed';

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
        {expanded && canExpand && (() => {
          // Resolve the schema once per render so the dupe panel and the
          // form below it always agree on which slug is in play (novel
          // vs comic decides which compare rows render in the panel).
          // Same resolution logic that drove the form's `schema` prop
          // pre-refactor — lifted up so both consumers share one source.
          const resolvedSchema =
            item.formValues.category_id != null
              ? schemaForCategoryId(item.formValues.category_id, categories)
              : schemaForPath(item.path) ??
                (item.preparedImport?.source_is_directory
                  ? REGISTRY.comic
                  : defaultSchema());
          // Resolve the new-side author ids → names via the parent's
          // `authors` snapshot so the dupe panel can render the row
          // without a follow-up fetch. Ids that don't match anything
          // in the snapshot (extremely rare race) fall through to
          // `#<id>` so the panel still has something to display.
          const resolvedNewAuthors = item.formValues.author_ids
            .map((id) => authors.find((a) => a.id === id)?.name ?? `#${id}`);
          return (
          <div className="mt-4 pt-4 border-t border-border space-y-4">
            {item.preparedImport?.duplicate_of && (
              <DuplicateWarning
                duplicateInfo={item.preparedImport.duplicate_of}
                schema={resolvedSchema}
                newDisplayName={item.formValues.display_name}
                newAuthorNames={resolvedNewAuthors}
                newProgress={item.formValues.progress ?? null}
                newCoverData={item.formValues.cover_data}
                newCoverMimeType={item.formValues.cover_mime_type}
                newStagedCoverPath={item.formValues.staged_cover_path}
                selectedAction={item.duplicateAction}
                onActionChange={(action) => onDuplicateAction(item.path, action)}
              />
            )}

            {item.suggestedAuthors.length > 0 && (
              <div className="space-y-2">
                <p className="text-xs text-muted-foreground">Suggested authors:</p>
                <div className="flex flex-wrap gap-1.5">
                  {item.suggestedAuthors.map((author) => (
                    <SuggestedTagChip
                      key={author}
                      name={author}
                      noun="author"
                      onApprove={(name) => onApproveSuggestedAuthor(item.path, name)}
                      onDismiss={(name) => onDismissSuggestedAuthor(item.path, name)}
                    />
                  ))}
                </div>
              </div>
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

            {resolvedSchema.slug === 'galgame' && (
              <GalgameVndbPanel
                item={item}
                onFormChange={onFormChange}
                onApproveSuggestedAuthor={onApproveSuggestedAuthor}
              />
            )}

            <DynamicMetadataForm
              values={item.formValues}
              onChange={(values) => onFormChange(item.path, values)}
              // Reuse the resolved schema lifted to the IIFE top so the
              // form and the dupe panel agree on the slug. Resolution
              // rule unchanged: user-picked category wins; fallback to
              // path-based schema; folder imports default to comic.
              schema={resolvedSchema}
              categories={categories}
              tags={tags}
              authors={authors}
              onTagCreate={onTagCreate}
              onAuthorCreate={onAuthorCreate}
            />
          </div>
          );
        })()}

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

/** Minimized state for the import dialog — a single click-target pill
 *  that re-expands. Mirrors the shape and corner placement of the
 *  remote-upload pill so the two coexist visually. The pill summarizes
 *  in-flight + ready + failed counts; the worker keeps emitting events
 *  while minimized, so these numbers stay live. */
function MinimizedPipelinePill({
  totalFiles,
  readyCount,
  reviewCount,
  failedCount,
  processingCount,
  analyzing,
  onExpand,
}: {
  totalFiles: number;
  readyCount: number;
  reviewCount: number;
  failedCount: number;
  processingCount: number;
  analyzing: boolean;
  onExpand: () => void;
}) {
  const done = readyCount + reviewCount + failedCount;
  return (
    <button
      type="button"
      onClick={onExpand}
      className="fixed bottom-4 right-4 z-50 bg-background border border-border rounded-full shadow-lg flex items-center pl-3 pr-2 py-1 gap-2 text-xs hover:bg-secondary/40 transition-colors"
      aria-label="Expand import dialog"
    >
      {analyzing ? (
        <Loader2 className="h-3.5 w-3.5 animate-spin text-primary shrink-0" />
      ) : failedCount > 0 ? (
        <AlertCircle className="h-3.5 w-3.5 text-destructive shrink-0" />
      ) : (
        <CheckCircle2 className="h-3.5 w-3.5 text-success shrink-0" />
      )}
      <span className="text-foreground/80">
        Import {done}/{totalFiles}
        {processingCount > 0 && (
          <span className="text-muted-foreground ml-1.5">· {processingCount} analyzing</span>
        )}
        {failedCount > 0 && (
          <span className="text-destructive ml-1.5">· {failedCount} failed</span>
        )}
      </span>
      <ChevronUp className="h-3.5 w-3.5 text-muted-foreground" />
    </button>
  );
}
