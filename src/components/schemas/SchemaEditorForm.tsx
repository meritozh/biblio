import { useMemo, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { ArrowDown, ArrowUp, Plus, Trash2 } from 'lucide-react';
import { schemaFieldDataCount } from '@/lib/schemaBridge';
import {
  CUSTOM_FIELD_TYPES,
  TEMPLATES,
  draftForTemplate,
  type FieldDraft,
  type SchemaDraft,
  type StepDraft,
} from '@/lib/schemaDraft';
import type { SchemaDefinition } from '@/types';

export function SchemaEditorForm({
  draft,
  onChange,
  isCreate,
  schemas,
}: {
  draft: SchemaDraft;
  onChange: (next: SchemaDraft) => void;
  isCreate: boolean;
  schemas: ReadonlyArray<SchemaDefinition>;
}) {
  const patch = (p: Partial<SchemaDraft>) => onChange({ ...draft, ...p });
  const patchField = (index: number, p: Partial<FieldDraft>) => {
    const fields = draft.fields.map((f, i) => (i === index ? { ...f, ...p } : f));
    patch({ fields });
  };
  const moveField = (index: number, delta: -1 | 1) => {
    const target = index + delta;
    if (target < 0 || target >= draft.fields.length) return;
    const fields = [...draft.fields];
    const [row] = fields.splice(index, 1);
    fields.splice(target, 0, row!);
    patch({ fields });
  };
  const patchStep = (index: number, p: Partial<StepDraft>) => {
    const steps = draft.steps.map((s, i) => (i === index ? { ...s, ...p } : s));
    patch({ steps });
  };
  const moveStep = (index: number, delta: -1 | 1) => {
    const target = index + delta;
    if (target < 0 || target >= draft.steps.length) return;
    const steps = [...draft.steps];
    const [row] = steps.splice(index, 1);
    steps.splice(target, 0, row!);
    patch({ steps });
  };

  // Field removal confirmation state: holds the index plus the affected
  // file count once fetched.
  const [removal, setRemoval] = useState<{ index: number; count: number } | null>(null);

  const requestRemoveField = async (index: number) => {
    const field = draft.fields[index]!;
    if (field.isNew) {
      patch({ fields: draft.fields.filter((_, i) => i !== index) });
      return;
    }
    try {
      const count = await schemaFieldDataCount(draft.id, field.field_key);
      if (count === 0) {
        patch({ fields: draft.fields.filter((_, i) => i !== index) });
      } else {
        setRemoval({ index, count });
      }
    } catch (error) {
      alert(`Failed to check field data: ${error}`);
    }
  };

  const confirmRemoveField = () => {
    if (!removal) return;
    patch({ fields: draft.fields.filter((_, i) => i !== removal.index) });
    setRemoval(null);
  };

  // New-custom-field mini form.
  const [newKey, setNewKey] = useState('');
  const [newLabel, setNewLabel] = useState('');
  const [newType, setNewType] = useState('text');
  const [newOptions, setNewOptions] = useState('');

  const addField = () => {
    const key = newKey.trim();
    if (!key || !newLabel.trim()) return;
    if (draft.fields.some((f) => f.field_key === key)) {
      alert(`Field "${key}" already exists.`);
      return;
    }
    patch({
      fields: [
        ...draft.fields,
        {
          field_key: key,
          semantic: null,
          field_type: newType,
          label: newLabel.trim(),
          optionsText: newType === 'enum' ? newOptions : null,
          form_visible: true,
          card_visible: false,
          sortable: false,
          filterable: false,
          required: false,
          isNew: true,
        },
      ],
    });
    setNewKey('');
    setNewLabel('');
    setNewType('text');
    setNewOptions('');
  };

  return (
    <div className="space-y-5 py-4">
      {/* ── Basic info ─────────────────────────────────────────────── */}
      <div className="grid grid-cols-2 gap-4">
        <div>
          <Label className="text-sm font-medium mb-2 block">Name</Label>
          <Input
            value={draft.name}
            onChange={(e) => patch({ name: e.target.value })}
            placeholder="e.g. Podcast"
          />
        </div>
        <div>
          <Label className="text-sm font-medium mb-2 block">Slug</Label>
          <Input
            value={draft.id}
            onChange={(e) => patch({ id: e.target.value })}
            placeholder="e.g. podcast"
            disabled={!isCreate}
            className="font-mono"
          />
          <p className="text-xs text-muted-foreground mt-1.5">
            {isCreate
              ? 'Lowercase letters, digits, underscores; starts with a letter. Immutable after creation.'
              : 'The slug is immutable — categories and prompts reference it.'}
          </p>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div>
          <Label className="text-sm font-medium mb-2 block">Pipeline template</Label>
          <Select
            value={draft.pipeline_template}
            onValueChange={(v) => {
              if (isCreate) {
                // Re-seed the field + step lists from the new template.
                onChange({ ...draftForTemplate(v, schemas), id: draft.id, name: draft.name });
              } else {
                patch({ pipeline_template: v });
              }
            }}
          >
            <SelectTrigger className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {TEMPLATES.map((t) => (
                <SelectItem key={t.value} value={t.value}>
                  {t.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <p className="text-xs text-muted-foreground mt-1.5">
            Which built-in import pipeline (and its prompts) files under
            this schema run through.
          </p>
        </div>
        <div>
          <Label className="text-sm font-medium mb-2 block">Accepted extensions</Label>
          <Input
            value={draft.extensionsText}
            onChange={(e) => patch({ extensionsText: e.target.value })}
            placeholder="e.g. mp3, m4a"
            className="font-mono"
          />
          <p className="text-xs text-muted-foreground mt-1.5">
            Comma-separated, without dots. Used to route files when no
            category is chosen yet.
          </p>
          <ExtensionConflicts draft={draft} schemas={schemas} />
        </div>
      </div>

      <div>
        <Label className="text-sm font-medium mb-2 block">Description</Label>
        <Input
          value={draft.description}
          onChange={(e) => patch({ description: e.target.value })}
          placeholder="Optional"
        />
      </div>

      {/* ── Field designer ─────────────────────────────────────────── */}
      <div className="pt-2 border-t">
        <div className="flex items-baseline justify-between mb-3">
          <h3 className="text-sm font-medium">Fields</h3>
          <span className="text-xs text-muted-foreground">
            Order here is the form render order
          </span>
        </div>

        <div className="space-y-2">
          {draft.fields.map((field, index) => (
            <div
              key={field.field_key}
              className="flex items-center gap-2 rounded-lg border border-border px-3 py-2"
            >
              <div className="flex flex-col">
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-4 w-4"
                  disabled={index === 0}
                  onClick={() => moveField(index, -1)}
                  aria-label="Move up"
                >
                  <ArrowUp className="h-3 w-3" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-4 w-4"
                  disabled={index === draft.fields.length - 1}
                  onClick={() => moveField(index, 1)}
                  aria-label="Move down"
                >
                  <ArrowDown className="h-3 w-3" />
                </Button>
              </div>
              <div className="flex-1 min-w-0">
                <Input
                  value={field.label}
                  onChange={(e) => patchField(index, { label: e.target.value })}
                  className="h-8 text-sm"
                />
              </div>
              <span className="text-xs text-muted-foreground font-mono w-28 truncate">
                {field.field_key}
              </span>
              <Badge variant={field.semantic ? 'secondary' : 'gray'} className="text-xs w-20 justify-center">
                {field.semantic ? field.semantic : field.field_type}
              </Badge>
              <label className="flex items-center gap-1 text-xs text-muted-foreground">
                <Switch
                  checked={field.form_visible}
                  onCheckedChange={(v) => patchField(index, { form_visible: v })}
                  aria-label="Show in form"
                />
                Form
              </label>
              <label className="flex items-center gap-1 text-xs text-muted-foreground">
                <Switch
                  checked={field.card_visible}
                  onCheckedChange={(v) => patchField(index, { card_visible: v })}
                  aria-label="Show on card"
                />
                Card
              </label>
              {field.semantic === null && (
                <>
                  <label className="flex items-center gap-1 text-xs text-muted-foreground">
                    <Switch
                      checked={field.sortable}
                      onCheckedChange={(v) => patchField(index, { sortable: v })}
                      aria-label="Sortable"
                    />
                    Sort
                  </label>
                  <label className="flex items-center gap-1 text-xs text-muted-foreground">
                    <Switch
                      checked={field.filterable}
                      onCheckedChange={(v) => patchField(index, { filterable: v })}
                      aria-label="Filterable"
                    />
                    Filter
                  </label>
                  <Button
                    size="icon"
                    variant="ghost"
                    className="h-8 w-8 text-destructive"
                    onClick={() => void requestRemoveField(index)}
                    aria-label={`Remove ${field.field_key}`}
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </>
              )}
            </div>
          ))}
        </div>

        {/* Add custom field */}
        <div className="mt-4 rounded-lg border border-dashed border-border p-3">
          <p className="text-xs font-medium text-muted-foreground mb-2">Add custom field</p>
          <div className="flex items-center gap-2">
            <Input
              value={newKey}
              onChange={(e) => setNewKey(e.target.value)}
              placeholder="key (e.g. publisher)"
              className="h-8 text-sm font-mono w-40"
            />
            <Input
              value={newLabel}
              onChange={(e) => setNewLabel(e.target.value)}
              placeholder="Label"
              className="h-8 text-sm flex-1"
            />
            <Select value={newType} onValueChange={setNewType}>
              <SelectTrigger className="h-8 w-28">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {CUSTOM_FIELD_TYPES.map((t) => (
                  <SelectItem key={t.value} value={t.value}>
                    {t.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button
              size="sm"
              variant="outline"
              className="h-8"
              onClick={addField}
              disabled={!newKey.trim() || !newLabel.trim()}
            >
              <Plus className="h-4 w-4" />
            </Button>
          </div>
          {newType === 'enum' && (
            <Input
              value={newOptions}
              onChange={(e) => setNewOptions(e.target.value)}
              placeholder="Options, comma-separated (e.g. ongoing, completed)"
              className="h-8 text-sm mt-2"
            />
          )}
        </div>
      </div>

      {/* ── Pipeline steps ─────────────────────────────────────────── */}
      <div className="pt-2 border-t">
        <div className="flex items-baseline justify-between mb-3">
          <h3 className="text-sm font-medium">Pipeline steps</h3>
          <span className="text-xs text-muted-foreground">
            Disabled steps are skipped at import; order drives the Prompts page
          </span>
        </div>
        <div className="space-y-2">
          {draft.steps.map((step, index) => (
            <div
              key={step.step_key}
              className="flex items-center gap-2 rounded-lg border border-border px-3 py-2"
            >
              <div className="flex flex-col">
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-4 w-4"
                  disabled={index === 0}
                  onClick={() => moveStep(index, -1)}
                  aria-label="Move step up"
                >
                  <ArrowUp className="h-3 w-3" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-4 w-4"
                  disabled={index === draft.steps.length - 1}
                  onClick={() => moveStep(index, 1)}
                  aria-label="Move step down"
                >
                  <ArrowDown className="h-3 w-3" />
                </Button>
              </div>
              <span className="text-sm flex-1">{step.label}</span>
              <span className="text-xs text-muted-foreground font-mono">{step.step_key}</span>
              <Switch
                checked={step.enabled}
                onCheckedChange={(v) => patchStep(index, { enabled: v })}
                aria-label={`Enable ${step.label}`}
              />
            </div>
          ))}
          {draft.steps.length === 0 && (
            <p className="text-xs text-muted-foreground">
              No steps — pick a pipeline template above to seed the default set.
            </p>
          )}
        </div>
      </div>

      {/* Field-removal confirmation */}
      <AlertDialog open={removal !== null} onOpenChange={(open) => !open && setRemoval(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Remove field</AlertDialogTitle>
            <AlertDialogDescription>
              {removal && (
                <>
                  {removal.count} file{removal.count === 1 ? '' : 's'} have a value stored
                  for "{draft.fields[removal.index]?.label}". Removing the field deletes
                  that data permanently when you save.
                </>
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Keep field</AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => {
                e.preventDefault();
                confirmRemoveField();
              }}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              Remove and delete data
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

/** Warns when the draft's extensions overlap with other schemas.
 *  Extension routing (no category picked) resolves to the schema with
 *  the lowest `sort_order`, so overlaps silently shadow the rest. */
function ExtensionConflicts({
  draft,
  schemas,
}: {
  draft: SchemaDraft;
  schemas: ReadonlyArray<SchemaDefinition>;
}) {
  const conflicts = useMemo(() => {
    const mine = new Set(
      draft.extensionsText
        .split(',')
        .map((e) => e.trim().replace(/^\./, '').toLowerCase())
        .filter((e) => e.length > 0)
    );
    if (mine.size === 0) return [];
    const out: { ext: string; others: string[] }[] = [];
    for (const ext of mine) {
      const others = schemas
        .filter((d) => d.id !== draft.id && d.accepted_extensions.includes(ext))
        .map((d) => d.name);
      if (others.length > 0) out.push({ ext, others });
    }
    return out;
  }, [draft.extensionsText, draft.id, schemas]);

  if (conflicts.length === 0) return null;
  return (
    <div className="mt-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 dark:border-amber-900 dark:bg-amber-950/30">
      {conflicts.map(({ ext, others }) => (
        <p key={ext} className="text-xs text-amber-800 dark:text-amber-200">
          .{ext} is also used by {others.join(', ')} — when no category is
          picked at import, the schema with the lower sort order wins.
        </p>
      ))}
    </div>
  );
}
