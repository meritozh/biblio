import { createFileRoute } from '@tanstack/react-router';
import { useMemo, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
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
import { Pencil, Plus, Shapes, Trash2, Upload, Download } from 'lucide-react';
import { open, save } from '@tauri-apps/plugin-dialog';
import { loadSchemas, useSchemas } from '@/stores/schemaStore';
import {
  schemaCreate,
  schemaDelete,
  schemaExport,
  schemaImportRead,
  schemaStepsUpdate,
  schemaUpdate,
} from '@/lib/schemaBridge';
import {
  draftForTemplate,
  draftFromDefinition,
  draftToPayload,
  type SchemaDraft,
} from '@/lib/schemaDraft';
import { SchemaEditorForm } from '@/components/schemas/SchemaEditorForm';
import type { SchemaDefinition } from '@/types';

export const Route = createFileRoute('/schemas')({
  component: SchemasPage,
});

/** Dialog filter for exported schema files (both pickers). */
const SCHEMA_FILE_FILTER = [{ name: 'Biblio Schema', extensions: ['json'] }];

/** Human-readable messages for the backend's import error codes; raw
 *  codes are terse ("IMPORT_NOT_A_SCHEMA_FILE") and would surface in
 *  alerts unexplained. */
function describeImportError(error: unknown): string {
  const message = String(error);
  if (message.startsWith('IMPORT_READ_FAILED:')) return `Could not read the file: ${message}`;
  if (message.startsWith('IMPORT_NOT_A_SCHEMA_FILE'))
    return 'This file is not a Biblio schema export.';
  if (message.startsWith('IMPORT_UNSUPPORTED_VERSION:'))
    return `This export uses format version ${message.split(':')[1]}, which this app cannot import.`;
  return `Failed to import schema: ${message}`;
}

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

  const handleExport = async (def: SchemaDefinition) => {
    const path = await save({
      defaultPath: `${def.id}.schema.json`,
      filters: SCHEMA_FILE_FILTER,
    });
    if (!path) return;
    try {
      await schemaExport(def.id, path);
      alert(`Exported "${def.name}" to ${path}`);
    } catch (error) {
      alert(`Failed to export schema: ${error}`);
    }
  };

  const handleImport = async () => {
    const picked = await open({
      multiple: false,
      directory: false,
      filters: SCHEMA_FILE_FILTER,
    });
    if (typeof picked !== 'string') return;
    try {
      const def = await schemaImportRead(picked);
      const draft = draftFromDefinition(def);
      // Nothing is persisted yet: every imported row is removable
      // without a data-loss warning.
      draft.fields = draft.fields.map((f) => ({ ...f, isNew: true }));
      setEditing(null);
      setDraft(draft);
      setEditorOpen(true);
    } catch (error) {
      alert(describeImportError(error));
    }
  };

  const handleSave = async () => {
    if (!draft) return;
    setSaving(true);
    try {
      const payload = draftToPayload(draft);
      const savedId = editing ? editing.id : draft.id.trim();
      if (editing) {
        await schemaUpdate(editing.id, payload);
      } else {
        await schemaCreate(savedId, payload);
      }
      // Steps ride a separate command — they're replaced wholesale,
      // so this covers both toggles and reordering.
      await schemaStepsUpdate(
        savedId,
        draft.steps.map((s, index) => ({
          step_key: s.step_key,
          label: s.label,
          enabled: s.enabled,
          order_index: index,
        }))
      );
      await loadSchemas();
      setEditorOpen(false);
      setDraft(null);
      setEditing(null);
    } catch (error) {
      const message = String(error);
      if (message === 'SCHEMA_ID_EXISTS') {
        alert(
          `A schema with slug "${editing ? editing.id : draft.id.trim()}" already exists. Pick a different slug.`
        );
      } else {
        alert(`Failed to save schema: ${error}`);
      }
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
        <div className="flex items-center gap-2">
          <Button variant="outline" onClick={() => void handleImport()} className="gap-2">
            <Upload className="h-4 w-4" aria-hidden="true" />
            Import
          </Button>
          <Button onClick={handleStartCreate} className="gap-2">
            <Plus className="h-4 w-4" aria-hidden="true" />
            New Schema
          </Button>
        </div>
      </div>

      <div className="flex-1 overflow-auto px-8 py-6">
        <p className="text-xs text-muted-foreground mb-4 max-w-2xl">
          A schema defines a resource type: which files it accepts, which fields
          its files carry, and which import pipeline runs. Categories bind to a
          schema; files inherit it from their category. Built-in schemas can be
          edited but not deleted. Export shares a schema as a JSON file (fields
          and pipeline steps; prompts fall back to the template's).
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
                <TableHead className="w-[130px]">Actions</TableHead>
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
                        onClick={() => void handleExport(def)}
                        aria-label={`Export ${def.name}`}
                        title={`Export ${def.name} as JSON`}
                      >
                        <Download className="h-4 w-4" />
                      </Button>
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
