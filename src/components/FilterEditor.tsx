import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
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
import type { FileStatus, StorageKind, Tag } from '@/types';

interface FilterEditorProps {
  conditions: ReadonlyArray<Condition>;
  onConditionsChange: (next: Condition[]) => void;
  tags: ReadonlyArray<Tag>;
}

const FIELD_KEYS = Object.keys(FIELD_LABELS) as Field[];

export function FilterEditor({ conditions, onConditionsChange, tags }: FilterEditorProps) {
  const updateAt = (idx: number, next: Condition) => {
    onConditionsChange(conditions.map((c, i) => (i === idx ? next : c)));
  };
  const removeAt = (idx: number) => {
    onConditionsChange(conditions.filter((_, i) => i !== idx));
  };
  const addCondition = () => {
    onConditionsChange([...conditions, newCondition('tags')]);
  };

  return (
    <div className="space-y-3 w-[460px]">
      <div className="flex items-center justify-between">
        <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          Filter conditions
        </span>
        {conditions.length > 0 && (
          <button
            type="button"
            onClick={() => onConditionsChange([])}
            className="text-xs text-muted-foreground hover:text-foreground underline-offset-2 hover:underline"
          >
            Clear all
          </button>
        )}
      </div>
      {conditions.length === 0 ? (
        <p className="text-xs text-muted-foreground py-2">
          No filters yet. Add a condition to narrow the library.
        </p>
      ) : (
        <ul className="space-y-2">
          {conditions.map((c, i) => (
            <li key={c.id}>
              <ConditionRow
                condition={c}
                onChange={(next) => updateAt(i, next)}
                onRemove={() => removeAt(i)}
                tags={tags}
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
    </div>
  );
}

interface ConditionRowProps {
  condition: Condition;
  onChange: (next: Condition) => void;
  onRemove: () => void;
  tags: ReadonlyArray<Tag>;
}

function ConditionRow({ condition, onChange, onRemove, tags }: ConditionRowProps) {
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

      <ValueEditor condition={condition} onChange={onChange} tags={tags} />

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
}

/** Dispatches to the right input shape by (field, op). Operators with no
 *  value (`empty` / `not_empty`) render an inert spacer so the row's other
 *  controls keep stable widths. */
function ValueEditor({ condition: c, onChange, tags }: ValueEditorProps) {
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
      return (
        <Select
          value={c.tagId !== undefined ? String(c.tagId) : undefined}
          onValueChange={(v) => onChange({ ...c, tagId: Number(v) })}
        >
          <SelectTrigger className="h-8 flex-1 text-xs">
            <SelectValue placeholder="pick a tag…" />
          </SelectTrigger>
          <SelectContent>
            {tags.length === 0 ? (
              <div className="text-xs text-muted-foreground p-2">No tags defined</div>
            ) : (
              tags.map((t) => (
                <SelectItem key={t.id} value={String(t.id)} className="text-xs">
                  {t.name}
                </SelectItem>
              ))
            )}
          </SelectContent>
        </Select>
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
