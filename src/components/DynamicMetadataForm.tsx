import { useEffect, useState } from 'react';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { CategorySelect } from '@/components/CategorySelect';
import { TagManager } from '@/components/TagManager';
import { AuthorManager } from '@/components/AuthorManager';
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
import { Button } from '@/components/ui/button';
import { coverGet, preparedCoverGet } from '@/lib/tauri';
import type { CategorySchema, FormFieldKey } from '@/lib/categorySchema';
import { schemaForCategoryId } from '@/lib/categorySchema';
import type { Category, Tag, Author, MetadataType } from '@/types';

export interface DynamicMetadataFormValues {
  display_name: string;
  category_id: number | null;
  tag_ids: number[];
  author_ids: number[];
  metadata: Array<{ key: string; value: string; data_type: MetadataType }>;
  /** When set, the user uploaded a replacement cover and these bytes
   *  should be written to the DB on save. */
  cover_data?: string;
  cover_mime_type?: string;
  /** When true, the user clicked Remove and the existing DB cover should
   *  be deleted on save. Mutually exclusive with `cover_data`. */
  cover_removed?: boolean;
  /** Token for a cover the Phase-2 pipeline staged in Rust-side memory.
   *  Used only for preview rendering and the commit hand-off; the bytes
   *  never enter `cover_data` so the JS heap stays flat regardless of
   *  how many rows are open in the review dialog. */
  staged_cover_path?: string;
  progress?: string;
}

/** Self-fetches the existing cover from the DB by file id, mirroring the
 *  grid card's CardCover so we don't relay bytes through formValues state.
 *  Renders nothing on rejection (no cover row) or while loading. */
function ExistingCoverPreview({ fileId }: { fileId: number }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    coverGet(fileId)
      .then(({ data, mime_type }) => {
        if (!cancelled) setSrc(`data:${mime_type};base64,${data}`);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [fileId]);
  return src ? (
    <img
      src={src}
      alt="Cover preview"
      className="h-24 w-16 object-cover rounded-md border"
    />
  ) : (
    <div className="h-24 w-16 rounded-md border border-dashed flex items-center justify-center text-xs text-muted-foreground">
      …
    </div>
  );
}

/** Self-fetches a staged cover (Phase-2 pipeline output not yet committed)
 *  from the Rust-side cache and renders it via a Blob URL. The base64 string
 *  from the IPC is converted to bytes and then dropped — the only retained
 *  reference is the blob URL, whose underlying bytes live in the browser's
 *  blob store off the JS heap. Revokes on unmount so memory releases
 *  promptly when the dialog scrolls the row out of view. */
function StagedCoverPreview({ stagedPath }: { stagedPath: string }) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    let createdUrl: string | null = null;
    preparedCoverGet(stagedPath)
      .then(({ data, mime_type }) => {
        if (cancelled) return;
        const binary = atob(data);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
        const blob = new Blob([bytes], { type: mime_type });
        createdUrl = URL.createObjectURL(blob);
        setUrl(createdUrl);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      if (createdUrl) URL.revokeObjectURL(createdUrl);
    };
  }, [stagedPath]);
  return url ? (
    <img
      src={url}
      alt="Cover preview"
      className="h-24 w-16 object-cover rounded-md border"
    />
  ) : (
    <div className="h-24 w-16 rounded-md border border-dashed flex items-center justify-center text-xs text-muted-foreground">
      …
    </div>
  );
}

interface DynamicMetadataFormProps {
  values: DynamicMetadataFormValues;
  onChange: (values: DynamicMetadataFormValues) => void;
  /** The schema whose `formFields` list drives section render order.
   *  Callers can pass either a resolved `CategorySchema` (preferred —
   *  e.g. resolved from `values.category_id`) or a literal field list
   *  for dialogs that don't yet know the category (legacy). The form
   *  watches `values.category_id` and re-resolves the schema from
   *  `categories` when a `schema` isn't passed in directly, so editing
   *  the category in-place re-renders the section list. */
  schema?: CategorySchema;
  fields?: ReadonlyArray<FormFieldKey>;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onTagCreate?: (name: string) => Promise<Tag>;
  onAuthorCreate?: (name: string) => Promise<Author>;
  fileId?: number;
  inStorage?: boolean;
  onCategoryChange?: (newCategoryId: number | null) => Promise<void>;
}

export function DynamicMetadataForm({
  values,
  onChange,
  schema,
  fields,
  categories,
  tags,
  authors,
  onTagCreate,
  onAuthorCreate,
  fileId,
  inStorage,
  onCategoryChange,
}: DynamicMetadataFormProps) {
  // If the caller didn't pin a schema, resolve from the current
  // category_id. This makes the form reactive to category changes:
  // pick a different category in the dropdown → next render uses the
  // new schema's `formFields`. Falls back to the literal `fields` list
  // for legacy callers that pre-date the schema model.
  const resolvedFields: ReadonlyArray<FormFieldKey> =
    schema?.formFields ??
    fields ??
    schemaForCategoryId(values.category_id, categories).formFields;
  // State for category change confirmation dialog
  const [pendingCategoryId, setPendingCategoryId] = useState<number | null>(null);
  const [isMoving, setIsMoving] = useState(false);

  // Helper to get metadata value by key
  const getMetadataValue = (key: string): string => {
    const meta = values.metadata.find((m) => m.key === key);
    return meta?.value ?? '';
  };

  // Handle display name change
  const handleDisplayNameChange = (display_name: string) => {
    onChange({ ...values, display_name });
  };

  // Handle category change
  const handleCategoryChange = (category_id: number | null) => {
    // If editing existing file that's in storage, prompt for move
    if (fileId && inStorage && onCategoryChange && values.category_id !== category_id) {
      setPendingCategoryId(category_id);
    } else {
      onChange({ ...values, category_id });
    }
  };

  // Handle confirmation of file move
  const handleConfirmMove = async () => {
    if (!onCategoryChange || pendingCategoryId === null) return;

    setIsMoving(true);
    try {
      await onCategoryChange(pendingCategoryId);
      onChange({ ...values, category_id: pendingCategoryId });
      setPendingCategoryId(null); // Close dialog on success
    } catch (error) {
      console.error('Failed to move file:', error);
      // Don't close dialog or clear pendingCategoryId so user can retry
    } finally {
      setIsMoving(false);
    }
  };

  // Handle cancel of file move
  const handleCancelMove = () => {
    setPendingCategoryId(null);
  };

  // Handle tag change
  const handleTagChange = (tag_ids: number[]) => {
    onChange({ ...values, tag_ids });
  };

  // Handle author change
  const handleAuthorChange = (author_ids: number[]) => {
    onChange({ ...values, author_ids });
  };

  // Handle progress change
  const handleProgressChange = (progress: string) => {
    onChange({ ...values, progress });
  };

  // Handle metadata field change
  const handleMetadataFieldChange = (
    key: string,
    value: string | number | boolean,
    dataType: MetadataType
  ) => {
    const newMetadata = values.metadata.filter((m) => m.key !== key);
    if (value !== '' && value !== null && value !== undefined) {
      newMetadata.push({
        key,
        value: String(value),
        data_type: dataType,
      });
    }
    onChange({ ...values, metadata: newMetadata });
  };

  const renderField = (key: FormFieldKey) => {
    switch (key) {
      case 'display_name':
        return (
          <div className="space-y-2" key={key}>
            <Label className="text-sm font-medium">Display Name</Label>
            <Input
              value={values.display_name}
              onChange={(e) => handleDisplayNameChange(e.target.value)}
              placeholder="File name"
            />
          </div>
        );

      case 'category':
        return (
          <div className="space-y-2" key={key}>
            <Label className="text-sm font-medium">Category</Label>
            <CategorySelect
              categories={categories}
              value={values.category_id}
              onValueChange={handleCategoryChange}
            />
          </div>
        );

      case 'authors':
        return (
          <div className="space-y-2" key={key}>
            <Label className="text-sm font-medium">Authors</Label>
            <AuthorManager
              authors={authors}
              selectedAuthorIds={values.author_ids}
              onAuthorAssign={handleAuthorChange}
              onAuthorCreate={onAuthorCreate}
            />
          </div>
        );

      case 'tags':
        return (
          <div className="space-y-2" key={key}>
            <Label className="text-sm font-medium">Tags</Label>
            <TagManager
              tags={tags}
              selectedTagIds={values.tag_ids}
              onTagAssign={handleTagChange}
              onTagCreate={onTagCreate}
            />
          </div>
        );

      case 'progress':
        return (
          <div className="space-y-2" key={key}>
            <Label htmlFor="progress" className="text-sm font-medium">
              Progress
            </Label>
            <Input
              id="progress"
              value={values.progress ?? ''}
              onChange={(e) => handleProgressChange(e.target.value)}
              placeholder="e.g. 50%, Chapter 5, Reading..."
              className="text-sm"
            />
          </div>
        );

      case 'volume':
        return (
          <div className="space-y-2" key={key}>
            <Label className="text-sm font-medium">Volume</Label>
            <Input
              type="number"
              value={getMetadataValue('volume')}
              onChange={(e) =>
                handleMetadataFieldChange('volume', e.target.value, 'number')
              }
            />
          </div>
        );

      case 'cover': {
        // Four-state cover intent:
        //   1. user uploaded a replacement → render the new bytes inline
        //   2. user clicked Remove → render "None" placeholder
        //   3. import row with a pipeline-staged cover → fetch via
        //      StagedCoverPreview (bytes stay in Rust + Blob URL only)
        //   4. edit dialog → fetch the saved DB cover via ExistingCoverPreview
        const userBlobUrl = values.cover_data
          ? `data:${values.cover_mime_type ?? 'image/jpeg'};base64,${values.cover_data}`
          : null;
        const stagedPath =
          !values.cover_data && !values.cover_removed
            ? values.staged_cover_path
            : undefined;
        const showExisting =
          !values.cover_data &&
          !values.cover_removed &&
          !stagedPath &&
          fileId !== undefined;

        const handleCoverPick = (e: React.ChangeEvent<HTMLInputElement>) => {
          const file = e.target.files?.[0];
          if (!file) return;
          const reader = new FileReader();
          reader.onload = () => {
            const result = reader.result;
            if (typeof result !== 'string') return;
            // result = "data:<mime>;base64,<data>". Split off the prefix.
            const comma = result.indexOf(',');
            if (comma < 0) return;
            const data = result.slice(comma + 1);
            onChange({
              ...values,
              cover_data: data,
              cover_mime_type: file.type || 'image/jpeg',
              cover_removed: false,
            });
          };
          reader.readAsDataURL(file);
        };
        const handleCoverClear = () => {
          const next = { ...values, cover_removed: true };
          delete next.cover_data;
          delete next.cover_mime_type;
          // Drop the staged token too so the commit doesn't fall back to
          // the pipeline-staged cover after the user explicitly removed it.
          delete next.staged_cover_path;
          onChange(next);
        };

        const hasPreview = !!userBlobUrl || !!stagedPath || showExisting;

        return (
          <div className="space-y-2" key={key}>
            <Label className="text-sm font-medium">Cover</Label>
            <div className="flex items-center gap-3">
              {userBlobUrl ? (
                <img
                  src={userBlobUrl}
                  alt="Cover preview"
                  className="h-24 w-16 object-cover rounded-md border"
                />
              ) : stagedPath ? (
                <StagedCoverPreview stagedPath={stagedPath} />
              ) : showExisting ? (
                <ExistingCoverPreview fileId={fileId!} />
              ) : (
                <div className="h-24 w-16 rounded-md border border-dashed flex items-center justify-center text-xs text-muted-foreground">
                  None
                </div>
              )}
              <div className="flex flex-col gap-2">
                <label className="inline-flex">
                  <input
                    type="file"
                    accept="image/*"
                    className="hidden"
                    onChange={handleCoverPick}
                  />
                  <span className="cursor-pointer inline-flex items-center px-3 py-1.5 text-xs rounded-lg bg-secondary hover:bg-secondary/80 transition-colors">
                    {hasPreview ? 'Replace' : 'Upload'}
                  </span>
                </label>
                {hasPreview && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="text-xs h-auto py-1"
                    onClick={handleCoverClear}
                  >
                    Remove
                  </Button>
                )}
              </div>
            </div>
          </div>
        );
      }

      default:
        return null;
    }
  };

  return (
    <div className="space-y-4">
      {resolvedFields.map((field) => renderField(field))}

      {/* Category Change Confirmation Dialog */}
      <AlertDialog
        open={pendingCategoryId !== null}
        onOpenChange={(open) => {
          if (!open) setPendingCategoryId(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Move file to new category folder?</AlertDialogTitle>
            <AlertDialogDescription>
              The file will be moved to the new category's folder. This cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel onClick={handleCancelMove}>Cancel</AlertDialogCancel>
            <AlertDialogAction asChild>
              <Button onClick={handleConfirmMove} disabled={isMoving}>
                {isMoving ? 'Moving...' : 'Move File'}
              </Button>
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
