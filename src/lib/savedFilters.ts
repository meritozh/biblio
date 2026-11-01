/**
 * Named filter presets persisted in localStorage.
 *
 * Presets are global (not per-category): a "Remote + uncached" shortcut
 * is useful across categories. Conditions keep their existing shape so
 * Apply just replaces the active editor set.
 */

import type { Condition } from '@/lib/filters';

export interface SavedFilter {
  id: string;
  name: string;
  conditions: Condition[];
  createdAt: string;
  updatedAt: string;
}

const STORAGE_KEY = 'biblio.saved-filters';

function makeId(): string {
  return `sf-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

function readAll(): SavedFilter[] {
  if (typeof window === 'undefined') return [];
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (item): item is SavedFilter =>
        item != null &&
        typeof item === 'object' &&
        typeof item.id === 'string' &&
        typeof item.name === 'string' &&
        Array.isArray(item.conditions)
    );
  } catch {
    return [];
  }
}

function writeAll(presets: SavedFilter[]): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(presets));
}

export function listSavedFilters(): SavedFilter[] {
  return readAll().sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
}

export function saveFilterPreset(name: string, conditions: Condition[]): SavedFilter {
  const trimmed = name.trim();
  if (!trimmed) {
    throw new Error('Filter name is required');
  }
  if (conditions.length === 0) {
    throw new Error('Cannot save an empty filter');
  }

  const now = new Date().toISOString();
  const presets = readAll();
  const existing = presets.find((p) => p.name.toLowerCase() === trimmed.toLowerCase());

  if (existing) {
    const updated: SavedFilter = {
      ...existing,
      name: trimmed,
      // Fresh ids for conditions so React keys stay unique if the same
      // preset is applied while a previous instance is still mounted.
      conditions: conditions.map((c) => ({ ...c })),
      updatedAt: now,
    };
    writeAll(presets.map((p) => (p.id === existing.id ? updated : p)));
    return updated;
  }

  const created: SavedFilter = {
    id: makeId(),
    name: trimmed,
    conditions: conditions.map((c) => ({ ...c })),
    createdAt: now,
    updatedAt: now,
  };
  writeAll([created, ...presets]);
  return created;
}

export function deleteSavedFilter(id: string): void {
  writeAll(readAll().filter((p) => p.id !== id));
}

/** Clone conditions with fresh ids so applying a preset never collides
 *  with ids already present in the live editor. */
export function clonePresetConditions(conditions: Condition[]): Condition[] {
  const stamp = Date.now().toString(36);
  return conditions.map((c, i) => ({ ...c, id: `${c.id}-apply-${stamp}-${i}` }));
}
