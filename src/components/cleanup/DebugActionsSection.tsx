import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  ChevronDown,
  ChevronRight,
  FolderInput,
  Loader2,
  Sparkles,
  Tag as TagIcon,
  UserPlus,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  PaginatedPicker,
  type PickerPage,
} from '@/components/PaginatedPicker';
import {
  authorList,
  categoryMerge,
  fileAssignAuthorToAuthorless,
  fileCountAuthorlessInCategory,
  fileCountForCategoryReanalyze,
  fileCountNovelsMissingTags,
  fileReanalyzeForCategory,
  fileReanalyzeMissingTags,
  type CategoryMergeResponse,
  type ReanalyzeError,
  type ReanalyzeResponse,
  type ReclassifyResponse,
} from '@/lib/tauri';
import { coerceSchemaSlug } from '@/lib/categorySchema';
import { loadCategories, useAppState } from '@/stores/appStore';

interface DebugActionsSectionProps {
  /** Called after a successful run so the parent can refresh its own
   *  derived state (unused-tags section, similar-names groups). The
   *  global tag-change event is also fired by the backend, so other
   *  surfaces refresh on their own. */
  onAfterRun?: () => void;
}

/** Maintenance actions that aren't destructive but still need an
 *  explicit user gesture (LLM cost, slow). Currently a single action:
 *  re-analyze novels with no tags. */
export function DebugActionsSection({ onAfterRun }: DebugActionsSectionProps) {
  const [count, setCount] = useState<number | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<ReanalyzeResponse | null>(null);
  const [errorsExpanded, setErrorsExpanded] = useState(false);

  const refreshCount = useCallback(async () => {
    try {
      const c = await fileCountNovelsMissingTags();
      setCount(c);
    } catch (err) {
      console.error('Failed to count novels missing tags:', err);
      setCount(null);
    }
  }, []);

  useEffect(() => {
    void refreshCount();
  }, [refreshCount]);

  const handleRun = useCallback(async () => {
    setRunning(true);
    setResult(null);
    try {
      const res = await fileReanalyzeMissingTags();
      setResult(res);
      onAfterRun?.();
      void refreshCount();
    } catch (err) {
      // Surface the error as a single synthetic "all failed" result so
      // the user sees what happened without a separate toast layer.
      const message = err instanceof Error ? err.message : String(err);
      setResult({
        processed: 0,
        succeeded: 0,
        failed: 0,
        errors: [
          {
            file_id: 0,
            display_name: 'Re-analyze action failed',
            message,
          },
        ],
      });
    } finally {
      setRunning(false);
      setConfirmOpen(false);
    }
  }, [refreshCount, onAfterRun]);

  return (
    <section className="space-y-4">
      <h2 className="text-sm uppercase tracking-wide text-muted-foreground font-medium">
        Debug actions
      </h2>

      <div className="rounded-lg border bg-background p-4 space-y-3">
        <div className="flex items-start justify-between gap-3">
          <div className="space-y-1 min-w-0">
            <h3 className="text-sm font-medium flex items-center gap-1.5">
              <Sparkles className="h-3.5 w-3.5 text-muted-foreground" aria-hidden="true" />
              Re-analyze novels with no tags
              {count != null && (
                <span className="text-muted-foreground font-normal">· {count}</span>
              )}
            </h3>
            <p className="text-xs text-muted-foreground leading-relaxed">
              Runs the import-time LLM content extraction on every novel that has
              zero tags. Returned tags and category are applied to each file
              (unknown tags are created, file is moved on disk if the category
              changes). Remote files without a local cache are skipped.
            </p>
          </div>
          <Button
            variant="outline"
            size="sm"
            className="h-8 text-xs shrink-0"
            onClick={() => setConfirmOpen(true)}
            disabled={running || count === 0 || count == null}
          >
            {running ? (
              <>
                <Loader2 className="h-3.5 w-3.5 mr-1 animate-spin" />
                Running…
              </>
            ) : (
              'Run'
            )}
          </Button>
        </div>

        {result != null && !running && (
          <ResultPanel
            result={result}
            expanded={errorsExpanded}
            onToggleExpand={() => setErrorsExpanded((v) => !v)}
          />
        )}
      </div>

      <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Re-analyze {count ?? 0} files?</AlertDialogTitle>
            <AlertDialogDescription>
              This calls your configured LLM once per file. At a few seconds
              each, the run can take minutes. The window stays open during the
              run — wait for the result before closing the page.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={running}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => {
                e.preventDefault();
                void handleRun();
              }}
              disabled={running}
            >
              {running ? 'Running…' : 'Run'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AssignAuthorCard onAfterRun={onAfterRun} />

      <ReclassifyToCategoryCard onAfterRun={onAfterRun} />

      <MergeCategoryCard onAfterRun={onAfterRun} />
    </section>
  );
}

interface ResultPanelProps {
  result: ReanalyzeResponse;
  expanded: boolean;
  onToggleExpand: () => void;
}

function ResultPanel({ result, expanded, onToggleExpand }: ResultPanelProps) {
  const summary =
    result.processed === 0
      ? 'No files matched — nothing to re-analyze.'
      : `${result.succeeded} re-analyzed, ${result.failed} failed (out of ${result.processed}).`;
  return (
    <div className="border-t pt-3 space-y-2">
      <p className="text-xs text-foreground">{summary}</p>
      {result.errors.length > 0 && (
        <div className="space-y-1">
          <button
            type="button"
            onClick={onToggleExpand}
            className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground focus:outline-none focus:ring-2 focus:ring-ring rounded"
            aria-expanded={expanded}
          >
            {expanded ? (
              <ChevronDown className="h-3 w-3" />
            ) : (
              <ChevronRight className="h-3 w-3" />
            )}
            {result.errors.length}{' '}
            {result.errors.length === 1 ? 'issue' : 'issues'}
          </button>
          {expanded && (
            <ul className="space-y-1 pl-4 border-l text-[11px] text-muted-foreground">
              {result.errors.map((err, i) => (
                <ErrorRow key={`${err.file_id}-${i}`} err={err} />
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}

function ErrorRow({ err }: { err: ReanalyzeError }) {
  return (
    <li className="leading-tight">
      <span className="text-foreground/80">{err.display_name}</span>
      <span className="mx-1.5">·</span>
      <span>{err.message}</span>
    </li>
  );
}

// ── Assign-author card ───────────────────────────────────────────────────────

interface AssignAuthorCardProps {
  onAfterRun?: () => void;
}

/** Bulk-assigns one existing author to every file in a chosen category
 *  (or library-wide) that currently has no author. Lives inside the
 *  Debug section because it touches many rows in one go and is
 *  irreversible via the UI. */
function AssignAuthorCard({ onAfterRun }: AssignAuthorCardProps) {
  const categories = useAppState((s) => s.categories);

  // null = all categories. Stays null after a successful run so the
  // user can re-target without re-selecting.
  const [categoryId, setCategoryId] = useState<number | null>(null);
  const [authorId, setAuthorId] = useState<number | null>(null);
  const [authorName, setAuthorName] = useState<string | null>(null);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [count, setCount] = useState<number | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<{ assigned: number } | null>(null);
  const [resultError, setResultError] = useState<string | null>(null);

  const refreshCount = useCallback(async () => {
    try {
      const c = await fileCountAuthorlessInCategory(categoryId);
      setCount(c);
    } catch (err) {
      console.error('Failed to count authorless files:', err);
      setCount(null);
    }
  }, [categoryId]);

  useEffect(() => {
    void refreshCount();
  }, [refreshCount]);

  // Single-pick fetcher for PaginatedPicker — no onCreate prop is
  // passed below, so the picker hides its inline-create row and the
  // user can only pick from the existing authors list.
  const authorFetcher = useCallback(
    async ({
      query,
      offset,
      limit,
    }: {
      query: string;
      offset: number;
      limit: number;
    }): Promise<PickerPage> => {
      const { authors: page } = await authorList({
        limit,
        offset,
        nameQuery: query.length > 0 ? query : undefined,
      });
      const total =
        page.length < limit ? offset + page.length : offset + page.length + 1;
      return {
        items: page.map((a) => ({ id: a.id, name: a.name })),
        total,
      };
    },
    []
  );

  const handleRun = useCallback(async () => {
    if (authorId == null) return;
    setRunning(true);
    setResult(null);
    setResultError(null);
    try {
      const res = await fileAssignAuthorToAuthorless(categoryId, authorId);
      setResult(res);
      onAfterRun?.();
      void refreshCount();
    } catch (err) {
      setResultError(err instanceof Error ? err.message : String(err));
    } finally {
      setRunning(false);
      setConfirmOpen(false);
    }
  }, [authorId, categoryId, onAfterRun, refreshCount]);

  const categoryLabel = useMemo(() => {
    if (categoryId == null) return 'All categories';
    return categories.find((c) => c.id === categoryId)?.name ?? 'Unknown';
  }, [categoryId, categories]);

  const canRun =
    !running && authorId != null && count != null && count > 0;

  return (
    <div className="rounded-lg border bg-background p-4 space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1 min-w-0">
          <h3 className="text-sm font-medium flex items-center gap-1.5">
            <UserPlus
              className="h-3.5 w-3.5 text-muted-foreground"
              aria-hidden="true"
            />
            Assign author to authorless files
            {count != null && (
              <span className="text-muted-foreground font-normal">· {count}</span>
            )}
          </h3>
          <p className="text-xs text-muted-foreground leading-relaxed">
            Picks every file in the chosen category that has no author and
            links it to the selected one. Existing authors only (use the
            Authors page to create a new name first). Single transaction,
            no undo.
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          className="h-8 text-xs shrink-0"
          onClick={() => setConfirmOpen(true)}
          disabled={!canRun}
        >
          {running ? (
            <>
              <Loader2 className="h-3.5 w-3.5 mr-1 animate-spin" />
              Running…
            </>
          ) : (
            'Run'
          )}
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <Select
          value={categoryId == null ? 'all' : String(categoryId)}
          onValueChange={(v) =>
            setCategoryId(v === 'all' ? null : Number(v))
          }
        >
          <SelectTrigger className="h-8 w-auto text-xs gap-1.5">
            <span className="text-muted-foreground">Category</span>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all" className="text-xs">
              All categories
            </SelectItem>
            {categories.map((c) => (
              <SelectItem key={c.id} value={String(c.id)} className="text-xs">
                {c.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Popover open={pickerOpen} onOpenChange={setPickerOpen}>
          <PopoverTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              className="h-8 text-xs gap-1.5 font-normal"
            >
              <span className="text-muted-foreground">Author</span>
              <span className={authorName ? '' : 'text-muted-foreground'}>
                {authorName ?? 'pick an author…'}
              </span>
            </Button>
          </PopoverTrigger>
          <PopoverContent align="start" sideOffset={4} className="p-0">
            <PaginatedPicker
              mode="single"
              selectedIds={authorId != null ? [authorId] : []}
              fetcher={authorFetcher}
              onSelect={(id, item) => {
                setAuthorId(id);
                setAuthorName(item.name);
                setPickerOpen(false);
              }}
              searchPlaceholder="Search authors…"
              emptyLabel="No authors defined"
              noMatchLabel="No matching authors"
            />
          </PopoverContent>
        </Popover>
      </div>

      {result != null && !running && (
        <div className="border-t pt-3">
          <p className="text-xs text-foreground">
            {result.assigned === 0
              ? 'Nothing to assign — every file in scope already has an author.'
              : `Assigned ${authorName ?? 'author'} to ${result.assigned} ${
                  result.assigned === 1 ? 'file' : 'files'
                }.`}
          </p>
        </div>
      )}
      {resultError != null && !running && (
        <div className="border-t pt-3">
          <p className="text-xs text-destructive">{resultError}</p>
        </div>
      )}

      <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              Assign &ldquo;{authorName}&rdquo; to {count ?? 0} files?
            </AlertDialogTitle>
            <AlertDialogDescription>
              Scope: <span className="font-medium">{categoryLabel}</span>.
              Each affected file currently has no author. This adds a single
              author link per file in one transaction and can&apos;t be undone
              in bulk — open a file&apos;s Edit dialog to remove an individual
              link, or use the Authors page to delete the author entirely.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={running}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => {
                e.preventDefault();
                void handleRun();
              }}
              disabled={running}
            >
              {running ? 'Running…' : 'Assign'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

// ── Re-classify card ─────────────────────────────────────────────────────────

interface ReclassifyToCategoryCardProps {
  onAfterRun?: () => void;
}

/** Re-runs the import-time content LLM on every novel-schema file that
 *  isn't already in the chosen target category, optionally narrowed to a
 *  single source category. Files whose LLM-picked category equals the
 *  target's name get moved into the target (disk + DB). The user picks
 *  the target from their novel-schema categories. */
function ReclassifyToCategoryCard({ onAfterRun }: ReclassifyToCategoryCardProps) {
  const categories = useAppState((s) => s.categories);

  const novelCategories = useMemo(
    () => categories.filter((c) => c.schema_slug === 'novel'),
    [categories]
  );

  const [targetCategoryId, setTargetCategoryId] = useState<number | null>(null);
  // null = all novel-schema categories except the target.
  const [sourceCategoryId, setSourceCategoryId] = useState<number | null>(null);
  const [count, setCount] = useState<number | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<ReclassifyResponse | null>(null);
  const [errorsExpanded, setErrorsExpanded] = useState(false);

  const refreshCount = useCallback(async () => {
    if (targetCategoryId == null) {
      setCount(null);
      return;
    }
    try {
      const c = await fileCountForCategoryReanalyze(
        targetCategoryId,
        sourceCategoryId
      );
      setCount(c);
    } catch (err) {
      console.error('Failed to count re-classify candidates:', err);
      setCount(null);
    }
  }, [targetCategoryId, sourceCategoryId]);

  useEffect(() => {
    void refreshCount();
  }, [refreshCount]);

  const handleRun = useCallback(async () => {
    if (targetCategoryId == null) return;
    setRunning(true);
    setResult(null);
    try {
      const res = await fileReanalyzeForCategory(targetCategoryId, sourceCategoryId);
      setResult(res);
      onAfterRun?.();
      void refreshCount();
    } catch (err) {
      // Surface single-shot failures (e.g. LLM not configured) the same
      // way the tags-reanalyze card does: synthesize a one-row result so
      // the message lands in the same panel.
      const message = err instanceof Error ? err.message : String(err);
      setResult({
        processed: 0,
        moved: 0,
        skipped: 0,
        failed: 0,
        errors: [
          {
            file_id: 0,
            display_name: 'Re-classify action failed',
            message,
          },
        ],
      });
    } finally {
      setRunning(false);
      setConfirmOpen(false);
    }
  }, [targetCategoryId, sourceCategoryId, onAfterRun, refreshCount]);

  const targetLabel = useMemo(() => {
    if (targetCategoryId == null) return null;
    return novelCategories.find((c) => c.id === targetCategoryId)?.name ?? null;
  }, [targetCategoryId, novelCategories]);

  const sourceLabel = useMemo(() => {
    if (sourceCategoryId == null) return 'All novel categories';
    return novelCategories.find((c) => c.id === sourceCategoryId)?.name ?? 'Unknown';
  }, [sourceCategoryId, novelCategories]);

  const canRun =
    !running && targetCategoryId != null && count != null && count > 0;

  return (
    <div className="rounded-lg border bg-background p-4 space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1 min-w-0">
          <h3 className="text-sm font-medium flex items-center gap-1.5">
            <TagIcon
              className="h-3.5 w-3.5 text-muted-foreground"
              aria-hidden="true"
            />
            Re-classify novels for category
            {count != null && (
              <span className="text-muted-foreground font-normal">· {count}</span>
            )}
          </h3>
          <p className="text-xs text-muted-foreground leading-relaxed">
            Runs the import-time LLM content extraction on every novel-schema
            file that&apos;s not already in the target category. Files whose
            LLM-picked category matches the target are moved into it (disk +
            DB). Useful for re-classifying novels across categories. Tags
            returned by the LLM are ignored on this path. Remote files
            without a local cache are skipped.
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          className="h-8 text-xs shrink-0"
          onClick={() => setConfirmOpen(true)}
          disabled={!canRun}
        >
          {running ? (
            <>
              <Loader2 className="h-3.5 w-3.5 mr-1 animate-spin" />
              Running…
            </>
          ) : (
            'Run'
          )}
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <Select
          value={targetCategoryId == null ? '' : String(targetCategoryId)}
          onValueChange={(v) => setTargetCategoryId(v === '' ? null : Number(v))}
        >
          <SelectTrigger className="h-8 w-auto text-xs gap-1.5">
            <span className="text-muted-foreground">Target</span>
            <SelectValue placeholder="pick a category…" />
          </SelectTrigger>
          <SelectContent>
            {novelCategories.map((c) => (
              <SelectItem key={c.id} value={String(c.id)} className="text-xs">
                {c.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Select
          value={sourceCategoryId == null ? 'all' : String(sourceCategoryId)}
          onValueChange={(v) =>
            setSourceCategoryId(v === 'all' ? null : Number(v))
          }
        >
          <SelectTrigger className="h-8 w-auto text-xs gap-1.5">
            <span className="text-muted-foreground">Source</span>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all" className="text-xs">
              All novel categories
            </SelectItem>
            {novelCategories
              .filter((c) => c.id !== targetCategoryId)
              .map((c) => (
                <SelectItem key={c.id} value={String(c.id)} className="text-xs">
                  {c.name}
                </SelectItem>
              ))}
          </SelectContent>
        </Select>
      </div>

      {result != null && !running && (
        <ReclassifyResultPanel
          result={result}
          expanded={errorsExpanded}
          onToggleExpand={() => setErrorsExpanded((v) => !v)}
        />
      )}

      <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              Re-classify {count ?? 0} files for &ldquo;{targetLabel}&rdquo;?
            </AlertDialogTitle>
            <AlertDialogDescription>
              Source: <span className="font-medium">{sourceLabel}</span>. This
              calls your configured LLM once per file. Files whose
              LLM-picked category matches &ldquo;{targetLabel}&rdquo; will be
              moved into that category on disk and in the DB. Files whose pick
              doesn&apos;t match stay where they are. At a few seconds per
              file the run can take minutes — wait for the result before
              closing the page.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={running}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => {
                e.preventDefault();
                void handleRun();
              }}
              disabled={running}
            >
              {running ? 'Running…' : 'Run'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

interface ReclassifyResultPanelProps {
  result: ReclassifyResponse;
  expanded: boolean;
  onToggleExpand: () => void;
}

function ReclassifyResultPanel({
  result,
  expanded,
  onToggleExpand,
}: ReclassifyResultPanelProps) {
  const summary =
    result.processed === 0
      ? 'No files matched — nothing to re-classify.'
      : `${result.moved} moved, ${result.skipped} left alone, ${result.failed} failed (out of ${result.processed}).`;
  return (
    <div className="border-t pt-3 space-y-2">
      <p className="text-xs text-foreground">{summary}</p>
      {result.errors.length > 0 && (
        <div className="space-y-1">
          <button
            type="button"
            onClick={onToggleExpand}
            className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground focus:outline-none focus:ring-2 focus:ring-ring rounded"
            aria-expanded={expanded}
          >
            {expanded ? (
              <ChevronDown className="h-3 w-3" />
            ) : (
              <ChevronRight className="h-3 w-3" />
            )}
            {result.errors.length}{' '}
            {result.errors.length === 1 ? 'issue' : 'issues'}
          </button>
          {expanded && (
            <ul className="space-y-1 pl-4 border-l text-[11px] text-muted-foreground">
              {result.errors.map((err, i) => (
                <ErrorRow key={`${err.file_id}-${i}`} err={err} />
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}

interface MergeCategoryCardProps {
  onAfterRun?: () => void;
}

/** Merges every file in a source category into a target category — on
 *  disk (folder rename + file moves) and in the DB (path + category_id
 *  rewrites). Source and target must share a `schema_slug` so the
 *  metadata layout stays compatible. When the merge is clean the source
 *  category row is deleted; when duplicates collide the source is left
 *  in place so the user can resolve them and re-run. Irreversible — the
 *  confirm dialog says so. */
function MergeCategoryCard({ onAfterRun }: MergeCategoryCardProps) {
  const categories = useAppState((s) => s.categories);

  const [sourceId, setSourceId] = useState<number | null>(null);
  const [targetId, setTargetId] = useState<number | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<CategoryMergeResponse | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const sourceCategory = useMemo(
    () => (sourceId == null ? null : categories.find((c) => c.id === sourceId) ?? null),
    [sourceId, categories]
  );

  // Target options are limited to categories that share the source's
  // schema_slug — the backend rejects INCOMPATIBLE_SCHEMAS, so filtering
  // here avoids a dead-end confirm dialog.
  const targetOptions = useMemo(() => {
    if (!sourceCategory) return [] as typeof categories;
    const slug = coerceSchemaSlug(sourceCategory.schema_slug);
    return categories.filter(
      (c) => c.id !== sourceCategory.id && coerceSchemaSlug(c.schema_slug) === slug
    );
  }, [sourceCategory, categories]);

  const sourceLabel = sourceCategory?.name ?? null;
  const targetLabel = useMemo(() => {
    if (targetId == null) return null;
    return categories.find((c) => c.id === targetId)?.name ?? null;
  }, [targetId, categories]);

  // Clear target whenever source changes — the previous pick may not be
  // schema-compatible with the new source.
  useEffect(() => {
    if (
      targetId != null &&
      !targetOptions.some((c) => c.id === targetId)
    ) {
      setTargetId(null);
    }
  }, [targetId, targetOptions]);

  const canRun =
    !running && sourceId != null && targetId != null && sourceId !== targetId;

  const handleRun = useCallback(async () => {
    if (sourceId == null || targetId == null) return;
    setRunning(true);
    setResult(null);
    setErrorMessage(null);
    try {
      const res = await categoryMerge(sourceId, targetId);
      setResult(res);
      onAfterRun?.();
      // Refresh the global category list — the source row may have been
      // deleted, and the sidebar / category-derived UIs read from this
      // store. `loadCategories` also reconciles the active selection.
      void loadCategories();
      // When the source category was deleted, drop the now-stale id from
      // local state so the picker doesn't keep it selected.
      if (res.deleted_source) {
        setSourceId(null);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setErrorMessage(message);
    } finally {
      setRunning(false);
      setConfirmOpen(false);
    }
  }, [sourceId, targetId, onAfterRun]);

  return (
    <div className="rounded-lg border bg-background p-4 space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1 min-w-0">
          <h3 className="text-sm font-medium flex items-center gap-1.5">
            <FolderInput
              className="h-3.5 w-3.5 text-muted-foreground"
              aria-hidden="true"
            />
            Merge category
          </h3>
          <p className="text-xs text-muted-foreground leading-relaxed">
            Moves every file from the source category into the target
            category — on disk (folder + files) and in the DB. Source and
            target must use the same schema. When every file moves cleanly,
            the source category is deleted. Files that collide with a same-
            named file at the target are skipped and reported; the source
            category stays put so you can resolve those by hand.
            Irreversible.
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          className="h-8 text-xs shrink-0"
          onClick={() => setConfirmOpen(true)}
          disabled={!canRun}
        >
          {running ? (
            <>
              <Loader2 className="h-3.5 w-3.5 mr-1 animate-spin" />
              Merging…
            </>
          ) : (
            'Merge'
          )}
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <Select
          value={sourceId == null ? '' : String(sourceId)}
          onValueChange={(v) => setSourceId(v === '' ? null : Number(v))}
        >
          <SelectTrigger className="h-8 w-auto text-xs gap-1.5">
            <span className="text-muted-foreground">Source</span>
            <SelectValue placeholder="pick a category…" />
          </SelectTrigger>
          <SelectContent>
            {categories.map((c) => (
              <SelectItem key={c.id} value={String(c.id)} className="text-xs">
                {c.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Select
          value={targetId == null ? '' : String(targetId)}
          onValueChange={(v) => setTargetId(v === '' ? null : Number(v))}
          disabled={sourceId == null}
        >
          <SelectTrigger className="h-8 w-auto text-xs gap-1.5">
            <span className="text-muted-foreground">Target</span>
            <SelectValue
              placeholder={
                sourceId == null ? 'pick a source first' : 'pick a category…'
              }
            />
          </SelectTrigger>
          <SelectContent>
            {targetOptions.map((c) => (
              <SelectItem key={c.id} value={String(c.id)} className="text-xs">
                {c.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {errorMessage != null && !running && (
        <div className="border-t pt-3">
          <p className="text-xs text-destructive">{errorMessage}</p>
        </div>
      )}

      {result != null && !running && <MergeResultPanel result={result} />}

      <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              Merge &ldquo;{sourceLabel}&rdquo; into &ldquo;{targetLabel}&rdquo;?
            </AlertDialogTitle>
            <AlertDialogDescription>
              Every file under &ldquo;{sourceLabel}&rdquo; will be moved into
              &ldquo;{targetLabel}&rdquo; on disk and in the database. When the
              move is clean, the &ldquo;{sourceLabel}&rdquo; category is
              deleted along with its folder. Files whose basename already
              exists at the target are skipped and reported for manual
              resolution. This cannot be undone — files are physically moved.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={running}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => {
                e.preventDefault();
                void handleRun();
              }}
              disabled={running}
            >
              {running ? 'Merging…' : 'Merge'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function MergeResultPanel({ result }: { result: CategoryMergeResponse }) {
  const summary =
    result.moved === 0 && result.skipped_duplicates.length === 0
      ? 'Nothing to merge — source category was empty.'
      : `${result.moved} moved · ${result.skipped_duplicates.length} skipped · source ${
          result.deleted_source ? 'deleted' : 'kept'
        }.`;
  return (
    <div className="border-t pt-3 space-y-2">
      <p className="text-xs text-foreground">{summary}</p>
      {result.skipped_duplicates.length > 0 && (
        <div className="space-y-1">
          <p className="text-[11px] text-muted-foreground">
            Skipped (same filename already at target):
          </p>
          <ul className="space-y-0.5 pl-4 border-l text-[11px] text-muted-foreground">
            {result.skipped_duplicates.map((name, i) => (
              <li key={`${name}-${i}`} className="break-all">
                {name}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
