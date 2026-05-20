/**
 * Per-category view defaults.
 *
 * Each `Category` row carries an opaque `view_config` JSON blob. This file
 * is the only place that knows the shape. `resolveViewConfig` reads it
 * back (with schema-slug fallbacks from `categorySchema.ts::REGISTRY`)
 * and `serializeViewConfig` writes it out for the categories page form.
 *
 * Forward compatibility: every field is optional. An older client reading
 * a row written by a newer client ignores fields it doesn't understand
 * instead of failing the whole row. Adding a field is a pure additive
 * change — old rows just keep using their existing defaults.
 */

import type { Category } from '@/types';
import type { SortKey } from '@/stores';
import type { Condition } from '@/lib/filters';

/** Future-compat list — comic categories will surface a real view-mode
 *  toggle in a follow-up; the field is declared now so persisted configs
 *  written today round-trip correctly when the toggle ships. */
export type CategoryViewMode = 'flat' | 'author' | 'name_prefix';

/** The on-disk shape of a `view_config` JSON blob. All fields optional:
 *  a missing key means "fall back to the hard-coded default below". */
export interface CategoryViewConfig {
  view_mode?: CategoryViewMode;
  sort?: { by: SortKey; desc: boolean };
  conditions?: Condition[];
  /** Per-category "Open with" override for `cache_open`. Format is
   *  platform-dependent: macOS app name (e.g. "iA Writer") or bundle id,
   *  Windows full `.exe` path, Linux command name in PATH. Empty / unset
   *  → OS default for the file's extension. The backend resolves this
   *  in `cache_open`; callers stay one-arg (`cacheOpen(fileId)`). */
  open_app?: string;
}

/** Fully-resolved view config — every field populated, ready for the
 *  HomePage to seed its state from. */
export interface ResolvedViewConfig {
  viewMode: CategoryViewMode;
  sortBy: SortKey;
  sortDesc: boolean;
  conditions: Condition[];
}

const HARDCODED_DEFAULTS: ResolvedViewConfig = {
  viewMode: 'flat',
  sortBy: 'name',
  sortDesc: false,
  conditions: [],
};

/** Parse the JSON blob, tolerating malformed input. A stored config that
 *  fails to parse falls back to `{}` rather than throwing — the file list
 *  still renders, the user just sees the schema defaults until they
 *  resave from the categories page. */
export function parseViewConfig(raw: string | null | undefined): CategoryViewConfig {
  if (!raw) return {};
  try {
    const parsed = JSON.parse(raw);
    return typeof parsed === 'object' && parsed !== null ? (parsed as CategoryViewConfig) : {};
  } catch {
    return {};
  }
}

/** Serialize for the Tauri bridge. Returns `undefined` when the resolved
 *  config exactly matches schema defaults — keeps the DB column NULL for
 *  unconfigured categories so a future change to schema defaults still
 *  flows through to them automatically. */
export function serializeViewConfig(
  config: CategoryViewConfig
): string | undefined {
  // Trim out empty/equivalent-to-default fields so the blob is minimal.
  const trimmed: CategoryViewConfig = {};
  if (config.view_mode && config.view_mode !== 'flat') {
    trimmed.view_mode = config.view_mode;
  }
  if (config.sort && (config.sort.by !== 'name' || config.sort.desc)) {
    trimmed.sort = config.sort;
  }
  if (config.conditions && config.conditions.length > 0) {
    trimmed.conditions = config.conditions;
  }
  if (config.open_app && config.open_app.trim().length > 0) {
    trimmed.open_app = config.open_app.trim();
  }
  if (Object.keys(trimmed).length === 0) return undefined;
  return JSON.stringify(trimmed);
}

/** Resolve a category to a full view config: per-row JSON overrides first,
 *  hard-coded defaults second. Safe to call with `null` (no category
 *  selected) — returns the hard-coded defaults. */
export function resolveViewConfig(
  category: Category | null | undefined
): ResolvedViewConfig {
  const overrides = parseViewConfig(category?.view_config);
  return {
    viewMode: overrides.view_mode ?? HARDCODED_DEFAULTS.viewMode,
    sortBy: overrides.sort?.by ?? HARDCODED_DEFAULTS.sortBy,
    sortDesc: overrides.sort?.desc ?? HARDCODED_DEFAULTS.sortDesc,
    conditions: overrides.conditions ?? HARDCODED_DEFAULTS.conditions,
  };
}

/** Convenience: look up the category by id from a list, then resolve. */
export function resolveViewConfigById(
  categoryId: number | null | undefined,
  categories: ReadonlyArray<Category>
): ResolvedViewConfig {
  if (categoryId == null) return resolveViewConfig(null);
  const cat = categories.find((c) => c.id === categoryId);
  return resolveViewConfig(cat ?? null);
}
