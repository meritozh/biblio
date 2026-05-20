import { useMemo, useState } from 'react';
import { ChevronDown, ChevronRight, ExternalLink, Trash2 } from 'lucide-react';
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
import { CardStatus } from '@/components/cards/CardStatus';
import { cacheOpen } from '@/lib/tauri';
import { useAppState } from '@/stores/appStore';
import type { Category, FileEntry } from '@/types';

/** "2 weeks ago" / "yesterday" / "5 months ago". Uses
 *  `Intl.RelativeTimeFormat` so no date library is needed. Returns "" on
 *  parse failure so the meta line collapses to its remaining pieces.
 *
 *  SQLite's CURRENT_TIMESTAMP ships `YYYY-MM-DD HH:MM:SS` in UTC with no
 *  zone marker; V8 parses that format as LOCAL time, which would offset
 *  every relative date by the user's UTC delta. Coerce to explicit ISO
 *  with a Z suffix before parsing. */
function relativeTime(iso: string): string {
  let s = iso.includes(' ') ? iso.replace(' ', 'T') : iso;
  if (!/Z$|[+-]\d{2}:?\d{2}$/.test(s)) s += 'Z';
  const ms = Date.parse(s);
  if (Number.isNaN(ms)) return '';
  const seconds = Math.round((ms - Date.now()) / 1000); // negative for past
  const abs = Math.abs(seconds);
  const f = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });
  if (abs < 60) return f.format(seconds, 'second');
  if (abs < 3600) return f.format(Math.round(seconds / 60), 'minute');
  if (abs < 86_400) return f.format(Math.round(seconds / 3600), 'hour');
  if (abs < 86_400 * 30) return f.format(Math.round(seconds / 86_400), 'day');
  if (abs < 86_400 * 365)
    return f.format(Math.round(seconds / (86_400 * 30)), 'month');
  return f.format(Math.round(seconds / (86_400 * 365)), 'year');
}

/** Last two path segments, prefixed with `…/`. Disambiguates rows that
 *  share a display_name but live in different folders. Splits on both
 *  `/` and `\\` so Windows paths render correctly too. */
function pathTail(path: string): string {
  const parts = path.split(/[/\\]+/).filter(Boolean);
  if (parts.length <= 1) return path;
  return `…/${parts.slice(-2).join('/')}`;
}

interface DuplicateGroupCardProps {
  /** Group label — the shared display-name prefix. */
  prefix: string;
  /** Hydrated rows; storage badge reads `storage_kind` + `local_cache_path`. */
  files: ReadonlyArray<FileEntry>;
  /** Default-expanded for small group counts. */
  defaultExpanded?: boolean;
  /** Per-row delete handler — runs the file through the worker queue so
   *  remote cleanup happens before the DB row drops. */
  onDeleteFile: (file: FileEntry) => void;
  /** Dismiss this group from the cleanup view for the current session. */
  onDismiss: () => void;
}

/** Collapsible card showing one group of similar-named files. Per-row
 *  delete uses an AlertDialog so the user can review which one they're
 *  removing; "Keep all" is session-only — no DB state. */
export function DuplicateGroupCard({
  prefix,
  files,
  defaultExpanded = false,
  onDeleteFile,
  onDismiss,
}: DuplicateGroupCardProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const [pendingFile, setPendingFile] = useState<FileEntry | null>(null);

  // Category name lookup for the meta line. Resolved once per render
  // instead of doing the linear find inside each row.
  const categories = useAppState((s) => s.categories);
  const categoriesById = useMemo(() => {
    const m = new Map<number, Category>();
    for (const c of categories) m.set(c.id, c);
    return m;
  }, [categories]);

  return (
    <div className="rounded-lg border bg-background overflow-hidden">
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="w-full flex items-center gap-2 px-4 py-3 text-left hover:bg-muted/40 transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-inset"
        aria-expanded={expanded}
      >
        {expanded ? (
          <ChevronDown className="h-4 w-4 text-muted-foreground shrink-0" aria-hidden="true" />
        ) : (
          <ChevronRight className="h-4 w-4 text-muted-foreground shrink-0" aria-hidden="true" />
        )}
        <span className="font-medium text-sm truncate" title={prefix}>
          {prefix}
        </span>
        <span className="text-xs text-muted-foreground shrink-0">
          · {files.length} files
        </span>
      </button>

      {expanded && (
        <div className="border-t">
          <ul className="divide-y">
            {files.map((file) => {
              // Build the meta line lazily per row. Order: category →
              // relative date → progress → path tail. Empty pieces drop
              // out so the line stays compact for sparse rows.
              const metaParts: string[] = [];
              const cat =
                file.category_id != null
                  ? categoriesById.get(file.category_id)?.name
                  : undefined;
              if (cat) metaParts.push(cat);
              const rel = relativeTime(file.created_at);
              if (rel) metaParts.push(rel);
              if (file.progress) metaParts.push(file.progress);
              metaParts.push(pathTail(file.path));
              const metaLine = metaParts.join(' · ');

              // Mirrors FileContextMenu's gate: a remote file with no
              // cached copy can't be opened locally; the user has to
              // download it first. Visible-but-disabled (with a title
              // hint) is clearer than hiding the button entirely —
              // keeps the row layout stable across rows in the group.
              const isRemote = file.storage_kind === 'remote';
              const hasLocalCopy = !isRemote || !!file.local_cache_path;

              return (
                <li
                  key={file.id}
                  className="flex items-start gap-3 px-4 py-2.5 text-sm"
                >
                  <CardStatus
                    storageKind={file.storage_kind}
                    isUploading={false}
                    hasLocalCache={!!file.local_cache_path}
                  />
                  <div className="flex-1 min-w-0 space-y-0.5">
                    <p
                      className="truncate leading-tight"
                      title={file.display_name}
                    >
                      {file.display_name}
                    </p>
                    <p
                      className="truncate text-[11px] text-muted-foreground leading-tight"
                      title={metaLine}
                    >
                      {metaLine}
                    </p>
                  </div>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7 text-muted-foreground hover:text-foreground shrink-0"
                    onClick={async () => {
                      try {
                        await cacheOpen(file.id);
                      } catch (error) {
                        console.error('Failed to open file:', error);
                      }
                    }}
                    disabled={!hasLocalCopy}
                    aria-label={`Open ${file.display_name}`}
                    title={
                      hasLocalCopy
                        ? 'Open file'
                        : 'Download first to open'
                    }
                  >
                    <ExternalLink className="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-7 w-7 text-muted-foreground hover:text-destructive shrink-0"
                    onClick={() => setPendingFile(file)}
                    aria-label={`Delete ${file.display_name}`}
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </Button>
                </li>
              );
            })}
          </ul>
          <div className="flex justify-end px-4 py-2 border-t bg-muted/20">
            <Button
              variant="ghost"
              size="sm"
              className="h-7 text-xs text-muted-foreground hover:text-foreground"
              onClick={onDismiss}
            >
              Keep all
            </Button>
          </div>
        </div>
      )}

      <AlertDialog
        open={pendingFile != null}
        onOpenChange={(open) => {
          if (!open) setPendingFile(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete this file?</AlertDialogTitle>
            <AlertDialogDescription>
              <span className="font-medium text-foreground break-all">
                {pendingFile?.display_name}
              </span>
              {' '}
              will be removed from the library. Remote copies are deleted first;
              the DB row drops only after that succeeds. This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => {
                e.preventDefault();
                if (pendingFile) onDeleteFile(pendingFile);
                setPendingFile(null);
              }}
            >
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

