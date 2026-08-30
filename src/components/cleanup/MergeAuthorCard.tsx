import { useCallback, useState } from 'react';
import { ArrowRight, Loader2, UserX } from 'lucide-react';
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
  PaginatedPicker,
  type PickerPage,
} from '@/components/PaginatedPicker';
import { authorList } from '@/lib/tauri';
import { authorMerge } from '@/lib/authorBridge';

interface MergeAuthorCardProps {
  /** Called after a successful merge so the parent can refresh derived
   *  state (unused-metadata section — the source author is deleted). */
  onAfterRun?: () => void;
}

interface PickedAuthor {
  id: number;
  name: string;
  /** Files currently linked to this author; null while loading. */
  usage: number | null;
}

/** Merge one author into another — the fix for typos and duplicates
 *  ("Tolkien, J.R.R." vs "J.R.R. Tolkien"). Every file linked to the
 *  source is relinked to the target; the source row is deleted. */
export function MergeAuthorCard({ onAfterRun }: MergeAuthorCardProps) {
  const [from, setFrom] = useState<PickedAuthor | null>(null);
  const [to, setTo] = useState<PickedAuthor | null>(null);
  const [fromOpen, setFromOpen] = useState(false);
  const [toOpen, setToOpen] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<{ moved: number } | null>(null);
  const [resultError, setResultError] = useState<string | null>(null);

  // Single-pick fetcher shared by both popovers — the merge operates on
  // existing authors only, so the inline-create row stays hidden.
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

  // The picker's item shape carries no usage count, so look it up after
  // selection with an exact id filter over a usage-enabled query.
  const pickWithUsage = useCallback(async (id: number, name: string) => {
    const picked: PickedAuthor = { id, name, usage: null };
    try {
      const { authors } = await authorList({ includeUsage: true, nameQuery: name });
      picked.usage = authors.find((a) => a.id === id)?.usageCount ?? 0;
    } catch {
      picked.usage = 0;
    }
    return picked;
  }, []);

  const handlePickFrom = useCallback(
    async (id: number, item: { name: string }) => {
      setFromOpen(false);
      setFrom(await pickWithUsage(id, item.name));
      setResult(null);
      setResultError(null);
    },
    [pickWithUsage]
  );

  const handlePickTo = useCallback(
    async (id: number, item: { name: string }) => {
      setToOpen(false);
      setTo(await pickWithUsage(id, item.name));
      setResult(null);
      setResultError(null);
    },
    [pickWithUsage]
  );

  const handleRun = useCallback(async () => {
    if (!from || !to) return;
    setRunning(true);
    setResult(null);
    setResultError(null);
    try {
      const res = await authorMerge(from.id, to.id);
      setResult(res);
      onAfterRun?.();
      // The source row is gone — clear it so the card is ready for the
      // next merge; keep the target for chained cleanups.
      setFrom(null);
    } catch (err) {
      setResultError(err instanceof Error ? err.message : String(err));
    } finally {
      setRunning(false);
      setConfirmOpen(false);
    }
  }, [from, to, onAfterRun]);

  const sameAuthor = from != null && to != null && from.id === to.id;
  const canRun = !running && from != null && to != null && !sameAuthor;

  return (
    <div className="rounded-lg border bg-background p-4 space-y-3">
      <div className="flex items-start justify-between gap-3">
        <div className="space-y-1 min-w-0">
          <h3 className="text-sm font-medium flex items-center gap-1.5">
            <UserX className="h-3.5 w-3.5 text-muted-foreground" aria-hidden="true" />
            Merge one author into another
          </h3>
          <p className="text-xs text-muted-foreground leading-relaxed">
            Relinks every file from the source author to the target, then
            deletes the source. For fixing typos and duplicate spellings.
            Single transaction, no undo.
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
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              Running…
            </>
          ) : (
            'Merge'
          )}
        </Button>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <Popover open={fromOpen} onOpenChange={setFromOpen}>
          <PopoverTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              className="h-8 text-xs gap-1.5 font-normal"
            >
              <span className="text-muted-foreground">From</span>
              <span className={from ? '' : 'text-muted-foreground'}>
                {from
                  ? `${from.name}${from.usage != null ? ` (${from.usage})` : ''}`
                  : 'pick an author…'}
              </span>
            </Button>
          </PopoverTrigger>
          <PopoverContent align="start" sideOffset={4} className="p-0">
            <PaginatedPicker
              mode="single"
              selectedIds={from ? [from.id] : []}
              fetcher={authorFetcher}
              onSelect={(id, item) => void handlePickFrom(id, item)}
              searchPlaceholder="Search authors…"
              emptyLabel="No authors defined"
              noMatchLabel="No matching authors"
            />
          </PopoverContent>
        </Popover>

        <ArrowRight className="h-3.5 w-3.5 text-muted-foreground" aria-hidden="true" />

        <Popover open={toOpen} onOpenChange={setToOpen}>
          <PopoverTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              className="h-8 text-xs gap-1.5 font-normal"
            >
              <span className="text-muted-foreground">To</span>
              <span className={to ? '' : 'text-muted-foreground'}>
                {to
                  ? `${to.name}${to.usage != null ? ` (${to.usage})` : ''}`
                  : 'pick an author…'}
              </span>
            </Button>
          </PopoverTrigger>
          <PopoverContent align="start" sideOffset={4} className="p-0">
            <PaginatedPicker
              mode="single"
              selectedIds={to ? [to.id] : []}
              fetcher={authorFetcher}
              onSelect={(id, item) => void handlePickTo(id, item)}
              searchPlaceholder="Search authors…"
              emptyLabel="No authors defined"
              noMatchLabel="No matching authors"
            />
          </PopoverContent>
        </Popover>

        {sameAuthor && (
          <span className="text-xs text-destructive">
            Source and target are the same author.
          </span>
        )}
      </div>

      {result != null && !running && (
        <div className="border-t pt-3">
          <p className="text-xs text-foreground">
            {result.moved === 0
              ? `Nothing to relink — no files were solely linked to the source. Merged into ${to?.name}.`
              : `Relinked ${result.moved} ${result.moved === 1 ? 'file' : 'files'} to ${to?.name} and deleted the source.`}
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
              Merge &ldquo;{from?.name}&rdquo; into &ldquo;{to?.name}&rdquo;?
            </AlertDialogTitle>
            <AlertDialogDescription>
              {from?.usage ?? 0} {from?.usage === 1 ? 'file' : 'files'} will be
              relinked to <span className="font-medium">{to?.name}</span>, then{' '}
              <span className="font-medium">{from?.name}</span> is deleted.
              Files already linked to both keep only the target. This
              can&apos;t be undone.
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
              {running ? 'Running…' : 'Merge'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
