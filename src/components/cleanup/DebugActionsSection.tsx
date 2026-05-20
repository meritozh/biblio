import { useCallback, useEffect, useState } from 'react';
import { ChevronDown, ChevronRight, Loader2, Sparkles } from 'lucide-react';
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
  fileCountNovelsMissingTags,
  fileReanalyzeMissingTags,
  type ReanalyzeError,
  type ReanalyzeResponse,
} from '@/lib/tauri';

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
