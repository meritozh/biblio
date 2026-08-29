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
  return rows.map(normalizeDefinition);
}

/** Field payload for create/update. Keys are snake_case to match the
 *  Rust serde shape verbatim (Tauri passes nested objects through
 *  unchanged). */
export interface SchemaFieldInput {
  field_key: string;
  semantic: string | null;
  field_type: string;
  label: string;
  options: string | null;
  form_visible: boolean;
  card_visible: boolean;
  sortable: boolean;
  filterable: boolean;
  required: boolean;
  order_index: number;
}

export interface SchemaUpsertPayload {
  name: string;
  icon: string | null;
  description: string | null;
  accepted_extensions: string[];
  pipeline_template: string;
  fields: SchemaFieldInput[];
}

/** Create a custom schema. `id` is the slug — immutable afterwards,
 *  must match `^[a-z][a-z0-9_]*$`. Pipeline steps are copied from the
 *  chosen template. */
export async function schemaCreate(
  id: string,
  payload: SchemaUpsertPayload
): Promise<SchemaDefinition> {
  const row = await invoke<SchemaDefinitionWire>('schema_create', { id, payload });
  return normalizeDefinition(row);
}

/** Update basic info + replace the field list. Semantic fields must
 *  all survive; removed custom fields lose their stored values (warn
 *  via `schemaFieldDataCount` first). */
export async function schemaUpdate(
  id: string,
  payload: SchemaUpsertPayload
): Promise<SchemaDefinition> {
  const row = await invoke<SchemaDefinitionWire>('schema_update', { id, payload });
  return normalizeDefinition(row);
}

/** Delete a custom schema. Backend rejects built-ins
 *  (`SCHEMA_BUILTIN_NOT_DELETABLE`) and schemas still referenced by
 *  categories (`SCHEMA_IN_USE:<count>`). */
export async function schemaDelete(id: string): Promise<void> {
  return invoke('schema_delete', { id });
}

/** Number of metadata values a field holds across files under this
 *  schema — the "N files will lose this value" warning count. */
export async function schemaFieldDataCount(
  schemaId: string,
  fieldKey: string
): Promise<number> {
  return invoke('schema_field_data_count', { schemaId, fieldKey });
}

export interface SchemaStepInput {
  step_key: string;
  label: string;
  enabled: boolean;
  order_index: number;
}

/** Replace a schema's pipeline steps (toggle + reorder). `enabled` is
 *  functional — pipeline nodes skip disabled steps at runtime;
 *  `order_index` drives display order here and on the Prompts page. */
export async function schemaStepsUpdate(
  id: string,
  steps: SchemaStepInput[]
): Promise<SchemaDefinition> {
  const row = await invoke<SchemaDefinitionWire>('schema_steps_update', { id, steps });
  return normalizeDefinition(row);
}

/** Serialize a schema (basic info + fields + steps; prompts are not
 *  included — an imported schema falls back to its template's prompts)
 *  and write it to `path`, which should come from a native save
 *  dialog. */
export async function schemaExport(id: string, path: string): Promise<void> {
  return invoke('schema_export', { id, path });
}

/** Read and validate an exported schema file. Returns the definition
 *  *unpersisted* — the caller prefills the schema editor (create mode);
 *  actual creation rides `schemaCreate` + `schemaStepsUpdate`. */
export async function schemaImportRead(path: string): Promise<SchemaDefinition> {
  const row = await invoke<SchemaDefinitionWire>('schema_import_read', { path });
  return normalizeDefinition(row);
}

function normalizeDefinition(row: SchemaDefinitionWire): SchemaDefinition {
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
}
