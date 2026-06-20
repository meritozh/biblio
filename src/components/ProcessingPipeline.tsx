import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { DynamicMetadataFormValues } from '@/components/DynamicMetadataForm';
import { ProcessingPipelineDialog } from '@/components/processing-pipeline/ProcessingPipelineDialog';
import { MinimizedPipelinePill } from '@/components/processing-pipeline/ProcessingPipelineParts';
import {
  EMPTY_FORM_VALUES,
  type Bucket,
  type FileItemState,
  type FileStatus,
  type ProcessingPipelineProps,
  type TabKey,
} from '@/components/processing-pipeline/types';
import {
  bucketOf,
  mergeReplaceParams,
  needsDuplicateDecision,
  normalizeCatalogName,
} from '@/components/processing-pipeline/helpers';
import {
  authorList,
  cancelProcessing,
  fileCreate,
  fileDeleteSource,
  fileReplace,
  importFinalize,
  listenFilePrepared,
  listenProcessingProgress,
  preparedCoverClear,
  tagList,
} from '@/lib/tauri';
import { defaultCategoryIdForSchema, schemaForPath } from '@/lib/categorySchema';
import type { DuplicateAction, MetadataType } from '@/types';

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
              item.path === p.current_file ? { ...item, status: p.status as FileStatus } : item
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
                    staged_cover_path: result.cover_mime_type ? result.path : undefined,
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
        const fileName = path.substring(Math.max(0, path.lastIndexOf('/') + 1)) || path;
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
        .join('\0'),
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
      prev.map((item) => (item.path === path ? { ...item, selected: !item.selected } : item))
    );
  }, []);

  const handleFormChange = useCallback((path: string, values: DynamicMetadataFormValues) => {
    setFileItems((prev) =>
      prev.map((item) =>
        item.path === path ? { ...item, formValues: values, userEdited: true } : item
      )
    );
  }, []);

  // Find an existing row whose name matches `name` under the same key the
  // Rust pipeline's resolve nodes use (NFC + trim + lowercase). When the
  // user adopts an LLM suggestion that happens to already exist (or that
  // a different casing / Unicode form of already exists), we reuse the
  // row instead of creating a duplicate.
  const findExistingId = useCallback(
    (name: string, snapshot: ReadonlyArray<{ id: number; name: string }>): number | null => {
      const key = normalizeCatalogName(name);
      if (!key) return null;
      const hit = snapshot.find((row) => normalizeCatalogName(row.name) === key);
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
            {
              id: created.id,
              name: created.name,
              color: created.color,
              created_at: created.created_at,
            },
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
      const approvedKey = normalizeCatalogName(authorName);
      if (!approvedKey) return;

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
          const hasMatchingHint = item.suggestedAuthors.some(
            (name) => normalizeCatalogName(name) === approvedKey
          );
          if (item.path !== path && !hasMatchingHint) return item;

          const alreadyHas = item.formValues.author_ids.includes(resolvedId);
          return {
            ...item,
            userEdited: true,
            suggestedAuthors: item.suggestedAuthors.filter(
              (name) => normalizeCatalogName(name) !== approvedKey
            ),
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

  const handleDismissSuggestedAuthor = useCallback((path: string, authorName: string) => {
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
  }, []);

  const handleDuplicateAction = useCallback((path: string, action: DuplicateAction) => {
    setFileItems((prev) =>
      prev.map((item) => (item.path === path ? { ...item, duplicateAction: action } : item))
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

  const handleToggleAllInTab = useCallback((tab: TabKey, value: boolean) => {
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
  }, []);

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

          if (item.duplicateAction === 'Replace' && item.preparedImport?.duplicate_of) {
            // Inherit-on-empty: where the new file's form left a field
            // blank (typically because LLM extraction missed it, not
            // because the user explicitly cleared it), fall back to the
            // existing row's value. Replace then carries forward the
            // metadata the user already curated on the existing file
            // instead of resetting it to whatever the LLM extracted.
            const existingId = item.preparedImport.duplicate_of.existing_file_id;
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
    <ProcessingPipelineDialog
      open={open}
      onOpenChange={onOpenChange}
      onMinimize={onMinimize}
      totalFiles={totalFiles}
      processingCount={processingCount}
      analyzedCount={analyzedCount}
      activeTab={activeTab}
      onActiveTabChange={setActiveTab}
      buckets={buckets}
      expandedIds={expandedIds}
      onToggleExpand={handleToggleExpand}
      onToggleSelected={handleToggleSelected}
      onToggleAllInTab={handleToggleAllInTab}
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
      selectedNeedingDecision={selectedNeedingDecision}
      selectedToImport={selectedToImport}
      selectedToDelete={selectedToDelete}
      analyzing={analyzing}
      importing={importing}
      onCancelAnalysis={handleCancelAnalysis}
      onImport={handleImport}
    />
  );
}
