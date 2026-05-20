import { X, Plus } from 'lucide-react';

interface SuggestedTagChipProps {
  name: string;
  onApprove: (name: string) => void;
  onDismiss: (name: string) => void;
  /** Noun used in the Approve tooltip — defaults to "tag" for backward
   *  compatibility. Authors pass "author". */
  noun?: string;
}

export function SuggestedTagChip({
  name,
  onApprove,
  onDismiss,
  noun = 'tag',
}: SuggestedTagChipProps) {
  return (
    <span className="inline-flex items-center gap-1 rounded-full border border-dashed border-primary/40 bg-primary/5 px-2.5 py-0.5 text-xs font-medium text-primary/70 transition-colors">
      <span>{name}</span>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          onApprove(name);
        }}
        className="rounded-full p-0.5 hover:bg-primary/10 transition-colors"
        title={`Add "${name}" as a new ${noun}`}
      >
        <Plus className="h-3 w-3" />
      </button>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          onDismiss(name);
        }}
        className="rounded-full p-0.5 hover:bg-destructive/10 transition-colors"
        title="Dismiss suggestion"
      >
        <X className="h-3 w-3 text-muted-foreground" />
      </button>
    </span>
  );
}
