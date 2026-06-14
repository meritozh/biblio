import { useId } from 'react';
import { AlertTriangle } from 'lucide-react';
import {
  CoverPlaceholder,
  ExistingCoverPreview,
  InlineCoverPreview,
  StagedCoverPreview,
} from '@/components/CoverPreview';
import type { CategorySchema } from '@/lib/categorySchema';
import type { DuplicateInfo, DuplicateAction } from '@/types';

interface DuplicateWarningProps {
  duplicateInfo: DuplicateInfo;
  /** Schema of the file being imported. Drives which compare rows
   *  render — novel keeps Name/Progress/Size, comic swaps Progress for
   *  a side-by-side Cover row. The dedup matcher only fires within the
   *  same category, so both sides share this schema. */
  schema: CategorySchema;
  /** Current display name on the new file (typically
   *  `item.formValues.display_name` — the post-edit value, so what the
   *  user sees here matches what will actually land on Replace). */
  newDisplayName: string;
  /** Author names attached to the new file, resolved from
   *  `item.formValues.author_ids` against the parent's authors snapshot.
   *  Empty array renders as "None" so an empty-vs-empty pair shows
   *  muted matching values rather than blank cells. */
  newAuthorNames: string[];
  /** Current progress value on the new file (form value, mirroring
   *  display_name's source). Only consulted on novel-schema rows. */
  newProgress: string | null;
  /** New-side cover sources, mirroring the form's tri-state. Priority:
   *  inline `newCoverData` (user just uploaded) → `newStagedCoverPath`
   *  (pipeline staged into PreparedCoverCache) → empty placeholder.
   *  Only consulted on comic-schema rows. */
  newCoverData?: string;
  newCoverMimeType?: string;
  newStagedCoverPath?: string;
  selectedAction: DuplicateAction | null;
  onActionChange: (action: DuplicateAction) => void;
}

const ACTION_LABELS: Record<DuplicateAction, { label: string; description: string }> = {
  Replace: {
    label: 'Replace existing',
    description: 'Delete the existing file and import this one',
  },
  Delete: {
    label: 'Delete',
    description: 'Do not import — move this file to the system Trash',
  },
  ImportAnyway: {
    label: 'Import anyway',
    description: 'Keep both files',
  },
};

/** Render a byte count as a human-readable string. KB/MB/GB use 1024,
 *  one decimal past the unit transition (so 1.4 MB rather than
 *  1.40234 MB). Returns "—" placeholder for null so the comparison rows
 *  visually distinguish "couldn't resolve" from "0 B". */
function formatBytes(bytes: number | null): string {
  if (bytes == null) return '—';
  if (bytes < 1024) return `${bytes} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let n = bytes / 1024;
  let i = 0;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i += 1;
  }
  return `${n.toFixed(1)} ${units[i]}`;
}

interface CompareRowProps {
  label: string;
  existing: string;
  next: string;
}

/** One row of the side-by-side compare table. When the two strings
 *  differ, both values get bolded so the eye lands on the deltas. The
 *  unchanged baseline stays muted-weight so a panel of mostly-identical
 *  fields doesn't shout. */
function CompareRow({ label, existing, next }: CompareRowProps) {
  const differs = existing !== next;
  const valueClass = differs ? 'font-semibold text-foreground' : 'text-foreground/80';
  return (
    <>
      <div className="text-muted-foreground">{label}</div>
      <div className={`truncate ${valueClass}`} title={existing}>{existing}</div>
      <div className={`truncate ${valueClass}`} title={next}>{next}</div>
    </>
  );
}

/** Visual cover row — two thumbnails side by side instead of text values.
 *  Sits inside the same 3-column grid as `CompareRow` so the label
 *  column aligns with the text rows above/below it. */
function CoverCompareRow({
  existingFileId,
  newCoverData,
  newCoverMimeType,
  newStagedCoverPath,
}: {
  existingFileId: number;
  newCoverData?: string;
  newCoverMimeType?: string;
  newStagedCoverPath?: string;
}) {
  // New-side priority: inline blob → staged path → placeholder. Mirrors
  // the form's cover-field rendering exactly so the dupe panel and the
  // form below it never disagree about what the new cover looks like.
  const newCell = newCoverData ? (
    <InlineCoverPreview coverData={newCoverData} coverMimeType={newCoverMimeType} />
  ) : newStagedCoverPath ? (
    <StagedCoverPreview stagedPath={newStagedCoverPath} />
  ) : (
    <CoverPlaceholder />
  );
  return (
    <>
      <div className="text-muted-foreground self-start pt-1">Cover</div>
      <div className="self-start">
        <ExistingCoverPreview fileId={existingFileId} />
      </div>
      <div className="self-start">{newCell}</div>
    </>
  );
}

/** Render an author list as a single comma-joined string. Returns
 *  "None" for empty so an empty-vs-empty pair stays visually balanced
 *  and the differs-bolding stays correct (both render the same text). */
function formatAuthors(names: ReadonlyArray<string>): string {
  return names.length === 0 ? 'None' : names.join(', ');
}

export function DuplicateWarning({
  duplicateInfo,
  schema,
  newDisplayName,
  newAuthorNames,
  newProgress,
  newCoverData,
  newCoverMimeType,
  newStagedCoverPath,
  selectedAction,
  onActionChange,
}: DuplicateWarningProps) {
  const radioGroupName = `duplicate-action-${useId()}`;
  const existingName = duplicateInfo.existing_display_name;
  const existingAuthors = formatAuthors(duplicateInfo.existing_author_names);
  const nextAuthors = formatAuthors(newAuthorNames);
  const existingSize = formatBytes(duplicateInfo.existing_size);
  const nextSize = formatBytes(duplicateInfo.new_size);

  // Schema-routed middle row. Novel schema (covering every novel-schema
  // category) keeps the Progress text row; comic and galgame schemas store
  // real cover art, so they render the Cover visual row instead. Anything
  // else falls through to no middle row — safer than guessing.
  const middleRow =
    schema.slug === 'novel' ? (
      <CompareRow
        label="Progress"
        existing={duplicateInfo.existing_progress ?? 'None'}
        next={newProgress ?? 'None'}
      />
    ) : schema.slug === 'comic' || schema.slug === 'galgame' ? (
      <CoverCompareRow
        existingFileId={duplicateInfo.existing_file_id}
        newCoverData={newCoverData}
        newCoverMimeType={newCoverMimeType}
        newStagedCoverPath={newStagedCoverPath}
      />
    ) : null;

  return (
    <div
      className="rounded-lg border border-amber-200 bg-amber-50/50 p-3 space-y-3"
      onClick={(e) => e.stopPropagation()}
    >
      <div className="flex items-center gap-2 text-amber-700">
        <AlertTriangle className="h-4 w-4 shrink-0" />
        <span className="text-sm font-medium">
          Duplicate of &ldquo;{existingName}&rdquo;
        </span>
      </div>

      {/* Side-by-side comparison. CSS Grid with three columns: label /
          existing value / new value. Field rows live as siblings inside
          one grid so the column widths line up across rows without
          per-row width plumbing. */}
      <div className="grid grid-cols-[auto_1fr_1fr] gap-x-3 gap-y-1.5 text-xs">
        <div />
        <div className="text-muted-foreground font-medium">Existing</div>
        <div className="text-muted-foreground font-medium">New</div>
        <CompareRow label="Name" existing={existingName} next={newDisplayName} />
        {/* Authors row sits above the schema-specific row because both
            schemas (novel + comic) have author in their formFields — it
            applies to every dupe regardless of slug. */}
        <CompareRow label="Authors" existing={existingAuthors} next={nextAuthors} />
        {middleRow}
        <CompareRow label="Size" existing={existingSize} next={nextSize} />
      </div>

      {/* Action selection */}
      <div className="space-y-2">
        {(Object.keys(ACTION_LABELS) as DuplicateAction[]).map((action) => (
          <label
            key={action}
            className="flex items-start gap-2 cursor-pointer"
          >
            <input
              type="radio"
              name={radioGroupName}
              checked={selectedAction === action}
              onChange={() => onActionChange(action)}
              className="mt-0.5 accent-primary"
            />
            <div>
              <span className="text-sm font-medium block">
                {ACTION_LABELS[action].label}
              </span>
              <span className="text-xs text-muted-foreground block">
                {ACTION_LABELS[action].description}
              </span>
            </div>
          </label>
        ))}
      </div>
    </div>
  );
}
