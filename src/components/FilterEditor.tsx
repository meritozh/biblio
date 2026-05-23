import { useCallback, useEffect, useMemo, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Plus, X } from 'lucide-react';
import {
  FIELD_LABELS,
  FILE_STATUS_OPTIONS,
  OPS_BY_FIELD,
  OP_LABELS,
  STORAGE_KIND_OPTIONS,
  newCondition,
  withField,
  withOp,
  type Condition,
  type Field,
  type Op,
} from '@/lib/filters';
import { PaginatedPicker, type PickerPage } from '@/components/PaginatedPicker';
import { authorList, tagList } from '@/lib/tauri';
import type { FileStatus, StorageKind, Tag } from '@/types';

interface FilterEditorProps {
  conditions: ReadonlyArray<Condition>;
  onConditionsChange: (next: Condition[]) => void;
  tags: ReadonlyArray<Tag>;
  authors?: ReadonlyArray<{ id: number; name: string }>;
  /** Opt into buffered mode: every edit stages a local draft, and the
   *  parent's `onConditionsChange` is only called when the user clicks
   *  Apply. Cancel discards the draft. Both buttons fire `onClose` so
   *  the caller can close the surrounding popover. Default `false`
   *  preserves the live-propagate behavior used by the inline editor
   *  on the categories settings page. */
  bufferUntilApply?: boolean;
  /** Fired on Apply or Cancel in buffered mode. Used by popover callers
   *  to close the floating panel after the user commits. */
  onClose?: () => void;
}

const FIELD_KEYS = Object.keys(FIELD_LABELS) as Field[];

export function FilterEditor({
  conditions,
  onConditionsChange,
  tags,
  authors = [],
  bufferUntilApply = false,
  onClose,
}: FilterEditorProps) {
  // Buffered mode stages edits locally; the parent only sees the final
  // set on Apply. Live mode (the default) treats the prop array as the
  // single source of truth and writes through on every edit.
  const [draft, setDraft] = useState<Condition[]>(() => conditions.slice());

  // Re-sync the draft when the parent's `conditions` change while the
  // editor is mounted — typically only fires when the popover re-opens
  // after a prior commit. Cheap to compute; safe to overwrite a stale
  // draft because the parent only mutates `conditions` via our own
  // Apply path in buffered mode.
  useEffect(() => {
    if (bufferUntilApply) setDraft(conditions.slice());
  }, [bufferUntilApply, conditions]);

  const working = bufferUntilApply ? draft : conditions;

  const commit = (next: Condition[]) => {
    if (bufferUntilApply) {
      setDraft(next);
    } else {
      onConditionsChange(next);
    }
  };

  const updateAt = (idx: number, next: Condition) => {
    commit(working.map((c, i) => (i === idx ? next : c)));
  };
  const removeAt = (idx: number) => {
    commit(working.filter((_, i) => i !== idx));
  };
  const addCondition = () => {
    commit([...working, newCondition('tags')]);
  };
  const clearAll = () => {
    commit([]);
  };

  const handleApply = () => {
    onConditionsChange(draft);
    onClose?.();
  };
  const handleCancel = () => {
    setDraft(conditions.slice());
    onClose?.();
  };

  return (
    <div className="space-y-3 w-[460px]">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          Filter conditions
        </span>
        {working.length > 0 && (
          <button
            type="button"
            onClick={clearAll}
            className="text-xs text-muted-foreground hover:text-foreground underline-offset-2 hover:underline"
          >
            Clear all
          </button>
        )}
      </div>
      {working.length === 0 ? (
        <p className="text-xs text-muted-foreground py-2">
          No filters yet. Add a condition to narrow the library.
        </p>
      ) : (
        <ul className="space-y-2">
          {working.map((c, i) => (
            <li key={c.id}>
              <ConditionRow
                condition={c}
                onChange={(next) => updateAt(i, next)}
                onRemove={() => removeAt(i)}
                tags={tags}
                authors={authors}
              />
            </li>
          ))}
        </ul>
      )}
      <div className="pt-1">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-7 text-xs"
          onClick={addCondition}
        >
          <Plus className="h-3.5 w-3.5 mr-1" />
          Add condition
        </Button>
      </div>
      {bufferUntilApply && (
        <div className="flex items-center justify-end gap-2 pt-2 border-t">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-8 text-xs"
            onClick={handleCancel}
          >
            Cancel
          </Button>
          <Button
            type="button"
            size="sm"
            className="h-8 text-xs"
            onClick={handleApply}
          >
            Apply
          </Button>
        </div>
      )}
    </div>
  );
}

interface ConditionRowProps {
  condition: Condition;
  onChange: (next: Condition) => void;
  onRemove: () => void;
  tags: ReadonlyArray<Tag>;
  authors: ReadonlyArray<{ id: number; name: string }>;
}

function ConditionRow({ condition, onChange, onRemove, tags, authors }: ConditionRowProps) {
  return (
    <div className="flex items-center gap-1.5">
      <Select
        value={condition.field}
        onValueChange={(v) => onChange(withField(condition, v as Field))}
      >
        <SelectTrigger className="h-8 w-[110px] text-xs shrink-0">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {FIELD_KEYS.map((f) => (
            <SelectItem key={f} value={f} className="text-xs">
              {FIELD_LABELS[f]}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <Select
        value={condition.op}
        onValueChange={(v) => onChange(withOp(condition, v as Op))}
      >
        <SelectTrigger className="h-8 w-[140px] text-xs shrink-0">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {OPS_BY_FIELD[condition.field].map((op) => (
            <SelectItem key={op} value={op} className="text-xs">
              {OP_LABELS[op]}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <ValueEditor condition={condition} onChange={onChange} tags={tags} authors={authors} />

      <Button
        type="button"
        variant="ghost"
        size="icon"
        className="h-8 w-8 shrink-0"
        onClick={onRemove}
        aria-label="Remove condition"
      >
        <X className="h-3.5 w-3.5" />
      </Button>
    </div>
  );
}

interface ValueEditorProps {
  condition: Condition;
  onChange: (next: Condition) => void;
  tags: ReadonlyArray<Tag>;
  authors: ReadonlyArray<{ id: number; name: string }>;
}

/** Dispatches to the right input shape by (field, op). Operators with no
 *  value (`empty` / `not_empty`) render an inert spacer so the row's other
 *  controls keep stable widths. */
function ValueEditor({ condition: c, onChange, tags, authors }: ValueEditorProps) {
  if (c.field === 'authors' && (c.op === 'count_gte' || c.op === 'count_lt')) {
    return (
      <Input
        type="number"
        min={0}
        value={c.n ?? ''}
        onChange={(e) => {
          const raw = e.target.value;
          const num = Number(raw);
          const next = raw === '' || Number.isNaN(num) ? undefined : Math.max(0, num);
          onChange({ ...c, n: next });
        }}
        className="h-8 flex-1 text-xs"
        placeholder="N"
      />
    );
  }

  if (c.field === 'authors' && c.op === 'includes') {
    const name =
      c.authorId !== undefined
        ? authors.find((a) => a.id === c.authorId)?.name ?? `#${c.authorId}`
        : null;
    return (
      <SinglePickerButton
        kind="author"
        label={name ?? 'pick an author…'}
        muted={name == null}
        selectedId={c.authorId}
        onSelect={(id) => onChange({ ...c, authorId: id })}
      />
    );
  }

  if (c.field === 'tags') {
    if (c.op === 'count_gte' || c.op === 'count_lt') {
      return (
        <Input
          type="number"
          min={0}
          value={c.n ?? ''}
          onChange={(e) =>
            onChange({
              ...c,
              n: e.target.value === '' ? undefined : Math.max(0, Number(e.target.value)),
            })
          }
          className="h-8 flex-1 text-xs"
          placeholder="N"
        />
      );
    }
    if (c.op === 'includes' || c.op === 'excludes') {
      const name =
        c.tagId !== undefined
          ? tags.find((t) => t.id === c.tagId)?.name ?? `#${c.tagId}`
          : null;
      return (
        <SinglePickerButton
          kind="tag"
          label={name ?? 'pick a tag…'}
          muted={name == null}
          selectedId={c.tagId}
          onSelect={(id) => onChange({ ...c, tagId: id })}
        />
      );
    }
    if (c.op === 'includes_any' || c.op === 'excludes_any') {
      // Multi-tag picker: reuses PaginatedPicker so the dropdown stays
      // smooth at scale. Trigger button surfaces the count + first
      // picked tag's name as a preview.
      const previewName =
        c.tagIds.length > 0
          ? tags.find((t) => t.id === c.tagIds[0])?.name ?? `#${c.tagIds[0]}`
          : null;
      return (
        <MultiTagPicker
          tagIds={c.tagIds}
          previewName={previewName}
          onChange={(next) => onChange({ ...c, tagIds: next })}
        />
      );
    }
  }

  if (c.field === 'progress' && c.op === 'contains') {
    return (
      <Input
        value={c.text ?? ''}
        onChange={(e) => onChange({ ...c, text: e.target.value })}
        placeholder="text…"
        className="h-8 flex-1 text-xs"
      />
    );
  }

  if (c.field === 'file_status') {
    return (
      <Select
        value={c.value ?? undefined}
        onValueChange={(v) => onChange({ ...c, value: v as FileStatus })}
      >
        <SelectTrigger className="h-8 flex-1 text-xs">
          <SelectValue placeholder="pick…" />
        </SelectTrigger>
        <SelectContent>
          {FILE_STATUS_OPTIONS.map((opt) => (
            <SelectItem key={opt.value} value={opt.value} className="text-xs">
              {opt.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    );
  }

  if (c.field === 'storage_kind') {
    return (
      <Select
        value={c.value ?? undefined}
        onValueChange={(v) => onChange({ ...c, value: v as StorageKind })}
      >
        <SelectTrigger className="h-8 flex-1 text-xs">
          <SelectValue placeholder="pick…" />
        </SelectTrigger>
        <SelectContent>
          {STORAGE_KIND_OPTIONS.map((opt) => (
            <SelectItem key={opt.value} value={opt.value} className="text-xs">
              {opt.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    );
  }

  return <div className="flex-1" aria-hidden="true" />;
}

// ── Picker sub-components ─────────────────────────────────────────────────────

interface SinglePickerButtonProps {
  kind: 'tag' | 'author';
  /** Text shown on the button. When `muted` is true, render in
   *  muted-foreground so the placeholder reads visually distinct from a
   *  picked name. */
  label: string;
  muted: boolean;
  selectedId?: number;
  onSelect: (id: number) => void;
}

/** Popover trigger + paginated single-pick body for tag/author
 *  filter rows. Picking auto-closes (single-pick semantics). */
function SinglePickerButton({
  kind,
  label,
  muted,
  selectedId,
  onSelect,
}: SinglePickerButtonProps) {
  const [open, setOpen] = useState(false);
  const fetcher = useCallback(
    async ({ query, offset, limit }: { query: string; offset: number; limit: number }): Promise<PickerPage> => {
      if (kind === 'tag') {
        const { tags: page } = await tagList({
          limit,
          offset,
          nameQuery: query.length > 0 ? query : undefined,
        });
        const total = page.length < limit ? offset + page.length : offset + page.length + 1;
        return {
          items: page.map((t) => ({ id: t.id, name: t.name, color: t.color })),
          total,
        };
      }
      const { authors: page } = await authorList({
        limit,
        offset,
        nameQuery: query.length > 0 ? query : undefined,
      });
      const total = page.length < limit ? offset + page.length : offset + page.length + 1;
      return {
        items: page.map((a) => ({ id: a.id, name: a.name })),
        total,
      };
    },
    [kind]
  );
  const selectedIds = useMemo(
    () => (selectedId !== undefined ? [selectedId] : []),
    [selectedId]
  );
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          className="h-8 flex-1 text-xs justify-start font-normal"
        >
          <span className={muted ? 'text-muted-foreground' : undefined}>
            {label}
          </span>
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" sideOffset={4} className="p-0">
        <PaginatedPicker
          mode="single"
          selectedIds={selectedIds}
          fetcher={fetcher}
          onSelect={(id) => {
            onSelect(id);
            setOpen(false);
          }}
          searchPlaceholder={kind === 'tag' ? 'Search tags…' : 'Search authors…'}
          emptyLabel={kind === 'tag' ? 'No tags defined' : 'No authors defined'}
          noMatchLabel={kind === 'tag' ? 'No matching tags' : 'No matching authors'}
        />
      </PopoverContent>
    </Popover>
  );
}

interface MultiTagPickerProps {
  tagIds: number[];
  previewName: string | null;
  onChange: (next: number[]) => void;
}

/** Multi-pick tag chooser for `tags includes_any` / `excludes_any`. */
function MultiTagPicker({ tagIds, previewName, onChange }: MultiTagPickerProps) {
  const fetcher = useCallback(
    async ({ query, offset, limit }: { query: string; offset: number; limit: number }): Promise<PickerPage> => {
      const { tags: page } = await tagList({
        limit,
        offset,
        nameQuery: query.length > 0 ? query : undefined,
      });
      const total = page.length < limit ? offset + page.length : offset + page.length + 1;
      return {
        items: page.map((t) => ({ id: t.id, name: t.name, color: t.color })),
        total,
      };
    },
    []
  );
  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          className="h-8 flex-1 text-xs justify-start font-normal"
        >
          {tagIds.length === 0 ? (
            <span className="text-muted-foreground">pick tags…</span>
          ) : tagIds.length === 1 ? (
            previewName
          ) : (
            `${previewName} +${tagIds.length - 1}`
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" sideOffset={4} className="p-0">
        <PaginatedPicker
          mode="multi"
          selectedIds={tagIds}
          fetcher={fetcher}
          onToggle={onChange}
          searchPlaceholder="Search tags…"
          emptyLabel="No tags defined"
          noMatchLabel="No matching tags"
        />
      </PopoverContent>
    </Popover>
  );
}

