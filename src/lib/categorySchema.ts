/**
 * Category-driven schema registry.
 *
 * Each category row carries a `schema_slug` that picks a `CategorySchema`
 * from this file. The schema decides:
 *   - which form sections render in import / edit dialogs
 *   - which fields show on the file card in the grid
 *   - which file extensions this schema's pipeline can handle
 *   - the default storage destination for fresh imports
 *
 * Mirrors `src-tauri/src/schema.rs` on the backend (the Rust side only
 * cares about the slug → prompt routing). To add a new schema:
 *   1. Add a member to `SchemaSlug` in this file and `src/types/index.ts`.
 *   2. Add the matching variant in `src-tauri/src/schema.rs`.
 *   3. Add a row to `REGISTRY` here.
 *   4. Seed prompts under `(schema_slug, step)` via the Prompts page.
 *
 * `kindForPath` from the prior `fileKind.ts` lives here as
 * `schemaForPath` — used at the import-time boundary where the user
 * hasn't picked a category yet, so we fall back to extension routing.
 */

import type { Category, SchemaSlug } from '@/types';

export type { SchemaSlug } from '@/types';

/**
 * Form sections that callers can opt in/out of via the schema registry.
 * Order in the schema's `formFields` list IS the visual render order, so
 * the registry doubles as the form layout per schema.
 */
export type FormFieldKey =
  | 'display_name'
  | 'category'
  | 'authors'
  | 'tags'
  | 'progress'
  | 'cover'
  | 'volume';

/** Card display sections. The card always renders the cover image and
 *  the title; this list controls everything below them. */
export type CardFieldKey = 'authors' | 'tags' | 'progress';

export interface CategorySchema {
  slug: SchemaSlug;
  /** Form sections (ordered) used in both the import dialog's per-file
   *  review pane and the edit dialog. */
  formFields: ReadonlyArray<FormFieldKey>;
  /** Fields shown on the file card under the title, in order. */
  cardFields: ReadonlyArray<CardFieldKey>;
  /** Lowercased extensions this schema's pipeline accepts. Used at the
   *  import-time fallback when the user hasn't picked a category yet. */
  acceptedExtensions: ReadonlyArray<string>;
}

export const REGISTRY: Readonly<Record<SchemaSlug, CategorySchema>> = {
  novel: {
    slug: 'novel',
    formFields: [
      'display_name',
      'category',
      'authors',
      'tags',
      'progress',
    ],
    cardFields: ['authors'],
    acceptedExtensions: ['txt'],
  },
  comic: {
    slug: 'comic',
    formFields: ['display_name', 'category', 'authors', 'cover'],
    cardFields: ['authors'],
    acceptedExtensions: ['cbz', 'zip', 'cbr', 'rar'],
  },
};

/** Safe slug coercion. Mirrors the Rust fallback: any unknown value
 *  collapses to `'novel'`, so a stale slug from a row written by a
 *  newer binary doesn't crash render paths. */
export function coerceSchemaSlug(raw: string | null | undefined): SchemaSlug {
  if (raw === 'novel' || raw === 'comic') return raw;
  return 'novel';
}

export function defaultSchema(): CategorySchema {
  return REGISTRY.novel;
}

/** Resolve the schema for a category, with a safe fallback. */
export function schemaForCategory(
  category: Category | null | undefined
): CategorySchema {
  if (!category) return defaultSchema();
  return REGISTRY[coerceSchemaSlug(category.schema_slug)];
}

/** Resolve the schema by category id, given the user's category list.
 *  Returns the default schema when the id isn't in the list. */
export function schemaForCategoryId(
  categoryId: number | null | undefined,
  categories: ReadonlyArray<Category>
): CategorySchema {
  if (categoryId == null) return defaultSchema();
  const cat = categories.find((c) => c.id === categoryId);
  return schemaForCategory(cat);
}

/** Resolve the schema by extension. Used at import time when the user
 *  hasn't picked a category yet. Returns null if the extension isn't
 *  in any schema's `acceptedExtensions` set — the file picker treats
 *  that as "not importable" and skips the path. */
export function schemaForPath(
  path: string | null | undefined
): CategorySchema | null {
  const ext = (path ?? '').split('.').pop()?.toLowerCase() ?? '';
  for (const schema of Object.values(REGISTRY)) {
    if (schema.acceptedExtensions.includes(ext)) return schema;
  }
  return null;
}

/** True if the extension is in any registered schema. The drag-drop
 *  handler uses this to silently skip unsupported paths. */
export function isImportable(path: string | null | undefined): boolean {
  return schemaForPath(path) !== null;
}

/** First category in `categories` that uses the given schema slug.
 *  Used at import time to auto-pick a default category when the LLM
 *  didn't choose one. Returns null if the user has no category with
 *  that schema. */
export function defaultCategoryIdForSchema(
  slug: SchemaSlug,
  categories: ReadonlyArray<Category>
): number | null {
  const match = categories.find((c) => coerceSchemaSlug(c.schema_slug) === slug);
  return match?.id ?? null;
}

/** Allowed prompt steps per schema, mirrored from the backend whitelist
 *  in `src-tauri/src/commands/prompts.rs::validate_slug_step`. The
 *  prompts page reads this to populate its step picker. */
export const PROMPT_STEPS_BY_SCHEMA: Readonly<
  Record<SchemaSlug, ReadonlyArray<{ step: import('@/types').PromptStep; label: string }>>
> = {
  novel: [
    { step: 'filename', label: 'Filename extraction' },
    { step: 'content', label: 'Content analysis' },
    { step: 'category_reanalyze', label: 'Category re-analysis (cleanup)' },
  ],
  comic: [
    { step: 'filename', label: 'Filename extraction' },
    { step: 'cover_pick', label: 'Cover detection' },
    { step: 'filename_folder', label: 'Folder filename extraction' },
  ],
};

export const SCHEMA_LABELS: Readonly<Record<SchemaSlug, string>> = {
  novel: 'Novel',
  comic: 'Comic',
};
