import { useCallback, useEffect, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card, CardContent } from '@/components/ui/card';
import { TabsContent } from '@/components/ui/tabs';
import { Badge } from '@/components/ui/badge';
import {
  DynamicMetadataForm,
  type DynamicMetadataFormValues,
} from '@/components/DynamicMetadataForm';
import { SuggestedTagChip } from '@/components/SuggestedTagChip';
import { DuplicateWarning } from '@/components/DuplicateWarning';
import { vndbFetchCover, vndbSearch, type VndbCandidate } from '@/lib/tauri';
import { REGISTRY, defaultSchema, schemaForCategoryId, schemaForPath } from '@/lib/categorySchema';
import {
  AlertCircle,
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  ChevronUp,
  FileText,
  FolderArchive,
  Loader2,
} from 'lucide-react';
import type { Author, Category, DuplicateAction, Tag } from '@/types';
import type { FileItemState, FileStatus, TabKey } from './types';

export function CountBadge({
  count,
  tone,
}: {
  count: number;
  tone: 'warning' | 'success' | 'destructive';
}) {
  if (count === 0) {
    return <span className="text-xs text-muted-foreground/60 tabular-nums">0</span>;
  }
  const variant = tone === 'warning' ? 'orange' : tone === 'success' ? 'green' : 'destructive';
  return (
    <Badge
      variant={variant as 'orange' | 'green' | 'destructive'}
      className="px-1.5 py-0 h-5 text-[11px] tabular-nums"
    >
      {count}
    </Badge>
  );
}

interface TabPanelProps {
  tabKey: TabKey;
  isActive: boolean;
  items: FileItemState[];
  emptyLabel: string;
  expandedIds: Set<string>;
  onToggleExpand: (path: string) => void;
  onToggleSelected: (path: string) => void;
  onToggleAll: (tab: TabKey, value: boolean) => void;
  onFormChange: (path: string, values: DynamicMetadataFormValues) => void;
  onApproveSuggestedTag: (path: string, tagName: string) => void;
  onDismissSuggestedTag: (path: string, tagName: string) => void;
  onApproveSuggestedAuthor: (path: string, authorName: string) => void;
  onDismissSuggestedAuthor: (path: string, authorName: string) => void;
  onDuplicateAction: (path: string, action: DuplicateAction) => void;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onTagCreate: (name: string) => Promise<Tag>;
  onAuthorCreate: (name: string) => Promise<Author>;
}

export function TabPanel({
  tabKey,
  isActive,
  items,
  emptyLabel,
  expandedIds,
  onToggleExpand,
  onToggleSelected,
  onToggleAll,
  onFormChange,
  onApproveSuggestedTag,
  onDismissSuggestedTag,
  onApproveSuggestedAuthor,
  onDismissSuggestedAuthor,
  onDuplicateAction,
  categories,
  tags,
  authors,
  onTagCreate,
  onAuthorCreate,
}: TabPanelProps) {
  const selectableItems = tabKey === 'failed' ? [] : items;
  const selectedCount = items.filter((i) => i.selected).length;
  const allSelected = selectableItems.length > 0 && selectedCount === selectableItems.length;

  const parentRef = useRef<HTMLDivElement>(null);

  // `enabled: isActive` is load-bearing: the scroll element sits inside a
  // `data-[state=inactive]:hidden` ancestor, so its ResizeObserver rect stays
  // at 0×0 while inactive. Toggling enabled on activation forces the
  // virtualizer to re-subscribe the observer against the now-visible element.
  const virtualizer = useVirtualizer({
    count: items.length,
    enabled: isActive,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 72,
    overscan: 5,
    getItemKey: (index) => items[index]!.path,
  });

  return (
    <TabsContent
      value={tabKey}
      forceMount
      className="flex-1 min-h-0 flex flex-col data-[state=inactive]:hidden"
    >
      {items.length > 0 && tabKey !== 'failed' && (
        <div className="flex items-center justify-between px-1 pt-1 pb-2 text-xs text-muted-foreground shrink-0">
          <span>
            {selectedCount} of {items.length} selected
          </span>
          <button
            type="button"
            onClick={() => onToggleAll(tabKey, !allSelected)}
            className="text-primary hover:underline"
          >
            {allSelected ? 'Deselect all' : 'Select all'}
          </button>
        </div>
      )}
      <div ref={parentRef} className="flex-1 min-h-0 -mx-6 overflow-y-auto">
        {items.length === 0 ? (
          <div className="py-10 text-center text-sm text-muted-foreground font-serif-italic">
            {emptyLabel}
          </div>
        ) : (
          <div
            style={{
              height: virtualizer.getTotalSize(),
              width: '100%',
              position: 'relative',
            }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const item = items[virtualRow.index]!;
              return (
                <div
                  key={virtualRow.key}
                  data-index={virtualRow.index}
                  ref={virtualizer.measureElement}
                  className="px-6 pb-2"
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    width: '100%',
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                >
                  <FileCardRow
                    item={item}
                    tabKey={tabKey}
                    expanded={expandedIds.has(item.path)}
                    onToggleExpand={onToggleExpand}
                    onToggleSelected={onToggleSelected}
                    onFormChange={onFormChange}
                    onApproveSuggestedTag={onApproveSuggestedTag}
                    onDismissSuggestedTag={onDismissSuggestedTag}
                    onApproveSuggestedAuthor={onApproveSuggestedAuthor}
                    onDismissSuggestedAuthor={onDismissSuggestedAuthor}
                    onDuplicateAction={onDuplicateAction}
                    categories={categories}
                    tags={tags}
                    authors={authors}
                    onTagCreate={onTagCreate}
                    onAuthorCreate={onAuthorCreate}
                  />
                </div>
              );
            })}
          </div>
        )}
      </div>
    </TabsContent>
  );
}

/** Renders a VNDB cover thumbnail by fetching its bytes through the Rust
 *  `vndb_fetch_cover` command and showing a local data URL. We never put the
 *  remote `t.vndb.org` URL in an <img src> — the webview CSP is `self`-only,
 *  so a cross-origin image would be blocked. Falls back to a dashed
 *  placeholder while loading, on error, or when there's no cover. */
function VndbThumb({ url }: { url: string | null }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (!url) {
      setSrc(null);
      return;
    }
    let cancelled = false;
    vndbFetchCover(url)
      .then(({ data, mime_type }) => {
        if (!cancelled) setSrc(`data:${mime_type};base64,${data}`);
      })
      .catch(() => {
        if (!cancelled) setSrc(null);
      });
    return () => {
      cancelled = true;
    };
  }, [url]);
  return src ? (
    <img src={src} alt="" className="h-14 w-10 object-cover rounded border shrink-0" />
  ) : (
    <div className="h-14 w-10 rounded border border-dashed shrink-0" />
  );
}

/** Galgame-only VNDB match panel. Auto-searches on mount using the cleaned
 *  display name (the filename-LLM result, or the raw file name when the LLM is
 *  off), lets the user pick a candidate or re-search, and on pick autofills
 *  the form: origin title (alttitle → title), cover (fetched + dropped into
 *  `cover_data`), and developer (routed through the existing author-adopt
 *  handler so it reuses find-or-create). Failures degrade to manual entry. */
function GalgameVndbPanel({
  item,
  onFormChange,
  onApproveSuggestedAuthor,
}: {
  item: FileItemState;
  onFormChange: (path: string, values: DynamicMetadataFormValues) => void;
  onApproveSuggestedAuthor: (path: string, authorName: string) => void;
}) {
  const [query, setQuery] = useState(
    () => item.formValues.display_name || item.preparedImport?.file_name || ''
  );
  const [candidates, setCandidates] = useState<VndbCandidate[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [applyingId, setApplyingId] = useState<string | null>(null);

  const runSearch = useCallback(async (q: string) => {
    const trimmed = q.trim();
    if (!trimmed) {
      setCandidates([]);
      return;
    }
    setSearching(true);
    setError(null);
    try {
      setCandidates(await vndbSearch(trimmed));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setCandidates([]);
    } finally {
      setSearching(false);
    }
  }, []);

  // Auto-search once on mount with the cleaned name. Re-runs only via the
  // manual search box afterward so we don't spam the API on every re-render.
  const didAutoSearch = useRef(false);
  useEffect(() => {
    if (didAutoSearch.current) return;
    didAutoSearch.current = true;
    void runSearch(query);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const applyCandidate = async (c: VndbCandidate) => {
    setApplyingId(c.id);
    try {
      const title = c.alttitle?.trim() || c.title.trim();
      let next: DynamicMetadataFormValues = {
        ...item.formValues,
        display_name: title || item.formValues.display_name,
      };
      // Fetch the cover and inline it so the normal commit path stores it.
      if (c.image_url) {
        try {
          const { data, mime_type } = await vndbFetchCover(c.image_url);
          next = {
            ...next,
            cover_data: data,
            cover_mime_type: mime_type,
            cover_removed: false,
            staged_cover_path: undefined,
          };
        } catch (err) {
          console.error('VNDB cover fetch failed:', err);
        }
      }
      onFormChange(item.path, next);
      // Developer → author through the existing adopt handler (find-or-create
      // against the catalog snapshot). Only the first developer is adopted.
      const dev = c.developers[0]?.trim();
      if (dev) onApproveSuggestedAuthor(item.path, dev);
    } finally {
      setApplyingId(null);
    }
  };

  return (
    <div className="space-y-2">
      <p className="text-xs text-muted-foreground">VNDB match</p>
      <div className="flex items-center gap-2">
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              void runSearch(query);
            }
          }}
          placeholder="Search VNDB by title…"
          className="h-8 text-sm"
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-8 shrink-0"
          onClick={() => void runSearch(query)}
          disabled={searching}
        >
          {searching ? <Loader2 className="h-4 w-4 animate-spin" /> : 'Search'}
        </Button>
      </div>

      {error && <p className="text-xs text-destructive">VNDB error: {error}</p>}

      {candidates != null && candidates.length === 0 && !searching && !error && (
        <p className="text-xs text-muted-foreground">
          No matches — edit the title and search again, or fill metadata manually below.
        </p>
      )}

      {candidates != null && candidates.length > 0 && (
        <ul className="space-y-1.5">
          {candidates.map((c) => (
            <li key={c.id}>
              <button
                type="button"
                onClick={() => void applyCandidate(c)}
                disabled={applyingId != null}
                className="w-full flex items-center gap-3 rounded-md border p-2 text-left hover:bg-muted/50 transition-colors disabled:opacity-50"
              >
                <VndbThumb url={c.thumbnail ?? c.image_url} />
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium truncate">{c.alttitle?.trim() || c.title}</p>
                  {c.alttitle?.trim() && (
                    <p className="text-xs text-muted-foreground truncate">{c.title}</p>
                  )}
                  <p className="text-xs text-muted-foreground truncate">
                    {[c.released, c.developers[0]].filter(Boolean).join(' · ') || '—'}
                  </p>
                </div>
                {applyingId === c.id && <Loader2 className="h-4 w-4 animate-spin shrink-0" />}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

interface FileCardRowProps {
  item: FileItemState;
  tabKey: TabKey;
  expanded: boolean;
  onToggleExpand: (path: string) => void;
  onToggleSelected: (path: string) => void;
  onFormChange: (path: string, values: DynamicMetadataFormValues) => void;
  onApproveSuggestedTag: (path: string, tagName: string) => void;
  onDismissSuggestedTag: (path: string, tagName: string) => void;
  onApproveSuggestedAuthor: (path: string, authorName: string) => void;
  onDismissSuggestedAuthor: (path: string, authorName: string) => void;
  onDuplicateAction: (path: string, action: DuplicateAction) => void;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onTagCreate: (name: string) => Promise<Tag>;
  onAuthorCreate: (name: string) => Promise<Author>;
}

function FileCardRow({
  item,
  tabKey,
  expanded,
  onToggleExpand,
  onToggleSelected,
  onFormChange,
  onApproveSuggestedTag,
  onDismissSuggestedTag,
  onApproveSuggestedAuthor,
  onDismissSuggestedAuthor,
  onDuplicateAction,
  categories,
  tags,
  authors,
  onTagCreate,
  onAuthorCreate,
}: FileCardRowProps) {
  const canExpand = tabKey !== 'failed' && (item.status === 'ready' || item.status === 'partial');
  const checkboxDisabled = tabKey === 'failed';

  return (
    <Card
      className={`transition-all duration-200 ${
        !item.selected && tabKey !== 'failed' ? 'opacity-60' : ''
      }`}
    >
      <CardContent className="p-3">
        <div className="flex items-center gap-3">
          {/* Checkbox */}
          <input
            type="checkbox"
            checked={item.selected}
            disabled={checkboxDisabled}
            onChange={() => onToggleSelected(item.path)}
            onClick={(e) => e.stopPropagation()}
            className="h-4 w-4 shrink-0 accent-primary cursor-pointer disabled:cursor-not-allowed"
            aria-label={`Include ${item.fileName}`}
          />

          {/* Clickable header — toggles expand (only when there's something to show) */}
          <div
            className={`flex-1 min-w-0 flex items-center gap-3 ${
              canExpand ? 'cursor-pointer hover:text-primary/90' : ''
            }`}
            onClick={canExpand ? () => onToggleExpand(item.path) : undefined}
          >
            <StatusIcon status={item.status} />

            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2 min-w-0">
                <p className="text-sm font-medium truncate">{item.fileName}</p>
                {item.preparedImport?.source_is_directory && <FolderToZipHint />}
              </div>
              <StatusSubtitle item={item} />
            </div>

            {canExpand && (
              <div className="shrink-0 text-muted-foreground">
                {expanded ? (
                  <ChevronDown className="h-4 w-4" />
                ) : (
                  <ChevronRight className="h-4 w-4" />
                )}
              </div>
            )}
          </div>
        </div>

        {/* Expandable form */}
        {expanded &&
          canExpand &&
          (() => {
            // Resolve the schema once per render so the dupe panel and the
            // form below it always agree on which slug is in play (novel
            // vs comic decides which compare rows render in the panel).
            // Same resolution logic that drove the form's `schema` prop
            // pre-refactor — lifted up so both consumers share one source.
            const resolvedSchema =
              item.formValues.category_id != null
                ? schemaForCategoryId(item.formValues.category_id, categories)
                : (schemaForPath(item.path) ??
                  (item.preparedImport?.source_is_directory ? REGISTRY.comic : defaultSchema()));
            // Resolve the new-side author ids → names via the parent's
            // `authors` snapshot so the dupe panel can render the row
            // without a follow-up fetch. Ids that don't match anything
            // in the snapshot (extremely rare race) fall through to
            // `#<id>` so the panel still has something to display.
            const resolvedNewAuthors = item.formValues.author_ids.map(
              (id) => authors.find((a) => a.id === id)?.name ?? `#${id}`
            );
            return (
              <div className="mt-4 pt-4 border-t border-border space-y-4">
                {item.preparedImport?.duplicate_of && (
                  <DuplicateWarning
                    duplicateInfo={item.preparedImport.duplicate_of}
                    schema={resolvedSchema}
                    newDisplayName={item.formValues.display_name}
                    newAuthorNames={resolvedNewAuthors}
                    newProgress={item.formValues.progress ?? null}
                    newCoverData={item.formValues.cover_data}
                    newCoverMimeType={item.formValues.cover_mime_type}
                    newStagedCoverPath={item.formValues.staged_cover_path}
                    selectedAction={item.duplicateAction}
                    onActionChange={(action) => onDuplicateAction(item.path, action)}
                  />
                )}

                {item.suggestedAuthors.length > 0 && (
                  <div className="space-y-2">
                    <p className="text-xs text-muted-foreground">Suggested authors:</p>
                    <div className="flex flex-wrap gap-1.5">
                      {item.suggestedAuthors.map((author) => (
                        <SuggestedTagChip
                          key={author}
                          name={author}
                          noun="author"
                          onApprove={(name) => onApproveSuggestedAuthor(item.path, name)}
                          onDismiss={(name) => onDismissSuggestedAuthor(item.path, name)}
                        />
                      ))}
                    </div>
                  </div>
                )}

                {item.suggestedTags.length > 0 && (
                  <div className="space-y-2">
                    <p className="text-xs text-muted-foreground">Suggested new tags:</p>
                    <div className="flex flex-wrap gap-1.5">
                      {item.suggestedTags.map((tag) => (
                        <SuggestedTagChip
                          key={tag}
                          name={tag}
                          onApprove={(name) => onApproveSuggestedTag(item.path, name)}
                          onDismiss={(name) => onDismissSuggestedTag(item.path, name)}
                        />
                      ))}
                    </div>
                  </div>
                )}

                {resolvedSchema.slug === 'galgame' && (
                  <GalgameVndbPanel
                    item={item}
                    onFormChange={onFormChange}
                    onApproveSuggestedAuthor={onApproveSuggestedAuthor}
                  />
                )}

                <DynamicMetadataForm
                  values={item.formValues}
                  onChange={(values) => onFormChange(item.path, values)}
                  // Reuse the resolved schema lifted to the IIFE top so the
                  // form and the dupe panel agree on the slug. Resolution
                  // rule unchanged: user-picked category wins; fallback to
                  // path-based schema; folder imports default to comic.
                  schema={resolvedSchema}
                  categories={categories}
                  tags={tags}
                  authors={authors}
                  onTagCreate={onTagCreate}
                  onAuthorCreate={onAuthorCreate}
                />
              </div>
            );
          })()}

        {/* Error message */}
        {item.status === 'error' && item.error && (
          <div className="mt-2 ml-7 text-xs text-destructive">{item.error}</div>
        )}
      </CardContent>
    </Card>
  );
}

function FolderToZipHint() {
  return (
    <span
      className="shrink-0 inline-flex items-center gap-1 rounded-full border border-border/60 bg-secondary/40 px-1.5 py-0 text-[10px] text-muted-foreground"
      title="This folder of images will be packaged as a .zip on import"
    >
      <FolderArchive className="h-3 w-3" aria-hidden="true" />
      Folder → .zip
    </span>
  );
}

function StatusIcon({ status }: { status: FileStatus }) {
  return (
    <div className="shrink-0">
      {status === 'extracting_name' || status === 'analyzing_content' ? (
        <Loader2 className="h-4 w-4 animate-spin text-primary" />
      ) : status === 'ready' ? (
        <CheckCircle2 className="h-4 w-4 text-notion-green" />
      ) : status === 'partial' ? (
        <AlertTriangle className="h-4 w-4 text-notion-orange" />
      ) : status === 'error' ? (
        <AlertCircle className="h-4 w-4 text-destructive" />
      ) : (
        <FileText className="h-4 w-4 text-muted-foreground" />
      )}
    </div>
  );
}

function StatusSubtitle({ item }: { item: FileItemState }) {
  if (item.status === 'extracting_name') {
    return <p className="text-xs text-muted-foreground">Extracting name…</p>;
  }
  if (item.status === 'analyzing_content') {
    return <p className="text-xs text-muted-foreground">Analyzing content…</p>;
  }
  if (item.status === 'partial') {
    return (
      <p className="text-xs text-notion-orange">Partial extraction — please fill missing fields</p>
    );
  }
  if (item.preparedImport?.duplicate_of) {
    const d = item.preparedImport.duplicate_of;
    return (
      <p className="text-xs text-muted-foreground">
        Duplicate of <span className="font-serif-italic">{d.existing_display_name}</span>
        {d.existing_progress ? ` (${d.existing_progress})` : ''}
      </p>
    );
  }
  return null;
}

/** Minimized state for the import dialog — a single click-target pill
 *  that re-expands. Mirrors the shape and corner placement of the
 *  remote-upload pill so the two coexist visually. The pill summarizes
 *  in-flight + ready + failed counts; the worker keeps emitting events
 *  while minimized, so these numbers stay live. */
export function MinimizedPipelinePill({
  totalFiles,
  readyCount,
  reviewCount,
  failedCount,
  processingCount,
  analyzing,
  onExpand,
}: {
  totalFiles: number;
  readyCount: number;
  reviewCount: number;
  failedCount: number;
  processingCount: number;
  analyzing: boolean;
  onExpand: () => void;
}) {
  const done = readyCount + reviewCount + failedCount;
  return (
    <button
      type="button"
      onClick={onExpand}
      className="fixed bottom-4 right-4 z-50 bg-background border border-border rounded-full shadow-lg flex items-center pl-3 pr-2 py-1 gap-2 text-xs hover:bg-secondary/40 transition-colors"
      aria-label="Expand import dialog"
    >
      {analyzing ? (
        <Loader2 className="h-3.5 w-3.5 animate-spin text-primary shrink-0" />
      ) : failedCount > 0 ? (
        <AlertCircle className="h-3.5 w-3.5 text-destructive shrink-0" />
      ) : (
        <CheckCircle2 className="h-3.5 w-3.5 text-success shrink-0" />
      )}
      <span className="text-foreground/80">
        Import {done}/{totalFiles}
        {processingCount > 0 && (
          <span className="text-muted-foreground ml-1.5">· {processingCount} analyzing</span>
        )}
        {failedCount > 0 && <span className="text-destructive ml-1.5">· {failedCount} failed</span>}
      </span>
      <ChevronUp className="h-3.5 w-3.5 text-muted-foreground" />
    </button>
  );
}
