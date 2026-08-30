import { useCallback, useEffect, useMemo, useState } from 'react';
import { Loader2, UserPlus } from 'lucide-react';
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
} from '@/lib/tauri';
import { useAppState } from '@/stores/appStore';

interface AssignAuthorCardProps {
  /** Called after a successful run so the parent can refresh derived
   *  state (unused-metadata section, author lists elsewhere). */
  onAfterRun?: () => void;
}

/** Bulk-assigns one existing author to every file in a chosen category
 *  (or library-wide) that currently has no author. Lives in cleanup's
 *  Author tools section — it touches many rows in one go and is
 *  irreversible via the UI. */
export function AssignAuthorCard({ onAfterRun }: AssignAuthorCardProps) {
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
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
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
