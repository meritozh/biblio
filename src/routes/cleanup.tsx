import { createFileRoute } from '@tanstack/react-router';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useStore } from '@tanstack/react-store';
import { VirtualList } from '@/components/VirtualList';
import { Loader2, Settings2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
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
import { useAppState } from '@/stores/appStore';
import { UnusedItemsSection } from '@/components/cleanup/UnusedItemsSection';
import { DebugActionsSection } from '@/components/cleanup/DebugActionsSection';
import { DuplicateGroupCard } from '@/components/cleanup/DuplicateGroupCard';
import {
  authorDeleteUnused,
  authorList,
  fileDuplicateGroups,
  listenTagAuthorChanges,
  tagDeleteUnused,
  tagList,
} from '@/lib/tauri';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { fileStore, hydrateFiles } from '@/stores/fileStore';
import { enqueueDelete } from '@/stores/remoteDeleteStore';
import type { Author, DuplicateGroup, FileEntry, TagWithUsage } from '@/types';

export const Route = createFileRoute('/cleanup')({
  component: CleanupPage,
});

const DEFAULT_MIN_PREFIX_CHARS = 3;
const DEFAULT_PREFIX_RATIO = 0.5;
/** Auto-expand all group cards when there are at most this many; otherwise
 *  start collapsed so the page doesn't open as a wall of rows. */
const AUTO_EXPAND_LIMIT = 5;

function CleanupPage() {
  const [unusedTags, setUnusedTags] = useState<TagWithUsage[]>([]);
  const [unusedAuthors, setUnusedAuthors] = useState<(Author & { usageCount: number })[]>([]);
  const [groups, setGroups] = useState<DuplicateGroup[]>([]);
  const [loadingMeta, setLoadingMeta] = useState(true);
  const [loadingGroups, setLoadingGroups] = useState(true);
  const [bulkBusy, setBulkBusy] = useState(false);

  // Sensitivity controls — start at backend defaults; popover-edited.
  const [minPrefixChars, setMinPrefixChars] = useState(DEFAULT_MIN_PREFIX_CHARS);
  const [prefixRatio, setPrefixRatio] = useState(DEFAULT_PREFIX_RATIO);

  // Category scope for the duplicate scan. null = all categories (default).
  // Numeric id = scan only files in that category. Sourced from appStore
  // so we don't pay a second IPC; the root route already loads them.
  const categories = useAppState((s) => s.categories);
  const [categoryFilter, setCategoryFilter] = useState<number | null>(null);

  // Session-only dismissed groups, keyed by prefix string. "Keep all" on a
  // group adds its prefix here; refreshing the page wipes the set, which is
  // the intended escape hatch — see the plan note on no persistent ignores.
  const [dismissedPrefixes, setDismissedPrefixes] = useState<Set<string>>(new Set());

  // Subscribe to the file-store so deleted rows automatically drop out of
  // their groups. The delete worker calls `removeFile(id)` on success;
  // failures leave the row in `byId`, so the group stays visible — that
  // mirrors what would happen on a refresh, without the optimistic-then-
  // -revert flicker.
  const byId = useStore(fileStore, (s) => s.byId);

  const fetchUnused = useCallback(async () => {
    setLoadingMeta(true);
    try {
      const [tagsRes, authorsRes] = await Promise.all([
        tagList({ includeUsage: true }),
        authorList({ includeUsage: true }),
      ]);
      setUnusedTags(tagsRes.tags.filter((t) => t.usageCount === 0));
      setUnusedAuthors(authorsRes.authors.filter((a) => a.usageCount === 0));
    } finally {
      setLoadingMeta(false);
    }
  }, []);

  const fetchGroups = useCallback(async () => {
    setLoadingGroups(true);
    try {
      const result = await fileDuplicateGroups({
        minPrefixChars,
        prefixRatio,
        categoryId: categoryFilter,
      });
      // Push the hydrated rows into fileStore so worker-driven deletes
      // (which call removeFile) reactively drop them from `visibleGroups`
      // without this page having to manage its own optimistic state.
      hydrateFiles(result.flatMap((g) => g.files));
      setGroups(result);
      // New scope or thresholds invalidate the previous dismissal set —
      // different groups, different identities.
      setDismissedPrefixes(new Set());
    } finally {
      setLoadingGroups(false);
    }
  }, [minPrefixChars, prefixRatio, categoryFilter]);

  useEffect(() => {
    void fetchUnused();
  }, [fetchUnused]);
  useEffect(() => {
    void fetchGroups();
  }, [fetchGroups]);

  // Bulk delete fires a single tag-deleted / author-deleted event (sentinel
  // id 0). Subscribe here so the unused-items section refreshes the same
  // way an edit from /tags or /authors would — one source of truth, no
  // manual refetch after the bulk action.
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;
    listenTagAuthorChanges(() => {
      void fetchUnused();
    })
      .then((u) => {
        if (cancelled) u();
        else unlisten = u;
      })
      .catch((err) => {
        console.error('Failed to subscribe to tag/author changes:', err);
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [fetchUnused]);

  const handleDeleteUnusedTags = useCallback(async () => {
    setBulkBusy(true);
    try {
      await tagDeleteUnused();
    } finally {
      setBulkBusy(false);
    }
  }, []);

  const handleDeleteUnusedAuthors = useCallback(async () => {
    setBulkBusy(true);
    try {
      await authorDeleteUnused();
    } finally {
      setBulkBusy(false);
    }
  }, []);

  const handleDeleteFile = useCallback((file: FileEntry) => {
    const fileNames = new Map([[file.id, file.display_name]]);
    void enqueueDelete([file.id], fileNames);
    // No local state mutation — the worker calls removeFile(id) on
    // success; the byId subscription above re-derives `visibleGroups`
    // to drop the row. On failure, the row stays and we don't lie to
    // the user.
  }, []);

  const handleDismissGroup = useCallback((prefix: string) => {
    setDismissedPrefixes((prev) => {
      const next = new Set(prev);
      next.add(prefix);
      return next;
    });
  }, []);

  const visibleGroups = useMemo(
    () =>
      groups
        // Drop files no longer in fileStore (delete-worker success).
        .map((g) => ({
          ...g,
          files: g.files.filter((f) => byId.has(f.id)),
        }))
        // Drop groups that lost too many files to be a "group" anymore.
        .filter((g) => g.files.length >= 2 && !dismissedPrefixes.has(g.prefix)),
    [groups, byId, dismissedPrefixes]
  );

  const expandByDefault = visibleGroups.length <= AUTO_EXPAND_LIMIT;
  const hasUnused = unusedTags.length > 0 || unusedAuthors.length > 0;

  return (
    <>
      <div
        className="flex items-end justify-between px-8 pt-14 pb-5 border-b border-border"
        data-tauri-drag-region
      >
        <div className="min-w-0">
          <div className="text-3xl text-foreground">Cleanup</div>
          <p className="text-sm text-muted-foreground mt-1">
            Find unused tags / authors and duplicate file names.
          </p>
        </div>
      </div>

      <div className="flex-1 overflow-auto px-8 py-6 space-y-8">
        {/* ── Section A: unused metadata ──────────────────────────── */}
        <section className="space-y-4">
          <h2 className="text-sm uppercase tracking-wide text-muted-foreground font-medium">
            Unused metadata
          </h2>
          {loadingMeta ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              Scanning…
            </div>
          ) : !hasUnused ? (
            <p className="text-sm text-muted-foreground font-serif-italic">
              No unused tags or authors — everything is in use.
            </p>
          ) : (
            <div className="space-y-5">
              <UnusedItemsSection
                title="Unused tags"
                items={unusedTags.map((t) => ({ id: t.id, name: t.name }))}
                chipPrefix="#"
                busy={bulkBusy}
                onDeleteAll={handleDeleteUnusedTags}
              />
              <UnusedItemsSection
                title="Unused authors"
                items={unusedAuthors.map((a) => ({ id: a.id, name: a.name }))}
                busy={bulkBusy}
                onDeleteAll={handleDeleteUnusedAuthors}
              />
            </div>
          )}
        </section>

        {/* ── Section: debug actions (LLM re-analyze, etc.) ──────────── */}
        <DebugActionsSection onAfterRun={fetchUnused} />

        {/* ── Section B: duplicate-name candidates ────────────────── */}
        <section className="space-y-4">
          <div className="flex items-center justify-between gap-3 flex-wrap">
            <h2 className="text-sm uppercase tracking-wide text-muted-foreground font-medium">
              Similar names
              {!loadingGroups && (
                <span className="ml-2 font-normal normal-case text-muted-foreground/80">
                  · {visibleGroups.length}{' '}
                  {visibleGroups.length === 1 ? 'group' : 'groups'}
                </span>
              )}
            </h2>
            <div className="flex items-center gap-2">
              <Select
                value={categoryFilter == null ? 'all' : String(categoryFilter)}
                onValueChange={(v) =>
                  setCategoryFilter(v === 'all' ? null : Number(v))
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
              <SensitivityControl
                minPrefixChars={minPrefixChars}
                prefixRatio={prefixRatio}
                onApply={(chars, ratio) => {
                  setMinPrefixChars(chars);
                  setPrefixRatio(ratio);
                }}
              />
            </div>
          </div>

          {loadingGroups ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
              Looking for similar file names…
            </div>
          ) : visibleGroups.length === 0 ? (
            <p className="text-sm text-muted-foreground font-serif-italic">
              {groups.length === 0
                ? "No groups found. Looking only at shared name beginnings — files whose names start the same."
                : 'All groups dismissed for this session.'}
            </p>
          ) : (
            <VirtualList<DuplicateGroup>
              items={visibleGroups}
              getKey={(g) => g.prefix}
              estimateSize={64}
              measureElement
              overscan={4}
              className="max-h-[70vh] overflow-auto rounded-lg"
              renderItem={(g) => (
                <div style={{ paddingBottom: 8 }}>
                  <DuplicateGroupCard
                    prefix={g.prefix}
                    files={g.files}
                    defaultExpanded={expandByDefault}
                    onDeleteFile={handleDeleteFile}
                    onDismiss={() => handleDismissGroup(g.prefix)}
                  />
                </div>
              )}
            />
          )}
        </section>
      </div>
    </>
  );
}

interface SensitivityControlProps {
  minPrefixChars: number;
  prefixRatio: number;
  onApply: (chars: number, ratio: number) => void;
}

/** Hidden-by-default sensitivity tuner. Defaults match the backend so the
 *  initial open state matches the current grouping; "Apply" pushes the
 *  values to the parent which re-runs the fetch. */
function SensitivityControl({
  minPrefixChars,
  prefixRatio,
  onApply,
}: SensitivityControlProps) {
  const [open, setOpen] = useState(false);
  const [draftChars, setDraftChars] = useState(minPrefixChars);
  const [draftRatio, setDraftRatio] = useState(prefixRatio);

  // Re-seed when the popover opens so the inputs reflect the active values.
  useEffect(() => {
    if (open) {
      setDraftChars(minPrefixChars);
      setDraftRatio(prefixRatio);
    }
  }, [open, minPrefixChars, prefixRatio]);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button variant="outline" size="sm" className="h-8 text-xs gap-1.5">
          <Settings2 className="h-3.5 w-3.5" />
          Sensitivity
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" sideOffset={6} className="w-72 p-4 space-y-3">
        <div className="space-y-1.5">
          <Label htmlFor="min-prefix-chars" className="text-xs">
            Min shared prefix (chars)
          </Label>
          <Input
            id="min-prefix-chars"
            type="number"
            min={1}
            max={50}
            value={draftChars}
            onChange={(e) => {
              const v = Number(e.target.value);
              if (!Number.isNaN(v)) setDraftChars(Math.max(1, Math.min(50, v)));
            }}
            className="h-8 text-xs"
          />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="prefix-ratio" className="text-xs">
            Prefix ratio (0–1)
          </Label>
          <Input
            id="prefix-ratio"
            type="number"
            min={0}
            max={1}
            step={0.05}
            value={draftRatio}
            onChange={(e) => {
              const v = Number(e.target.value);
              if (!Number.isNaN(v)) setDraftRatio(Math.max(0, Math.min(1, v)));
            }}
            className="h-8 text-xs"
          />
          <p className="text-[11px] text-muted-foreground">
            Higher = stricter. {Math.round(prefixRatio * 100)}% means the shared
            prefix must be at least that fraction of the shorter file name.
          </p>
        </div>
        <div className="flex justify-end gap-2 pt-1">
          <Button
            variant="ghost"
            size="sm"
            className="h-7 text-xs"
            onClick={() => setOpen(false)}
          >
            Cancel
          </Button>
          <Button
            size="sm"
            className="h-7 text-xs"
            onClick={() => {
              onApply(draftChars, draftRatio);
              setOpen(false);
            }}
          >
            Apply
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
