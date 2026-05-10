import { Store, useStore } from '@tanstack/react-store';
import type { FileEntry } from '@/types';

interface ViewState {
  ids: number[];
  total: number;
  loading: boolean;
}

interface FileStoreState {
  /** Normalized entry cache shared across all views. Keeping a global pool
   *  means cross-view navigation (Library → tag detail) doesn't refetch
   *  rows that are already in memory. */
  byId: Map<number, FileEntry>;
  /** Per-view ordered id slice. Keys: `category::N::query=Q`, `tag::N`,
   *  `author::N`, `cat-detail::N`, `search::query=Q::category=N`, etc.
   *  Each route owns one key via `useView`. */
  views: Map<string, ViewState>;
  /** Bumped to force every active `useView` to re-fetch. Used by tag/author
   *  rename/delete events that invalidate denormalized chip text on rows. */
  refreshEpoch: number;
}

const EMPTY_VIEW: ViewState = { ids: [], total: 0, loading: false };

const initialState: FileStoreState = {
  byId: new Map(),
  views: new Map(),
  refreshEpoch: 0,
};

export const fileStore = new Store<FileStoreState>(initialState);

// ── Internal helpers ─────────────────────────────────────────────────────────

function mergeRows(
  byId: Map<number, FileEntry>,
  files: ReadonlyArray<FileEntry>
): Map<number, FileEntry> {
  const next = new Map(byId);
  for (const f of files) next.set(f.id, f);
  return next;
}

// ── Actions ──────────────────────────────────────────────────────────────────

/** Replace a view's slice; merges incoming rows into `byId`. Use on initial
 *  fetch and on every full re-fetch (category change, search change). */
export function setView(
  key: string,
  files: ReadonlyArray<FileEntry>,
  total: number
): void {
  fileStore.setState((s) => {
    const byId = mergeRows(s.byId, files);
    const views = new Map(s.views);
    views.set(key, { ids: files.map((f) => f.id), total, loading: false });
    return { ...s, byId, views };
  });
}

export function setViewLoading(key: string, loading: boolean): void {
  fileStore.setState((s) => {
    const views = new Map(s.views);
    const prev = views.get(key) ?? EMPTY_VIEW;
    views.set(key, { ...prev, loading });
    return { ...s, views };
  });
}

/** Append rows to an existing view (load-more pagination). */
export function appendToView(
  key: string,
  files: ReadonlyArray<FileEntry>,
  total?: number
): void {
  fileStore.setState((s) => {
    const byId = mergeRows(s.byId, files);
    const views = new Map(s.views);
    const prev = views.get(key) ?? EMPTY_VIEW;
    const seen = new Set(prev.ids);
    const newIds = files.map((f) => f.id).filter((id) => !seen.has(id));
    views.set(key, {
      ids: [...prev.ids, ...newIds],
      total: total ?? prev.total,
      loading: false,
    });
    return { ...s, byId, views };
  });
}

/** Merge a partial into an existing row. No-op if the row isn't cached.
 *  Mutating one row produces a new entry reference for that id; references
 *  for unchanged ids stay stable, so per-card selectors don't re-render. */
export function patchFile(id: number, partial: Partial<FileEntry>): void {
  fileStore.setState((s) => {
    const existing = s.byId.get(id);
    if (!existing) return s;
    const byId = new Map(s.byId);
    byId.set(id, { ...existing, ...partial });
    return { ...s, byId };
  });
}

/** Drop a row from `byId` and from every view's `ids`. Total counts on each
 *  view decrement only if the id was actually in that view, so non-affected
 *  views keep accurate totals. */
export function removeFile(id: number): void {
  fileStore.setState((s) => {
    if (!s.byId.has(id)) return s;
    const byId = new Map(s.byId);
    byId.delete(id);
    const views = new Map(s.views);
    for (const [key, view] of views) {
      if (view.ids.includes(id)) {
        views.set(key, {
          ids: view.ids.filter((x) => x !== id),
          total: Math.max(0, view.total - 1),
          loading: view.loading,
        });
      }
    }
    return { ...s, byId, views };
  });
}

/** Insert a single new row, optionally pushing it to the front of one view.
 *  The view's total bumps by one; other views are untouched. */
export function addFile(file: FileEntry, viewKey?: string): void {
  fileStore.setState((s) => {
    const byId = new Map(s.byId);
    byId.set(file.id, file);
    const views = new Map(s.views);
    if (viewKey) {
      const prev = views.get(viewKey) ?? EMPTY_VIEW;
      if (!prev.ids.includes(file.id)) {
        views.set(viewKey, {
          ids: [file.id, ...prev.ids],
          total: prev.total + 1,
          loading: prev.loading,
        });
      }
    }
    return { ...s, byId, views };
  });
}

/** Force every active `useView` to re-fetch. Used by tag/author rename
 *  events: chip text is denormalized onto each row, so any rename
 *  invalidates the cached row contents. */
export function refreshActiveView(): void {
  fileStore.setState((s) => ({ ...s, refreshEpoch: s.refreshEpoch + 1 }));
}

// ── Selectors / hooks ────────────────────────────────────────────────────────

/** Subscribe a card to a single row by id. Returns the same reference until
 *  that row is patched, so React.memo'd cards skip re-render on unrelated
 *  updates. */
export function useFile(id: number): FileEntry | undefined {
  return useStore(fileStore, (s) => s.byId.get(id));
}

export function useView(key: string): ViewState {
  return useStore(fileStore, (s) => s.views.get(key) ?? EMPTY_VIEW);
}

export function useRefreshEpoch(): number {
  return useStore(fileStore, (s) => s.refreshEpoch);
}
