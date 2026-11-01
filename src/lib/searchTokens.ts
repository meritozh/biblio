/**
 * Token search helpers — iOS Photos-style field chips.
 *
 * Typing a field keyword (`author`, `tag`, `cache`…) surfaces autocomplete;
 * confirming a value produces a `Condition`. Anything left in the input that
 * isn't consumed by a token is free-text search.
 */

import {
  FAVORITE_OPTIONS,
  FIELD_LABELS,
  FILE_STATUS_OPTIONS,
  STORAGE_KIND_OPTIONS,
  makeId,
  type Condition,
  type Field,
} from '@/lib/filters';
import type { Author, FileStatus, StorageKind, Tag } from '@/types';

export type SearchFieldKey =
  'authors' | 'tags' | 'local_cache' | 'file_status' | 'storage_kind' | 'favorite' | 'progress';

export interface FieldSuggestion {
  kind: 'field';
  field: SearchFieldKey;
  label: string;
  aliases: string[];
}

export interface ValueSuggestion {
  kind: 'value';
  field: SearchFieldKey;
  label: string;
  condition: Condition;
}

export type SearchSuggestion = FieldSuggestion | ValueSuggestion;

const FIELD_CATALOG: ReadonlyArray<FieldSuggestion> = [
  {
    kind: 'field',
    field: 'authors',
    label: FIELD_LABELS.authors,
    aliases: ['author', 'authors', 'by'],
  },
  {
    kind: 'field',
    field: 'tags',
    label: FIELD_LABELS.tags,
    aliases: ['tag', 'tags'],
  },
  {
    kind: 'field',
    field: 'local_cache',
    label: 'Cached',
    aliases: ['cache', 'cached'],
  },
  {
    kind: 'field',
    field: 'file_status',
    label: FIELD_LABELS.file_status,
    aliases: ['status'],
  },
  {
    kind: 'field',
    field: 'storage_kind',
    label: FIELD_LABELS.storage_kind,
    aliases: ['storage', 'store'],
  },
  {
    kind: 'field',
    field: 'favorite',
    label: FIELD_LABELS.favorite,
    aliases: ['favorite', 'favourite', 'fav', 'star'],
  },
  {
    kind: 'field',
    field: 'progress',
    label: FIELD_LABELS.progress,
    aliases: ['progress'],
  },
];

function normalize(text: string): string {
  return text.trim().toLowerCase();
}

/** Match typed text against field keywords. Empty query → all fields. */
export function suggestFields(query: string): FieldSuggestion[] {
  const q = normalize(query);
  if (!q) return [...FIELD_CATALOG];
  return FIELD_CATALOG.filter(
    (f) =>
      f.label.toLowerCase().startsWith(q) || f.aliases.some((a) => a.startsWith(q) || a.includes(q))
  );
}

const CONCRETE_FIELDS: readonly SearchFieldKey[] = [
  'tags',
  'authors',
  'favorite',
  'local_cache',
  'file_status',
  'storage_kind',
];

/**
 * Free-text autocomplete. Concrete values (a tag, an author, Favorited, …)
 * come first so Arrow+Enter can apply a finished filter. Field rows are the
 * drill-in when the query only matches a keyword.
 */
export function suggestSearchList(
  query: string,
  opts: {
    tags?: ReadonlyArray<Tag>;
    authors?: ReadonlyArray<Author>;
  } = {}
): SearchSuggestion[] {
  const q = normalize(query);
  const concrete = q
    ? CONCRETE_FIELDS.flatMap((field) => suggestFieldValues(field, query, opts))
    : [];
  return [...concrete, ...suggestFields(query)].slice(0, 12);
}

/**
 * Text to keep as free-text search after a token is applied.
 * A pure field keyword (`tag`) or a query that only picked the value
 * (`sci` → `sci-fi`) is consumed. Any other phrase is kept.
 */
export function textAfterTokenPick(text: string, label: string, field: SearchFieldKey): string {
  const trimmed = text.trim();
  if (!trimmed) return '';
  const keyword = matchFieldKeyword(trimmed);
  if (keyword?.field === field) return '';
  if (label.toLowerCase().includes(trimmed.toLowerCase())) return '';
  return trimmed;
}

/** Exact field keyword match (e.g. user typed `tag` + space/enter). */
export function matchFieldKeyword(query: string): FieldSuggestion | null {
  const q = normalize(query);
  if (!q) return null;
  return (
    FIELD_CATALOG.find((f) => f.aliases.some((a) => a === q) || f.label.toLowerCase() === q) ?? null
  );
}

export function suggestFieldValues(
  field: SearchFieldKey,
  query: string,
  opts: {
    tags?: ReadonlyArray<Tag>;
    authors?: ReadonlyArray<Author>;
  } = {}
): ValueSuggestion[] {
  const q = normalize(query);

  switch (field) {
    case 'authors': {
      const authors = opts.authors ?? [];
      return authors
        .filter((a) => !q || a.name.toLowerCase().includes(q))
        .slice(0, 8)
        .map((a) => ({
          kind: 'value' as const,
          field,
          label: a.name,
          condition: {
            id: makeId(),
            field: 'authors',
            op: 'includes',
            authorId: a.id,
          } satisfies Condition,
        }));
    }
    case 'tags': {
      const tags = opts.tags ?? [];
      return tags
        .filter((t) => !q || t.name.toLowerCase().includes(q))
        .slice(0, 8)
        .map((t) => ({
          kind: 'value' as const,
          field,
          label: t.name,
          condition: {
            id: makeId(),
            field: 'tags',
            op: 'includes',
            tagId: t.id,
          } satisfies Condition,
        }));
    }
    case 'local_cache': {
      const options: Array<{ label: string; op: 'not_empty' | 'empty' }> = [
        { label: 'Cached', op: 'not_empty' },
        { label: 'Not cached', op: 'empty' },
      ];
      return options
        .filter((o) => !q || o.label.toLowerCase().includes(q))
        .map((o) => ({
          kind: 'value' as const,
          field,
          label: o.label,
          condition: {
            id: makeId(),
            field: 'local_cache',
            op: o.op,
          } satisfies Condition,
        }));
    }
    case 'file_status':
      return FILE_STATUS_OPTIONS.filter(
        (o) => !q || o.label.toLowerCase().includes(q) || o.value.includes(q)
      ).map((o) => ({
        kind: 'value' as const,
        field,
        label: o.label,
        condition: {
          id: makeId(),
          field: 'file_status',
          op: 'is',
          value: o.value as FileStatus,
        } satisfies Condition,
      }));
    case 'storage_kind':
      return STORAGE_KIND_OPTIONS.filter(
        (o) => !q || o.label.toLowerCase().includes(q) || o.value.includes(q)
      ).map((o) => ({
        kind: 'value' as const,
        field,
        label: o.label,
        condition: {
          id: makeId(),
          field: 'storage_kind',
          op: 'is',
          value: o.value as StorageKind,
        } satisfies Condition,
      }));
    case 'favorite':
      return FAVORITE_OPTIONS.filter((o) => !q || o.label.toLowerCase().includes(q)).map((o) => ({
        kind: 'value' as const,
        field,
        label: o.label,
        condition: {
          id: makeId(),
          field: 'favorite',
          op: 'is',
          value: o.value,
        } satisfies Condition,
      }));
    case 'progress': {
      if (!q) return [];
      return [
        {
          kind: 'value',
          field,
          label: `Progress contains “${query.trim()}”`,
          condition: {
            id: makeId(),
            field: 'progress',
            op: 'contains',
            text: query.trim(),
          } satisfies Condition,
        },
      ];
    }
  }
}

/** Build a progress-contains condition from free confirmation (Enter). */
export function progressContainsCondition(text: string): Condition | null {
  const trimmed = text.trim();
  if (!trimmed) return null;
  return {
    id: makeId(),
    field: 'progress',
    op: 'contains',
    text: trimmed,
  };
}

export function isSearchField(field: Field): field is SearchFieldKey {
  return FIELD_CATALOG.some((f) => f.field === field);
}
