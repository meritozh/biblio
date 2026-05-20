import { createFileRoute } from '@tanstack/react-router';
import { useState, useEffect, useMemo } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
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
import { ArrowDown, ArrowUp, Pencil, FolderOpen } from 'lucide-react';
import { categoryUpdate, tagList } from '@/lib/tauri';
import { loadCategories, useAppState } from '@/stores/appStore';
import { SCHEMA_LABELS, coerceSchemaSlug } from '@/lib/categorySchema';
import {
  parseViewConfig,
  resolveViewConfig,
  serializeViewConfig,
  type CategoryViewConfig,
  type CategoryViewMode,
} from '@/lib/categoryViewConfig';
import { FilterEditor } from '@/components/FilterEditor';
import type { SortKey } from '@/stores';
import type { Category, SchemaSlug, Tag } from '@/types';

export const Route = createFileRoute('/categories')({
  component: CategoriesPage,
});

// ── Form state shared across Create and Edit dialogs ─────────────────────────

interface CategoryFormState {
  name: string;
  description: string;
  schemaSlug: SchemaSlug;
  viewConfig: CategoryViewConfig;
}

const EMPTY_FORM: CategoryFormState = {
  name: '',
  description: '',
  schemaSlug: 'novel',
  viewConfig: {},
};

const SORT_OPTIONS: ReadonlyArray<{ value: SortKey; label: string }> = [
  { value: 'name', label: 'Name' },
  { value: 'created', label: 'Date added' },
  { value: 'updated', label: 'Date updated' },
];

// Comic-only. Novels keep the flat grid because the collection endpoint
// (`comic_collection_list`) only knows how to group comic-schema files.
const VIEW_MODE_OPTIONS: ReadonlyArray<{ value: CategoryViewMode; label: string }> = [
  { value: 'flat', label: 'All files' },
  { value: 'author', label: 'By author' },
  { value: 'name_prefix', label: 'By series' },
];

function CategoryFormFields({
  values,
  onChange,
  availableTags,
}: {
  values: CategoryFormState;
  onChange: (next: CategoryFormState) => void;
  availableTags: ReadonlyArray<Tag>;
}) {
  // Resolve the effective sort + view mode so the form always shows a
  // concrete current value, even when fields in `viewConfig` are absent
  // (meaning "fall back to the hard-coded defaults from the resolver").
  const effective = useMemo(
    () =>
      resolveViewConfig({
        // Synthesize a minimal Category-shaped object for the resolver
        // — it only reads `view_config`.
        id: 0,
        name: '',
        description: null,
        icon: null,
        is_default: false,
        folder_name: null,
        schema_slug: values.schemaSlug,
        view_config: JSON.stringify(values.viewConfig),
        created_at: '',
      }),
    [values.schemaSlug, values.viewConfig]
  );

  const patchViewConfig = (patch: Partial<CategoryViewConfig>) => {
    onChange({ ...values, viewConfig: { ...values.viewConfig, ...patch } });
  };

  return (
    <div className="space-y-5 py-4">
      <div>
        <label className="text-sm font-medium mb-2 block">Name</label>
        <Input
          value={values.name}
          onChange={(e) => onChange({ ...values, name: e.target.value })}
          placeholder="Category name"
        />
      </div>

      <div>
        <label className="text-sm font-medium mb-2 block">Description</label>
        <Input
          value={values.description}
          onChange={(e) => onChange({ ...values, description: e.target.value })}
          placeholder="e.g., Chinese web novels, light novels, manga..."
        />
        <p className="text-xs text-muted-foreground mt-1.5">
          Helps the LLM pick the right category during import.
        </p>
      </div>

      <div>
        <label className="text-sm font-medium mb-2 block">Schema</label>
        <Select
          value={values.schemaSlug}
          onValueChange={(v) => onChange({ ...values, schemaSlug: v as SchemaSlug })}
        >
          <SelectTrigger className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {(Object.keys(SCHEMA_LABELS) as SchemaSlug[]).map((slug) => (
              <SelectItem key={slug} value={slug}>
                {SCHEMA_LABELS[slug]}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <p className="text-xs text-muted-foreground mt-1.5">
          Decides which form fields appear in import / edit dialogs, what
          the file card shows, and which prompts the LLM runs.
        </p>
      </div>

      <div className="pt-2 border-t">
        <div className="flex items-baseline justify-between mb-3">
          <h3 className="text-sm font-medium">View defaults</h3>
          <span className="text-xs text-muted-foreground font-serif-italic">
            Applied when you open this category in Library
          </span>
        </div>

        {/* ── View mode (comic-only) ─────────────────────────────────── */}
        {values.schemaSlug === 'comic' && (
          <div className="mb-4">
            <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-2 block">
              Default view mode
            </label>
            <Select
              value={effective.viewMode}
              onValueChange={(v) =>
                patchViewConfig({ view_mode: v as CategoryViewMode })
              }
            >
              <SelectTrigger className="h-9 w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {VIEW_MODE_OPTIONS.map((opt) => (
                  <SelectItem key={opt.value} value={opt.value}>
                    {opt.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground mt-1.5">
              How the file list opens for this category. "By author" / "By
              series" collapse the grid into collection cards.
            </p>
          </div>
        )}

        {/* ── Sort ────────────────────────────────────────────────────── */}
        <div className="mb-4">
          <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-2 block">
            Default sort
          </label>
          <div className="flex items-center gap-2">
            <Select
              value={effective.sortBy}
              onValueChange={(v) =>
                patchViewConfig({
                  sort: { by: v as SortKey, desc: effective.sortDesc },
                })
              }
            >
              <SelectTrigger className="h-9 flex-1">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {SORT_OPTIONS.map((opt) => (
                  <SelectItem key={opt.value} value={opt.value}>
                    {opt.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-9"
              onClick={() =>
                patchViewConfig({
                  sort: { by: effective.sortBy, desc: !effective.sortDesc },
                })
              }
              title={effective.sortDesc ? 'Descending' : 'Ascending'}
            >
              {effective.sortDesc ? (
                <ArrowDown className="h-4 w-4" />
              ) : (
                <ArrowUp className="h-4 w-4" />
              )}
            </Button>
          </div>
        </div>

        {/* ── Default filters ─────────────────────────────────────────── */}
        <div>
          <FilterEditor
            conditions={effective.conditions}
            onConditionsChange={(next) =>
              patchViewConfig({ conditions: next.length === 0 ? undefined : next })
            }
            tags={availableTags}
          />
        </div>
      </div>

      {/* ── Open behavior ───────────────────────────────────────────────── */}
      <div className="pt-2 border-t">
        <div className="flex items-baseline justify-between mb-3">
          <h3 className="text-sm font-medium">Open behavior</h3>
          <span className="text-xs text-muted-foreground font-serif-italic">
            Used by the Open action on file cards
          </span>
        </div>
        <div>
          <label className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-2 block">
            Open with
          </label>
          <Input
            value={values.viewConfig.open_app ?? ''}
            onChange={(e) => patchViewConfig({ open_app: e.target.value })}
            placeholder={openAppPlaceholder()}
          />
          <p className="text-xs text-muted-foreground mt-1.5">
            Leave empty to use the system default app. Format is
            platform-specific: macOS app name or bundle id ({'"'}iA Writer{'"'},
            {' "'}com.apple.Preview{'"'}); Windows full path to the .exe; Linux
            command name in PATH.
          </p>
        </div>
      </div>
    </div>
  );
}

/** Hint for the "Open with" input. Platform detection is just for the
 *  placeholder copy — the value the user enters is stored verbatim and
 *  passed straight to `tauri_plugin_opener`. */
function openAppPlaceholder(): string {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes('mac')) return 'iA Writer';
  if (ua.includes('win')) return 'C:\\Program Files\\AppName\\app.exe';
  return 'app-command';
}

function CategoriesPage() {
  const categories = useAppState((s) => s.categories);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editForm, setEditForm] = useState<CategoryFormState>(EMPTY_FORM);
  const [tags, setTags] = useState<Tag[]>([]);

  useEffect(() => {
    void loadCategories();
    // Tags drive the FilterEditor's includes/excludes dropdowns — load
    // once when the page mounts. Empty list is fine; the editor just
    // hides the tag-specific operators.
    tagList()
      .then(({ tags }) => setTags(tags))
      .catch((err) => console.error('Failed to load tags:', err));
  }, []);

  const handleStartEdit = (category: Category) => {
    setEditingId(category.id);
    setEditForm({
      name: category.name,
      description: category.description ?? '',
      schemaSlug: coerceSchemaSlug(category.schema_slug),
      viewConfig: parseViewConfig(category.view_config),
    });
  };

  const handleSaveEdit = async () => {
    if (!editingId || !editForm.name.trim()) return;
    const serialized = serializeViewConfig(editForm.viewConfig);
    try {
      await categoryUpdate({
        id: editingId,
        name: editForm.name.trim(),
        description: editForm.description.trim() || undefined,
        schemaSlug: editForm.schemaSlug,
        // `undefined` means "no override → clear back to schema defaults"
        // when there's also no inline JSON. Use `clearViewConfig` to make
        // that intent explicit on the backend.
        viewConfig: serialized,
        clearViewConfig: serialized === undefined,
      });
      setEditingId(null);
      setEditForm(EMPTY_FORM);
      void loadCategories();
    } catch (error) {
      console.error('Failed to update category:', error);
      alert(`Failed to update category: ${error}`);
    }
  };

  const handleCancelEdit = () => {
    setEditingId(null);
    setEditForm(EMPTY_FORM);
  };

  return (
    <>
      <div
        className="flex items-end justify-between px-8 pt-14 pb-5 border-b border-border"
        data-tauri-drag-region
      >
        <div className="flex items-baseline gap-3">
          <h1 className="text-3xl text-foreground flex items-center gap-3">
            <FolderOpen className="h-6 w-6 text-muted-foreground" aria-hidden="true" />
            Categories
          </h1>
          <span className="font-serif-italic text-sm text-muted-foreground">
            — {categories.length} {categories.length === 1 ? 'category' : 'categories'}
          </span>
        </div>
      </div>

      <div className="flex-1 overflow-auto px-8 py-6">
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Description</TableHead>
                <TableHead className="w-[120px]">Schema</TableHead>
                <TableHead className="w-[150px]">Folder</TableHead>
                <TableHead className="w-[140px]">View defaults</TableHead>
                <TableHead className="w-[100px]">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {categories.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6} className="h-24 text-center">
                    No categories seeded yet.
                  </TableCell>
                </TableRow>
              ) : (
                categories.map((category) => (
                  <TableRow key={category.id}>
                    <TableCell>
                      <div className="flex items-center gap-2">
                        {category.name}
                        {category.is_default && (
                          <Badge variant="secondary" className="text-xs">
                            Default
                          </Badge>
                        )}
                      </div>
                    </TableCell>
                    <TableCell>
                      <span className="text-xs text-muted-foreground">
                        {category.description || '-'}
                      </span>
                    </TableCell>
                    <TableCell>
                      <Badge variant="gray" className="text-xs">
                        {SCHEMA_LABELS[coerceSchemaSlug(category.schema_slug)]}
                      </Badge>
                    </TableCell>
                    <TableCell>
                      <span className="text-xs text-muted-foreground font-mono">
                        {category.folder_name || '-'}
                      </span>
                    </TableCell>
                    <TableCell>
                      <span className="text-xs text-muted-foreground">
                        {summarizeViewConfig(category)}
                      </span>
                    </TableCell>
                    <TableCell>
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-8 w-8"
                        onClick={() => handleStartEdit(category)}
                        aria-label={`Edit ${category.name}`}
                      >
                        <Pencil className="h-4 w-4" />
                      </Button>
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      </div>

      <Dialog
        open={editingId !== null}
        onOpenChange={(open) => {
          if (!open) handleCancelEdit();
        }}
      >
        <DialogContent className="max-w-lg max-h-[90vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>Edit Category</DialogTitle>
          </DialogHeader>
          <CategoryFormFields
            values={editForm}
            onChange={setEditForm}
            availableTags={tags}
          />
          <DialogFooter>
            <Button variant="outline" onClick={handleCancelEdit}>
              Cancel
            </Button>
            <Button onClick={handleSaveEdit} disabled={!editForm.name.trim()}>
              Save
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

    </>
  );
}

/** Short text for the View defaults table column: "—" when the category
 *  uses pure schema defaults, otherwise a compact "Sort: name↑ · Filters: 2"
 *  summary the user can scan at a glance. */
function summarizeViewConfig(category: Category): string {
  const cfg = parseViewConfig(category.view_config);
  const parts: string[] = [];
  if (cfg.view_mode && cfg.view_mode !== 'flat') {
    const opt = VIEW_MODE_OPTIONS.find((o) => o.value === cfg.view_mode);
    parts.push(opt?.label ?? cfg.view_mode);
  }
  if (cfg.sort) {
    const opt = SORT_OPTIONS.find((o) => o.value === cfg.sort!.by);
    parts.push(`${opt?.label ?? cfg.sort.by}${cfg.sort.desc ? ' ↓' : ' ↑'}`);
  }
  if (cfg.conditions && cfg.conditions.length > 0) {
    parts.push(`${cfg.conditions.length} filter${cfg.conditions.length === 1 ? '' : 's'}`);
  }
  return parts.length === 0 ? '—' : parts.join(' · ');
}
