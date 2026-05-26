import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  ChevronDown,
  ChevronRight,
  ImageIcon,
  Loader2,
  Sparkles,
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
  fileAssignAuthorToAuthorless,
  fileCountAuthorlessInCategory,
  fileCountComicsMissingCovers,
  fileCountNovelsMissingTags,
  fileReanalyzeMissingTags,
  fileRegenerateMissingCovers,
  type ReanalyzeError,
  type ReanalyzeResponse,
  type RegenerateCoversResponse,
} from '@/lib/tauri';
import { useAppState } from '@/stores/appStore';

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

      <RegenerateCoversCard onAfterRun={onAfterRun} />
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

interface RegenerateCoversCardProps {
  onAfterRun?: () => void;
}

/** Recovers comic-schema files whose `covers` row is missing — primarily
 *  the population dropped by the pre-2936908 cancel/clear race, but also
 *  any future case where a cover failed to write at import time. Calls
 *  `archive::pick_first_cover` (basename heuristic, no LLM) on each
 *  archive and writes the compressed result. Idempotent; safe to re-run. */
function RegenerateCoversCard({ onAfterRun }: RegenerateCoversCardProps) {
  const [count, setCount] = useState<number | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<RegenerateCoversResponse | null>(null);
  const [errorsExpanded, setErrorsExpanded] = useState(false);

  const refreshCount = useCallback(async () => {
    try {
      const c = await fileCountComicsMissingCovers();
      setCount(c);
    } catch (err) {
      console.error('Failed to count comics missing covers:', err);
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
      const res = await fileRegenerateMissingCovers();
      setResult(res);
      onAfterRun?.();
      void refreshCount();
    } catch (err) {
      // Surface single-shot failures (e.g. storage path not configured)
      // through the same panel the per-file errors use.
      const message = err instanceof Error ? err.message : String(err);
      setResult({
        processed: 0,
        regenerated: 0,
        skipped: 0,
        failed: 0,
        errors: [
          {
            file_id: 0,
            display_name: 'Regenerate action failed',
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
    <div className="rounded-lg border bg-background p-4 space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1 min-w-0">
          <h3 className="text-sm font-medium flex items-center gap-1.5">
            <ImageIcon
              className="h-3.5 w-3.5 text-muted-foreground"
              aria-hidden="true"
            />
            Regenerate missing comic covers
            {count != null && (
              <span className="text-muted-foreground font-normal">· {count}</span>
            )}
          </h3>
          <p className="text-xs text-muted-foreground leading-relaxed">
            Re-extracts the cover for every comic-schema file that has no
            cover stored. Runs the same cover-picking pipeline a fresh
            import would: baseline pick → LLM candidate ranking → vision
            verification → compress. Falls back to the basename heuristic
            when the LLM is disabled. Remote files without a local cache
            are skipped; download them first, then re-run.
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
        <RegenerateCoversResultPanel
          result={result}
          expanded={errorsExpanded}
          onToggleExpand={() => setErrorsExpanded((v) => !v)}
        />
      )}

      <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              Regenerate covers for {count ?? 0} files?
            </AlertDialogTitle>
            <AlertDialogDescription>
              Each file is opened, scanned for image entries, sent through
              the cover-picking LLM chain (when configured), and re-compressed.
              At a few seconds per file with LLM calls, the run can take many
              minutes for large libraries. The window stays open during the
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
    </div>
  );
}

interface RegenerateCoversResultPanelProps {
  result: RegenerateCoversResponse;
  expanded: boolean;
  onToggleExpand: () => void;
}

function RegenerateCoversResultPanel({
  result,
  expanded,
  onToggleExpand,
}: RegenerateCoversResultPanelProps) {
  const summary =
    result.processed === 0
      ? 'No comics with missing covers — nothing to do.'
      : `${result.regenerated} regenerated, ${result.skipped} skipped, ${result.failed} failed (out of ${result.processed}).`;
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
