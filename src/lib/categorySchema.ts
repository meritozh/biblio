/**
 * Category-driven schema resolution.
 *
 * Schemas are runtime-defined data: rows in the `schemas` table with
 * their `schema_fields` / `schema_pipeline_steps` children, fetched via
 * the `schema_list` command into `schemaStore`. This module derives the
 * render-time views (form field list, card field list, accepted
 * extensions, prompt steps) from those definitions. A schema decides:
 *   - which form sections render in import / edit dialogs
 *   - which fields show on the file card in the grid
 *   - which file extensions this schema's pipeline can handle
 *   - the default storage destination for fresh imports
 *
 * Before the first `loadSchemas()` resolves (and in unit tests without
 * a backend), readers fall back to `BUILTIN_FALLBACK` — a static copy
 * of the migration-v6 seeds that exactly mirrors the previous
 * hard-coded REGISTRY, so pre-load behavior is unchanged.
 *
 * `kindForPath` from the prior `fileKind.ts` lives here as
 * `schemaForPath` — used at the import-time boundary where the user
 * hasn't picked a category yet, so we fall back to extension routing.
 */

import type { Category, PromptStep, SchemaDefinition, SchemaField } from '@/types';
import { schemaStore } from '@/stores/schemaStore';

export type { SchemaSlug } from '@/types';

/**
 * Form sections that callers can opt in/out of via the schema's field
 * list. Order in the definition's fields IS the visual render order.
 * These are the built-in (semantic-backed) keys the form renderer has
 * dedicated widgets for; user-defined fields render through the
 * generic custom-field widgets keyed off `SchemaField.field_type`.
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
  slug: string;
  /** Form sections (ordered) used in both the import dialog's per-file
   *  review pane and the edit dialog. Built-in (semantic) fields only —
   *  see `fieldDefs` for the full ordered list including custom fields. */
  formFields: ReadonlyArray<FormFieldKey>;
  /** Fields shown on the file card under the title, in order. */
  cardFields: ReadonlyArray<CardFieldKey>;
  /** Lowercased extensions this schema's pipeline accepts. Used at the
   *  import-time fallback when the user hasn't picked a category yet. */
  acceptedExtensions: ReadonlyArray<string>;
  /** Full ordered field definitions (form-visible only), including
   *  user-defined fields. The metadata form iterates this so custom
   *  fields render interleaved with built-in ones. */
  fieldDefs: ReadonlyArray<SchemaField>;
}

const FORM_FIELD_KEYS: ReadonlySet<string> = new Set([
  'display_name',
  'category',
  'authors',
  'tags',
  'progress',
  'cover',
  'volume',
]);

const CARD_FIELD_KEYS: ReadonlySet<string> = new Set(['authors', 'tags', 'progress']);

/** Static copy of the migration-v6 seeds — the exact shape of the old
 *  hard-coded REGISTRY + PROMPT_STEPS_BY_SCHEMA. Used only until the
 *  first `loadSchemas()` lands and in backend-less unit tests. Keep in
 *  sync with `database/mod.rs` migration v6. */
const BUILTIN_FALLBACK: ReadonlyArray<SchemaDefinition> = [
  {
    id: 'novel',
    name: 'Novel',
    icon: null,
    description: null,
    accepted_extensions: ['txt'],
    pipeline_template: 'novel',
    is_builtin: true,
    sort_order: 0,
    created_at: '',
    updated_at: '',
    fields: [
      { id: -1, schema_id: 'novel', field_key: 'display_name', semantic: 'display_name', field_type: 'builtin', label: 'Display Name', options: null, form_visible: true, card_visible: false, sortable: false, filterable: false, required: false, order_index: 0 },
      { id: -2, schema_id: 'novel', field_key: 'category', semantic: 'category', field_type: 'builtin', label: 'Category', options: null, form_visible: true, card_visible: false, sortable: false, filterable: false, required: false, order_index: 1 },
      { id: -3, schema_id: 'novel', field_key: 'authors', semantic: 'authors', field_type: 'builtin', label: 'Authors', options: null, form_visible: true, card_visible: true, sortable: false, filterable: false, required: false, order_index: 2 },
      { id: -4, schema_id: 'novel', field_key: 'tags', semantic: 'tags', field_type: 'builtin', label: 'Tags', options: null, form_visible: true, card_visible: false, sortable: false, filterable: false, required: false, order_index: 3 },
      { id: -5, schema_id: 'novel', field_key: 'progress', semantic: 'progress', field_type: 'builtin', label: 'Progress', options: null, form_visible: true, card_visible: false, sortable: false, filterable: false, required: false, order_index: 4 },
    ],
    pipeline_steps: [
      { id: -1, schema_id: 'novel', step_key: 'filename', label: 'Filename extraction', enabled: true, order_index: 0 },
      { id: -2, schema_id: 'novel', step_key: 'content', label: 'Content analysis', enabled: true, order_index: 1 },
    ],
  },
  {
    id: 'comic',
    name: 'Comic',
    icon: null,
    description: null,
    accepted_extensions: ['cbz', 'zip', 'cbr', 'rar'],
    pipeline_template: 'comic',
    is_builtin: true,
    sort_order: 1,
    created_at: '',
    updated_at: '',
    fields: [
      { id: -6, schema_id: 'comic', field_key: 'display_name', semantic: 'display_name', field_type: 'builtin', label: 'Display Name', options: null, form_visible: true, card_visible: false, sortable: false, filterable: false, required: false, order_index: 0 },
      { id: -7, schema_id: 'comic', field_key: 'category', semantic: 'category', field_type: 'builtin', label: 'Category', options: null, form_visible: true, card_visible: false, sortable: false, filterable: false, required: false, order_index: 1 },
      { id: -8, schema_id: 'comic', field_key: 'authors', semantic: 'authors', field_type: 'builtin', label: 'Authors', options: null, form_visible: true, card_visible: true, sortable: false, filterable: false, required: false, order_index: 2 },
      { id: -9, schema_id: 'comic', field_key: 'cover', semantic: 'cover', field_type: 'builtin', label: 'Cover', options: null, form_visible: true, card_visible: false, sortable: false, filterable: false, required: false, order_index: 3 },
    ],
    pipeline_steps: [
      { id: -3, schema_id: 'comic', step_key: 'filename', label: 'Filename extraction', enabled: true, order_index: 0 },
      { id: -4, schema_id: 'comic', step_key: 'cover_pick', label: 'Cover detection', enabled: true, order_index: 1 },
      { id: -5, schema_id: 'comic', step_key: 'filename_folder', label: 'Folder filename extraction', enabled: true, order_index: 2 },
    ],
  },
  {
    id: 'galgame',
    name: 'Galgame',
    icon: null,
    description: null,
    accepted_extensions: ['zip', '7z', 'rar'],
    pipeline_template: 'galgame',
    is_builtin: true,
    sort_order: 2,
    created_at: '',
    updated_at: '',
    fields: [
      { id: -10, schema_id: 'galgame', field_key: 'display_name', semantic: 'display_name', field_type: 'builtin', label: 'Display Name', options: null, form_visible: true, card_visible: false, sortable: false, filterable: false, required: false, order_index: 0 },
      { id: -11, schema_id: 'galgame', field_key: 'category', semantic: 'category', field_type: 'builtin', label: 'Category', options: null, form_visible: true, card_visible: false, sortable: false, filterable: false, required: false, order_index: 1 },
      { id: -12, schema_id: 'galgame', field_key: 'authors', semantic: 'authors', field_type: 'builtin', label: 'Authors', options: null, form_visible: true, card_visible: true, sortable: false, filterable: false, required: false, order_index: 2 },
      { id: -13, schema_id: 'galgame', field_key: 'cover', semantic: 'cover', field_type: 'builtin', label: 'Cover', options: null, form_visible: true, card_visible: false, sortable: false, filterable: false, required: false, order_index: 3 },
    ],
    pipeline_steps: [
      { id: -6, schema_id: 'galgame', step_key: 'filename', label: 'Filename extraction', enabled: true, order_index: 0 },
    ],
  },
];

/** The definitions to read from: the loaded store when available, the
 *  static built-in copy otherwise. Non-reactive by design — components
 *  that need live updates subscribe via `useSchemas()` and re-render;
 *  these helpers then see the fresh state. */
function definitions(): ReadonlyArray<SchemaDefinition> {
  const { schemas, loaded } = schemaStore.state;
  return loaded ? schemas : BUILTIN_FALLBACK;
}

/** Every schema definition, in `sort_order`. Components rendering
 *  option lists should prefer `useSchemas()` for reactivity and pass
 *  the result down. */
export function allSchemas(): ReadonlyArray<SchemaDefinition> {
  return definitions();
}

export function schemaDefBySlug(slug: string | null | undefined): SchemaDefinition | undefined {
  return definitions().find((d) => d.id === slug);
}

/** Display label for a schema slug. Falls back to the raw slug so a
 *  not-yet-known custom schema still renders something sensible. */
export function schemaLabel(slug: string | null | undefined): string {
  return schemaDefBySlug(slug)?.name ?? slug ?? '';
}

/** Enabled prompt steps for a schema, in order. Mirrors the old
 *  PROMPT_STEPS_BY_SCHEMA lookup; unknown slugs get an empty list. */
export function promptStepsFor(
  slug: string | null | undefined
): ReadonlyArray<{ step: PromptStep; label: string }> {
  const def = schemaDefBySlug(slug);
  if (!def) return [];
  return def.pipeline_steps
    .filter((s) => s.enabled)
    .map((s) => ({ step: s.step_key as PromptStep, label: s.label }));
}

/** Project a definition into the render-time view the form and card
 *  components consume. Custom fields (keys outside the known built-in
 *  sets) are skipped here — the renderers only know built-in semantics
 *  until the schema-editor milestone adds custom-field widgets. */
function toCategorySchema(def: SchemaDefinition): CategorySchema {
  const sorted = [...def.fields].sort((a, b) => a.order_index - b.order_index);
  return {
    slug: def.id,
    formFields: sorted
      .filter((f) => f.form_visible && FORM_FIELD_KEYS.has(f.field_key))
      .map((f) => f.field_key as FormFieldKey),
    cardFields: sorted
      .filter((f) => f.card_visible && CARD_FIELD_KEYS.has(f.field_key))
      .map((f) => f.field_key as CardFieldKey),
    acceptedExtensions: def.accepted_extensions,
    fieldDefs: sorted.filter((f) => f.form_visible),
  };
}

/** The fallback slug when a value doesn't match any known schema —
 *  the lowest-`sort_order` definition ('novel' for the seeds, 'novel'
 *  again for the empty store). Mirrors the historical coerce-to-novel
 *  behavior so stale rows keep rendering. */
function defaultSchemaSlug(): string {
  return definitions()[0]?.id ?? 'novel';
}

/** Safe slug coercion: known slugs pass through, anything else
 *  collapses to the default. A stale slug from a row written by a
 *  newer binary doesn't crash render paths. */
export function coerceSchemaSlug(raw: string | null | undefined): string {
  if (raw != null && definitions().some((d) => d.id === raw)) return raw;
  return defaultSchemaSlug();
}

/** Resolve a schema by slug, falling back to the default schema for
 *  unknown values. */
export function schemaBySlug(slug: string | null | undefined): CategorySchema {
  const def = schemaDefBySlug(slug) ?? schemaDefBySlug(defaultSchemaSlug());
  // The store could theoretically be loaded-but-empty; never return
  // undefined — fall back to the static novel definition.
  return toCategorySchema(def ?? BUILTIN_FALLBACK[0]!);
}

export function defaultSchema(): CategorySchema {
  return schemaBySlug(defaultSchemaSlug());
}

/** Resolve the schema for a category, with a safe fallback. */
export function schemaForCategory(
  category: Category | null | undefined
): CategorySchema {
  if (!category) return defaultSchema();
  return schemaBySlug(category.schema_slug);
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
 *  in any schema's `accepted_extensions` set — the file picker treats
 *  that as "not importable" and skips the path. When several schemas
 *  share an extension, the lowest `sort_order` wins. */
export function schemaForPath(
  path: string | null | undefined
): CategorySchema | null {
  const ext = (path ?? '').split('.').pop()?.toLowerCase() ?? '';
  for (const def of definitions()) {
    if (def.accepted_extensions.includes(ext)) return toCategorySchema(def);
  }
  return null;
}

/** True if the extension is in any known schema. The drag-drop
 *  handler uses this to silently skip unsupported paths. */
export function isImportable(path: string | null | undefined): boolean {
  return schemaForPath(path) !== null;
}

/** First category in `categories` that uses the given schema slug.
 *  Used at import time to auto-pick a default category when the LLM
 *  didn't choose one. Returns null if the user has no category with
 *  that schema. */
export function defaultCategoryIdForSchema(
  slug: string,
  categories: ReadonlyArray<Category>
): number | null {
  const match = categories.find((c) => coerceSchemaSlug(c.schema_slug) === slug);
  return match?.id ?? null;
}
