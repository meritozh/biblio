/**
 * Frontend file-kind registry.
 *
 * Mirrors the backend `pipeline::nodes::kind_for_path` split: extension-based
 * dispatch, no DB column. One source of truth for the per-kind UI deltas:
 * which columns the file list shows, which fields the metadata form shows,
 * and where new imports default to (local vs remote storage).
 *
 * Adding a new kind = one entry here. The list/dialog components consume
 * the registry; they don't branch on kind themselves.
 */

import type { Category, StorageKind } from '@/types';

export type FileKind = 'novel' | 'comic';

/**
 * Form sections that callers can opt in/out of via the kind registry.
 * Order in the kind's `formFields` list IS the visual render order, so
 * the registry doubles as the form layout per kind.
 */
export type FormFieldKey =
  | 'display_name'
  | 'category'
  | 'authors'
  | 'tags'
  | 'progress'
  | 'description'
  | 'cover'
  | 'volume';

/** How FileList should render this kind's data. Comics emphasize cover
 *  art via a card grid; novels read better as a sortable table. */
export type FileListLayout = 'table' | 'grid';

export interface KindSchema {
  /** Lowercased file extensions that route to this kind. */
  extensions: ReadonlyArray<string>;
  /** Default destination for fresh imports of this kind. */
  defaultStorage: StorageKind;
  /**
   * Column visibility passed to TanStack Table on FileList. Hideable
   * column ids only — `display_name` and `actions` are always visible.
   * Only consulted when `layout === 'table'`.
   */
  columns: Readonly<Record<string, boolean>>;
  /** Form section render list, in the order they should appear. */
  formFields: ReadonlyArray<FormFieldKey>;
  /** Render shape for the file list. */
  layout: FileListLayout;
  /**
   * Case-insensitive name of the category to auto-select on import when
   * the LLM didn't choose one. `null` means "leave it blank."
   */
  defaultCategoryName: string | null;
}

export const KIND_REGISTRY: Readonly<Record<FileKind, KindSchema>> = {
  comic: {
    extensions: ['cbz', 'zip', 'cbr', 'rar'],
    defaultStorage: 'remote',
    columns: {
      cover: true,
      description: false,
      tags: false,
      authors: true,
      progress: false,
    },
    formFields: ['display_name', 'category', 'authors', 'cover'],
    layout: 'grid',
    defaultCategoryName: 'comic',
  },
  novel: {
    extensions: ['txt', 'epub', 'pdf'],
    defaultStorage: 'local',
    columns: {
      cover: false,
      description: true,
      tags: true,
      authors: true,
      progress: true,
    },
    formFields: [
      'display_name',
      'category',
      'authors',
      'tags',
      'progress',
      'description',
    ],
    layout: 'table',
    defaultCategoryName: null,
  },
};

/** Resolve a file path to its kind by extension. Anything unknown falls
 *  through to `novel` — matches the backend dispatcher's default. */
export function kindForPath(path: string | null | undefined): FileKind {
  const ext = (path ?? '').split('.').pop()?.toLowerCase() ?? '';
  if (KIND_REGISTRY.comic.extensions.includes(ext)) return 'comic';
  return 'novel';
}

export function schemaForPath(path: string | null | undefined): KindSchema {
  return KIND_REGISTRY[kindForPath(path)];
}

/** Resolve the registry-default category id for a kind, given the user's
 *  current category list. Case-insensitive exact match on the schema's
 *  `defaultCategoryName`; returns null if the schema has no default OR
 *  no category with that name exists. */
export function defaultCategoryIdForKind(
  kind: FileKind,
  categories: Category[]
): number | null {
  const target = KIND_REGISTRY[kind].defaultCategoryName;
  if (!target) return null;
  const lc = target.toLowerCase();
  return categories.find((c) => c.name.toLowerCase() === lc)?.id ?? null;
}
