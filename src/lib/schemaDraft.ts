/**
 * Draft model for the schema editor (/schemas). Mirrors the backend
 * wire shapes (`SchemaFieldInput` / `SchemaStepInput`) with UI-only
 * conveniences (comma-separated text fields, `isNew` markers).
 * Split out of `routes/schemas.tsx` to keep every source file under
 * the repo's 800-line guard.
 */

import type { SchemaDefinition, SchemaField } from '@/types';
import type { SchemaUpsertPayload } from '@/lib/schemaBridge';

/** Editable mirror of `SchemaField` / `SchemaFieldInput`. `isNew` marks
 *  rows added in this editing session — they can be removed without a
 *  data-loss warning because nothing is stored yet. */
export interface FieldDraft {
  field_key: string;
  semantic: string | null;
  field_type: string;
  label: string;
  /** Enum options as comma-separated text; serialized to a JSON array
   *  on save. Null for non-enum rows. */
  optionsText: string | null;
  form_visible: boolean;
  card_visible: boolean;
  sortable: boolean;
  filterable: boolean;
  required: boolean;
  isNew: boolean;
}

export interface StepDraft {
  step_key: string;
  label: string;
  enabled: boolean;
}

export interface SchemaDraft {
  id: string;
  name: string;
  icon: string;
  description: string;
  /** Comma-separated extensions, normalized on save. */
  extensionsText: string;
  pipeline_template: string;
  fields: FieldDraft[];
  steps: StepDraft[];
}

export const CUSTOM_FIELD_TYPES: ReadonlyArray<{ value: string; label: string }> = [
  { value: 'text', label: 'Text' },
  { value: 'number', label: 'Number' },
  { value: 'rating', label: 'Rating' },
  { value: 'date', label: 'Date' },
  { value: 'enum', label: 'Enum' },
  { value: 'bool', label: 'Boolean' },
];

export const TEMPLATES: ReadonlyArray<{ value: string; label: string }> = [
  { value: 'novel', label: 'Novel pipeline' },
  { value: 'comic', label: 'Comic pipeline' },
  { value: 'galgame', label: 'Galgame pipeline' },
];

function fieldToDraft(field: SchemaField): FieldDraft {
  let optionsText: string | null = null;
  if (field.options) {
    try {
      const parsed = JSON.parse(field.options);
      if (Array.isArray(parsed)) optionsText = parsed.join(', ');
    } catch {
      optionsText = field.options;
    }
  }
  return {
    field_key: field.field_key,
    semantic: field.semantic,
    field_type: field.field_type,
    label: field.label,
    optionsText,
    form_visible: field.form_visible,
    card_visible: field.card_visible,
    sortable: field.sortable,
    filterable: field.filterable,
    required: field.required,
    isNew: false,
  };
}

export function draftFromDefinition(def: SchemaDefinition): SchemaDraft {
  return {
    id: def.id,
    name: def.name,
    icon: def.icon ?? '',
    description: def.description ?? '',
    extensionsText: def.accepted_extensions.join(', '),
    pipeline_template: def.pipeline_template,
    fields: def.fields.map(fieldToDraft),
    steps: def.pipeline_steps.map((s) => ({
      step_key: s.step_key,
      label: s.label,
      enabled: s.enabled,
    })),
  };
}

/** Starting draft for a new schema: fields copied from the chosen
 *  template's built-in (semantic) field set, steps from its pipeline
 *  steps. */
export function draftForTemplate(
  template: string,
  schemas: ReadonlyArray<SchemaDefinition>
): SchemaDraft {
  const tpl = schemas.find((d) => d.id === template);
  return {
    id: '',
    name: '',
    icon: '',
    description: '',
    extensionsText: '',
    pipeline_template: template,
    fields: tpl ? tpl.fields.filter((f) => f.semantic !== null).map(fieldToDraft) : [],
    steps: tpl
      ? tpl.pipeline_steps.map((s) => ({
          step_key: s.step_key,
          label: s.label,
          enabled: s.enabled,
        }))
      : [],
  };
}

export function draftToPayload(draft: SchemaDraft): SchemaUpsertPayload {
  return {
    name: draft.name.trim(),
    icon: draft.icon.trim() || null,
    description: draft.description.trim() || null,
    accepted_extensions: draft.extensionsText
      .split(',')
      .map((e) => e.trim())
      .filter((e) => e.length > 0),
    pipeline_template: draft.pipeline_template,
    fields: draft.fields.map((f, index) => ({
      field_key: f.field_key,
      semantic: f.semantic,
      field_type: f.field_type,
      label: f.label.trim(),
      options:
        f.field_type === 'enum'
          ? JSON.stringify(
              (f.optionsText ?? '')
                .split(',')
                .map((o) => o.trim())
                .filter((o) => o.length > 0)
            )
          : null,
      form_visible: f.form_visible,
      card_visible: f.card_visible,
      sortable: f.sortable,
      filterable: f.filterable,
      required: f.required,
      order_index: index,
    })),
  };
}
