import { createFileRoute } from '@tanstack/react-router';
import { useMemo, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
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
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { ArrowDown, ArrowUp, Pencil, Plus, Shapes, Trash2 } from 'lucide-react';
import { loadSchemas, useSchemas } from '@/stores/schemaStore';
import {
  schemaCreate,
  schemaDelete,
  schemaFieldDataCount,
  schemaUpdate,
  type SchemaUpsertPayload,
} from '@/lib/schemaBridge';
import type { SchemaDefinition, SchemaField } from '@/types';

export const Route = createFileRoute('/schemas')({
  component: SchemasPage,
});

// ── Draft model ─────────────────────────────────────────────────────

/** Editable mirror of `SchemaField` / `SchemaFieldInput`. `isNew` marks
 *  rows added in this editing session — they can be removed without a
 *  data-loss warning because nothing is stored yet. */
interface FieldDraft {
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

interface SchemaDraft {
  id: string;
  name: string;
  icon: string;
  description: string;
  /** Comma-separated extensions, normalized on save. */
  extensionsText: string;
  pipeline_template: string;
  fields: FieldDraft[];
}

const CUSTOM_FIELD_TYPES: ReadonlyArray<{ value: string; label: string }> = [
  { value: 'text', label: 'Text' },
  { value: 'number', label: 'Number' },
  { value: 'rating', label: 'Rating' },
  { value: 'date', label: 'Date' },
  { value: 'enum', label: 'Enum' },
  { value: 'bool', label: 'Boolean' },
];

const TEMPLATES: ReadonlyArray<{ value: string; label: string }> = [
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

function draftFromDefinition(def: SchemaDefinition): SchemaDraft {
  return {
    id: def.id,
    name: def.name,
    icon: def.icon ?? '',
    description: def.description ?? '',
    extensionsText: def.accepted_extensions.join(', '),
    pipeline_template: def.pipeline_template,
    fields: def.fields.map(fieldToDraft),
  };
}

/** Starting draft for a new schema: fields copied from the chosen
 *  template's built-in (semantic) field set. */
function draftForTemplate(
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
  };
}

function draftToPayload(draft: SchemaDraft): SchemaUpsertPayload {
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

// ── Page ────────────────────────────────────────────────────────────

function SchemasPage() {
  const schemas = useSchemas();
  const [editorOpen, setEditorOpen] = useState(false);
  const [editing, setEditing] = useState<SchemaDefinition | null>(null);
  const [draft, setDraft] = useState<SchemaDraft | null>(null);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState<SchemaDefinition | null>(null);
  const [deleteBusy, setDeleteBusy] = useState(false);

  const handleStartCreate = () => {
    setEditing(null);
    setDraft(draftForTemplate('novel', schemas));
    setEditorOpen(true);
  };

  const handleStartEdit = (def: SchemaDefinition) => {
    setEditing(def);
    setDraft(draftFromDefinition(def));
    setEditorOpen(true);
  };

  const handleSave = async () => {
    if (!draft) return;
    setSaving(true);
    try {
      const payload = draftToPayload(draft);
      if (editing) {
        await schemaUpdate(editing.id, payload);
      } else {
        await schemaCreate(draft.id.trim(), payload);
      }
      await loadSchemas();
      setEditorOpen(false);
      setDraft(null);
      setEditing(null);
    } catch (error) {
      alert(`Failed to save schema: ${error}`);
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    if (!deleting) return;
    setDeleteBusy(true);
    try {
      await schemaDelete(deleting.id);
      await loadSchemas();
      setDeleting(null);
    } catch (error) {
      const message = String(error);
      if (message.startsWith('SCHEMA_IN_USE:')) {
        const count = message.split(':')[1];
        alert(
          `Cannot delete "${deleting.name}": ${count} categor${count === '1' ? 'y' : 'ies'} still use it. Move those categories to another schema first.`
        );
      } else {
        alert(`Failed to delete schema: ${error}`);
      }
    } finally {
      setDeleteBusy(false);
    }
  };

  const saveDisabled = useMemo(() => {
    if (!draft || saving) return true;
    if (!draft.name.trim() || !draft.extensionsText.trim()) return true;
    if (!editing && !draft.id.trim()) return true;
    return false;
  }, [draft, saving, editing]);

  return (
    <>
      <div
        className="flex items-end justify-between px-8 pt-14 pb-5 border-b border-border"
        data-tauri-drag-region
      >
        <div className="flex items-baseline gap-3">
          <h1 className="text-3xl text-foreground flex items-center gap-3">
            <Shapes className="h-6 w-6 text-muted-foreground" aria-hidden="true" />
            Schemas
          </h1>
          <span className="font-serif-italic text-sm text-muted-foreground">
            — {schemas.length} {schemas.length === 1 ? 'schema' : 'schemas'}
          </span>
        </div>
        <Button onClick={handleStartCreate} className="gap-2">
          <Plus className="h-4 w-4" aria-hidden="true" />
          New Schema
        </Button>
      </div>

      <div className="flex-1 overflow-auto px-8 py-6">
        <p className="text-xs text-muted-foreground mb-4 max-w-2xl">
          A schema defines a resource type: which files it accepts, which fields
          its files carry, and which import pipeline runs. Categories bind to a
          schema; files inherit it from their category. Built-in schemas can be
          edited but not deleted.
        </p>
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead className="w-[140px]">Slug</TableHead>
                <TableHead className="w-[140px]">Pipeline</TableHead>
                <TableHead>Extensions</TableHead>
                <TableHead className="w-[90px]">Fields</TableHead>
                <TableHead className="w-[110px]">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {schemas.map((def) => (
                <TableRow key={def.id}>
                  <TableCell>
                    <div className="flex items-center gap-2">
                      {def.name}
                      {def.is_builtin && (
                        <Badge variant="secondary" className="text-xs">
                          Built-in
                        </Badge>
                      )}
                    </div>
                  </TableCell>
                  <TableCell>
                    <span className="text-xs text-muted-foreground font-mono">{def.id}</span>
                  </TableCell>
                  <TableCell>
                    <Badge variant="gray" className="text-xs">
                      {def.pipeline_template}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    <span className="text-xs text-muted-foreground font-mono">
                      {def.accepted_extensions.map((e) => `.${e}`).join(' ')}
                    </span>
                  </TableCell>
                  <TableCell>
                    <span className="text-xs text-muted-foreground">{def.fields.length}</span>
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center gap-1">
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-8 w-8"
                        onClick={() => handleStartEdit(def)}
                        aria-label={`Edit ${def.name}`}
                      >
                        <Pencil className="h-4 w-4" />
                      </Button>
                      {!def.is_builtin && (
                        <Button
                          size="icon"
                          variant="ghost"
                          className="h-8 w-8 text-destructive"
                          onClick={() => setDeleting(def)}
                          aria-label={`Delete ${def.name}`}
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      )}
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      </div>

      <Dialog
        open={editorOpen}
        onOpenChange={(open) => {
          if (!open && !saving) {
            setEditorOpen(false);
            setDraft(null);
            setEditing(null);
          }
        }}
      >
        <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>{editing ? `Edit ${editing.name}` : 'New Schema'}</DialogTitle>
          </DialogHeader>
          {draft && (
            <SchemaEditorForm
              draft={draft}
              onChange={setDraft}
              isCreate={editing === null}
              schemas={schemas}
            />
          )}
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => {
                setEditorOpen(false);
                setDraft(null);
                setEditing(null);
              }}
              disabled={saving}
            >
              Cancel
            </Button>
            <Button onClick={handleSave} disabled={saveDisabled}>
              {saving ? 'Saving…' : editing ? 'Save' : 'Create'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <AlertDialog open={deleting !== null} onOpenChange={(open) => !open && setDeleting(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete schema</AlertDialogTitle>
            <AlertDialogDescription>
              Delete "{deleting?.name}"? Its prompts are deleted too. Categories using it
              must be moved to another schema first.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={(e) => {
                e.preventDefault();
                void handleDelete();
              }}
              disabled={deleteBusy}
              className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
            >
              {deleteBusy ? 'Deleting…' : 'Delete'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}

// ── Editor form ─────────────────────────────────────────────────────

function SchemaEditorForm({
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
                // Re-seed the field list from the new template.
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
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-8 w-8 text-destructive"
                  onClick={() => void requestRemoveField(index)}
                  aria-label={`Remove ${field.field_key}`}
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
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
