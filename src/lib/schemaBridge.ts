/**
 * IPC bridge for the runtime-defined schema commands. Lives outside
 * `tauri.ts` because that file is at the repo's 800-line guard
 * (tests/unit/sourceFileSize.test.ts) — new bridge domains get their
 * own file.
 */

import { invoke } from '@tauri-apps/api/core';
import type { SchemaDefinition } from '@/types';

/** Wire shape from `schema_list`: `accepted_extensions` is the raw JSON
 *  string column; everything else matches `SchemaDefinition`. */
type SchemaDefinitionWire = Omit<SchemaDefinition, 'accepted_extensions'> & {
  accepted_extensions: string;
};

/** All runtime-defined schemas with their fields and pipeline steps,
 *  ordered by `sort_order`. Drives the schema store that replaced the
 *  hard-coded frontend REGISTRY. */
export async function schemaList(): Promise<SchemaDefinition[]> {
  const rows = await invoke<SchemaDefinitionWire[]>('schema_list');
  return rows.map((row) => {
    let extensions: string[] = [];
    try {
      const parsed = JSON.parse(row.accepted_extensions);
      if (Array.isArray(parsed)) {
        extensions = parsed.filter((e): e is string => typeof e === 'string');
      }
    } catch {
      // Malformed JSON → treat as "no extensions" rather than failing
      // the whole load; the schema still works via category-first import.
    }
    return { ...row, accepted_extensions: extensions };
  });
}
