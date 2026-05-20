import { useState } from 'react';
import { Loader2, Trash2 } from 'lucide-react';
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

interface ChipItem {
  id: number;
  name: string;
}

interface UnusedItemsSectionProps {
  /** Heading for the section ("Unused tags" / "Unused authors"). */
  title: string;
  /** Chips to preview. Filter upstream — this component renders whatever it
   *  receives. */
  items: ReadonlyArray<ChipItem>;
  /** How many chips to show before collapsing to a "+N more" tail. */
  previewLimit?: number;
  /** Optional decoration shown inside each chip — '#' for tags, nothing
   *  for authors. */
  chipPrefix?: string;
  /** Disable the action when the listener is mid-fetch. */
  busy?: boolean;
  /** Run the bulk delete. Resolves after the backend confirms. */
  onDeleteAll: () => Promise<void>;
}

/** Compact list of zero-usage tags or authors with a one-click "Delete all"
 *  action. Renders nothing when items is empty — the parent decides whether
 *  to show an empty-state line. */
export function UnusedItemsSection({
  title,
  items,
  previewLimit = 12,
  chipPrefix = '',
  busy = false,
  onDeleteAll,
}: UnusedItemsSectionProps) {
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [deleting, setDeleting] = useState(false);

  const handleConfirm = async () => {
    setDeleting(true);
    try {
      await onDeleteAll();
    } finally {
      setDeleting(false);
      setConfirmOpen(false);
    }
  };

  if (items.length === 0) return null;

  const preview = items.slice(0, previewLimit);
  const overflow = items.length - preview.length;

  return (
    <div className="space-y-2">
      <div className="flex items-baseline justify-between gap-3">
        <h3 className="text-sm font-medium text-foreground">
          {title}{' '}
          <span className="text-muted-foreground font-normal">· {items.length}</span>
        </h3>
        <Button
          variant="outline"
          size="sm"
          className="h-8 text-xs"
          onClick={() => setConfirmOpen(true)}
          disabled={busy || deleting}
        >
          {deleting ? (
            <Loader2 className="h-3.5 w-3.5 mr-1 animate-spin" />
          ) : (
            <Trash2 className="h-3.5 w-3.5 mr-1" />
          )}
          Delete all {items.length}
        </Button>
      </div>
      <div className="flex flex-wrap gap-1.5">
        {preview.map((it) => (
          <span
            key={it.id}
            className="inline-flex items-center rounded-full border bg-secondary/40 px-2.5 py-0.5 text-[11px] text-muted-foreground"
          >
            {chipPrefix}
            {it.name}
          </span>
        ))}
        {overflow > 0 && (
          <span className="inline-flex items-center px-2 py-0.5 text-[11px] text-muted-foreground/80 font-serif-italic">
            +{overflow} more
          </span>
        )}
      </div>

      <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete {items.length} {title.toLowerCase()}?</AlertDialogTitle>
            <AlertDialogDescription>
              This removes them from the library. Files that previously referenced
              them are untouched — only the join rows are dropped. This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleting}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => {
                e.preventDefault();
                void handleConfirm();
              }}
              disabled={deleting}
            >
              {deleting ? 'Deleting…' : 'Delete all'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
