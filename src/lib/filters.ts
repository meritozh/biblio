import type { FileEntry, FileStatus, StorageKind, Tag } from '@/types';

// ── Condition shape ──────────────────────────────────────────────────────────
//
// A discriminated union: each row in the filter editor produces one Condition.
// Value-bearing fields are optional so the UI can render a half-built row
// without immediately emptying the grid; the evaluator treats `undefined` as
// a no-op (the predicate returns true) until the user picks a value.

export type Field =
  | 'authors'
  | 'tags'
  | 'progress'
  | 'file_status'
  | 'storage_kind';

export type Condition =
  | { id: string; field: 'authors'; op: 'empty' }
  | { id: string; field: 'authors'; op: 'not_empty' }
  | { id: string; field: 'authors'; op: 'count_gte'; n?: number }
  | { id: string; field: 'authors'; op: 'count_lt'; n?: number }
  | { id: string; field: 'tags'; op: 'empty' }
  | { id: string; field: 'tags'; op: 'not_empty' }
  | { id: string; field: 'tags'; op: 'count_gte'; n?: number }
  | { id: string; field: 'tags'; op: 'count_lt'; n?: number }
  | { id: string; field: 'tags'; op: 'includes'; tagId?: number }
  | { id: string; field: 'tags'; op: 'excludes'; tagId?: number }
  | { id: string; field: 'progress'; op: 'empty' }
  | { id: string; field: 'progress'; op: 'not_empty' }
  | { id: string; field: 'progress'; op: 'contains'; text?: string }
  | { id: string; field: 'file_status'; op: 'is'; value?: FileStatus }
  | { id: string; field: 'storage_kind'; op: 'is'; value?: StorageKind };

export type Op = Condition['op'];

// ── Catalog (drives the editor's dropdowns and chip rendering) ───────────────

export const FIELD_LABELS: Record<Field, string> = {
  authors: 'Authors',
  tags: 'Tags',
  progress: 'Progress',
  file_status: 'Status',
  storage_kind: 'Storage',
};

export const OP_LABELS: Record<Op, string> = {
  empty: 'is empty',
  not_empty: 'is not empty',
  count_gte: 'has at least',
  count_lt: 'has fewer than',
  includes: 'includes',
  excludes: 'excludes',
  contains: 'contains',
  is: 'is',
};

export const OPS_BY_FIELD: Record<Field, ReadonlyArray<Op>> = {
  authors: ['empty', 'not_empty', 'count_gte', 'count_lt'],
  tags: ['empty', 'not_empty', 'count_gte', 'count_lt', 'includes', 'excludes'],
  progress: ['empty', 'not_empty', 'contains'],
  file_status: ['is'],
  storage_kind: ['is'],
};

export const FILE_STATUS_OPTIONS: ReadonlyArray<{ value: FileStatus; label: string }> = [
  { value: 'available', label: 'Available' },
  { value: 'missing', label: 'Missing' },
  { value: 'moved', label: 'Moved' },
];

export const STORAGE_KIND_OPTIONS: ReadonlyArray<{ value: StorageKind; label: string }> = [
  { value: 'local', label: 'Local' },
  { value: 'remote', label: 'Remote' },
];

// ── Builders ─────────────────────────────────────────────────────────────────

let _idCounter = 1;
export function makeId(): string {
  return `c${_idCounter++}`;
}

/** Fresh condition with a sensible default op when the user picks a field. */
export function newCondition(field: Field): Condition {
  const id = makeId();
  switch (field) {
    case 'authors':
      return { id, field, op: 'not_empty' };
    case 'tags':
      return { id, field, op: 'not_empty' };
    case 'progress':
      return { id, field, op: 'not_empty' };
    case 'file_status':
      return { id, field, op: 'is' };
    case 'storage_kind':
      return { id, field, op: 'is' };
  }
}

/** Replace a condition's field; resets op + value to defaults for the new field
 *  so the union stays internally consistent. */
export function withField(c: Condition, field: Field): Condition {
  return { ...newCondition(field), id: c.id };
}

/** Replace a condition's op; preserves field. Value is preserved across ops
 *  that share the same value shape (e.g. `count_gte` ↔ `count_lt` keep `n`,
 *  `includes` ↔ `excludes` keep `tagId`); otherwise the value is dropped so
 *  the union stays internally consistent. */
export function withOp(c: Condition, op: Op): Condition {
  if (!OPS_BY_FIELD[c.field].includes(op)) return c;

  switch (c.field) {
    case 'authors':
      switch (op) {
        case 'empty':
        case 'not_empty':
          return { id: c.id, field: 'authors', op };
        case 'count_gte':
        case 'count_lt':
          return { id: c.id, field: 'authors', op, n: 'n' in c ? c.n : undefined };
        default:
          return c;
      }
    case 'tags':
      switch (op) {
        case 'empty':
        case 'not_empty':
          return { id: c.id, field: 'tags', op };
        case 'count_gte':
        case 'count_lt':
          return { id: c.id, field: 'tags', op, n: 'n' in c ? c.n : undefined };
        case 'includes':
        case 'excludes':
          return {
            id: c.id,
            field: 'tags',
            op,
            tagId: 'tagId' in c ? c.tagId : undefined,
          };
        default:
          return c;
      }
    case 'progress':
      switch (op) {
        case 'empty':
        case 'not_empty':
          return { id: c.id, field: 'progress', op };
        case 'contains':
          return {
            id: c.id,
            field: 'progress',
            op,
            text: 'text' in c ? c.text : undefined,
          };
        default:
          return c;
      }
    case 'file_status':
      return { id: c.id, field: 'file_status', op: 'is', value: c.value };
    case 'storage_kind':
      return { id: c.id, field: 'storage_kind', op: 'is', value: c.value };
  }
}

// ── Evaluation ───────────────────────────────────────────────────────────────

/** True if the condition matches the file. Conditions whose value is missing
 *  return true (no-op) so the user can build a row without immediately
 *  emptying the grid. Each field-specific helper exhausts its operator union
 *  so TypeScript can verify completeness without trailing dead returns. */
function matchAuthors(
  c: Extract<Condition, { field: 'authors' }>,
  file: FileEntry
): boolean {
  const n = file.authors?.length ?? 0;
  switch (c.op) {
    case 'empty':
      return n === 0;
    case 'not_empty':
      return n > 0;
    case 'count_gte':
      return c.n === undefined ? true : n >= c.n;
    case 'count_lt':
      return c.n === undefined ? true : n < c.n;
  }
}

function matchTags(
  c: Extract<Condition, { field: 'tags' }>,
  file: FileEntry
): boolean {
  const tags = file.tags ?? [];
  switch (c.op) {
    case 'empty':
      return tags.length === 0;
    case 'not_empty':
      return tags.length > 0;
    case 'count_gte':
      return c.n === undefined ? true : tags.length >= c.n;
    case 'count_lt':
      return c.n === undefined ? true : tags.length < c.n;
    case 'includes':
      return c.tagId === undefined ? true : tags.some((t) => t.id === c.tagId);
    case 'excludes':
      return c.tagId === undefined ? true : !tags.some((t) => t.id === c.tagId);
  }
}

function matchProgress(
  c: Extract<Condition, { field: 'progress' }>,
  file: FileEntry
): boolean {
  const p = (file.progress ?? '').trim();
  switch (c.op) {
    case 'empty':
      return p === '';
    case 'not_empty':
      return p !== '';
    case 'contains':
      return c.text === undefined || c.text === ''
        ? true
        : p.toLowerCase().includes(c.text.toLowerCase());
  }
}

function matches(c: Condition, file: FileEntry): boolean {
  switch (c.field) {
    case 'authors':
      return matchAuthors(c, file);
    case 'tags':
      return matchTags(c, file);
    case 'progress':
      return matchProgress(c, file);
    case 'file_status':
      return c.value === undefined ? true : file.file_status === c.value;
    case 'storage_kind':
      return c.value === undefined ? true : (file.storage_kind ?? 'local') === c.value;
  }
}

export function applyConditions(files: FileEntry[], conditions: ReadonlyArray<Condition>): FileEntry[] {
  if (conditions.length === 0) return files;
  return files.filter((f) => conditions.every((c) => matches(c, f)));
}

// ── Chip rendering ───────────────────────────────────────────────────────────

function describeAuthors(c: Extract<Condition, { field: 'authors' }>): string {
  const f = FIELD_LABELS.authors;
  switch (c.op) {
    case 'empty':
      return `${f} is empty`;
    case 'not_empty':
      return `${f} not empty`;
    case 'count_gte':
      return `${f} ≥ ${c.n ?? '…'}`;
    case 'count_lt':
      return `${f} < ${c.n ?? '…'}`;
  }
}

function describeTags(
  c: Extract<Condition, { field: 'tags' }>,
  tagsById: Map<number, Tag>
): string {
  const f = FIELD_LABELS.tags;
  switch (c.op) {
    case 'empty':
      return `${f} is empty`;
    case 'not_empty':
      return `${f} not empty`;
    case 'count_gte':
      return `${f} ≥ ${c.n ?? '…'}`;
    case 'count_lt':
      return `${f} < ${c.n ?? '…'}`;
    case 'includes': {
      const name = c.tagId !== undefined ? tagsById.get(c.tagId)?.name : undefined;
      return `${f} includes ${name ?? '…'}`;
    }
    case 'excludes': {
      const name = c.tagId !== undefined ? tagsById.get(c.tagId)?.name : undefined;
      return `${f} excludes ${name ?? '…'}`;
    }
  }
}

function describeProgress(c: Extract<Condition, { field: 'progress' }>): string {
  const f = FIELD_LABELS.progress;
  switch (c.op) {
    case 'empty':
      return `${f} is empty`;
    case 'not_empty':
      return `${f} not empty`;
    case 'contains':
      return c.text ? `${f} contains "${c.text}"` : `${f} contains …`;
  }
}

/** One-line summary used in chips and aria-labels. */
export function describeCondition(c: Condition, tagsById: Map<number, Tag>): string {
  switch (c.field) {
    case 'authors':
      return describeAuthors(c);
    case 'tags':
      return describeTags(c, tagsById);
    case 'progress':
      return describeProgress(c);
    case 'file_status':
      return `${FIELD_LABELS.file_status}: ${c.value ?? '…'}`;
    case 'storage_kind':
      return `${FIELD_LABELS.storage_kind}: ${c.value ?? '…'}`;
  }
}
