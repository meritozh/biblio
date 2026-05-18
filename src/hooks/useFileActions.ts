import { useCallback, useEffect, useState } from 'react';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { fetchCategories } from '@/stores';
import { patchFile, refreshActiveView, removeFile } from '@/stores/fileStore';
import {
  authorCreate,
  authorList,
  authorSet,
  coverDelete,
  coverSet,
  fileDelete,
  fileGet,
  fileUpdate,
  listenTagAuthorChanges,
  metadataDelete,
  metadataSet,
  tagAssign,
  tagCreate,
  tagList,
  tagUnassign,
} from '@/lib/tauri';
import type { DynamicMetadataFormValues } from '@/components/DynamicMetadataForm';
import type { Author, Category, FileEntry, Tag } from '@/types';

/**
 * All the plumbing the file list + edit dialog + delete dialog need, in one
 * place. Extracted from Library (`src/routes/index.tsx`) and FileDetailPage so
 * the two don't drift.
 *
 * Mutations flow through the normalized `fileStore`:
 *   - save → `fileGet` + `patchFile` so only the affected card re-renders
 *   - delete → `removeFile`
 *   - tag/author rename event → `refreshActiveView` (chip text is
 *     denormalized onto each row, so a rename invalidates cached rows)
 *
 * Exposed state is fully controlled: open/close the dialogs via the setters,
 * wire up the form callbacks, pass `categories`/`tags`/`authors` into the
 * edit dialog and import pipeline. Optimistic mutators (`handle*Create`)
 * return the persisted record so dialogs can immediately use the new id.
 */
export function useFileActions() {
  const [categories, setCategories] = useState<Category[]>([]);
  const [tags, setTags] = useState<Tag[]>([]);
  const [authors, setAuthors] = useState<Author[]>([]);

  const [editDialogOpen, setEditDialogOpen] = useState(false);
  const [editingFile, setEditingFile] = useState<FileEntry | null>(null);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deletingFile, setDeletingFile] = useState<FileEntry | null>(null);

  // Initial fetches — categories/tags/authors drive the edit dialog's
  // selectors, so they need to be populated before any row is clicked.
  useEffect(() => {
    void fetchCategories().then(setCategories);
    void tagList({ includeUsage: true }).then((r) => setTags(r.tags));
    void authorList({ includeUsage: true }).then((r) => setAuthors(r.authors));
  }, []);

  // Keep relations fresh when the user edits them on /tags or /authors.
  // Also bump the active view so chip text picks up the rename — chips are
  // denormalized onto each row, so a rename invalidates cached row content.
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    let cancelled = false;
    listenTagAuthorChanges(() => {
      void tagList({ includeUsage: true }).then((r) => setTags(r.tags));
      void authorList({ includeUsage: true }).then((r) => setAuthors(r.authors));
      refreshActiveView();
    })
      .then((u) => {
        if (cancelled) u();
        else unlisten = u;
      })
      .catch((err) => {
        console.error('Failed to subscribe to tag/author changes:', err);
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const handleTagCreate = useCallback(async (name: string): Promise<Tag> => {
    const result = await tagCreate(name);
    const newTag: Tag = {
      id: result.id,
      name,
      color: null,
      created_at: new Date().toISOString(),
    };
    setTags((prev) => [...prev, newTag]);
    return newTag;
  }, []);

  const handleAuthorCreate = useCallback(async (name: string): Promise<Author> => {
    const result = await authorCreate(name);
    const newAuthor: Author = {
      id: result.id,
      name,
      created_at: new Date().toISOString(),
    };
    setAuthors((prev) => [...prev, newAuthor]);
    return newAuthor;
  }, []);

  const handleFileEdit = useCallback((file: FileEntry) => {
    setEditingFile(file);
    setEditDialogOpen(true);
  }, []);

  const handleFileSave = useCallback(
    async (fileId: number, values: DynamicMetadataFormValues) => {
      await fileUpdate(fileId, {
        display_name: values.display_name,
        category_id: values.category_id,
        progress: values.progress ?? null,
      });

      // Tag diff against what the file had when the dialog opened — avoids
      // clobbering tags another surface added concurrently. Add-only for new
      // ones, explicit unassign for removed ones.
      const currentTagIds = editingFile?.tags?.map((t) => t.id) ?? [];
      const removedTagIds = currentTagIds.filter((id) => !values.tag_ids.includes(id));
      if (removedTagIds.length > 0) {
        await tagUnassign(fileId, removedTagIds);
      }
      await tagAssign(fileId, values.tag_ids);

      // Authors: explicit replacement is semantically simpler than diffing.
      await authorSet(fileId, values.author_ids);

      // Metadata: wipe prior then rewrite. Cheap because it's per-file.
      if (editingFile?.metadata) {
        for (const m of editingFile.metadata) {
          await metadataDelete(fileId, m.key);
        }
      }
      for (const m of values.metadata) {
        await metadataSet(fileId, m.key, m.value, m.data_type);
      }

      // Cover: tri-state intent
      //   • cover_data set → user uploaded a replacement, write it
      //   • cover_removed === true → user clicked Remove, delete DB row
      //   • neither → user didn't touch the cover, leave the DB row alone
      // The unconditional delete that lived here previously would silently
      // wipe the cover on every Save where the user hadn't touched it.
      if (values.cover_data) {
        const binary = atob(values.cover_data);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
        await coverSet(fileId, Array.from(bytes), values.cover_mime_type);
      } else if (values.cover_removed) {
        await coverDelete(fileId);
      }

      // Refresh just this row in the store. Only this card re-renders;
      // others keep their cover-image and DOM state intact.
      const updated = await fileGet(fileId);
      patchFile(fileId, updated);
    },
    [editingFile]
  );

  const handleFileDeleteClick = useCallback((file: FileEntry) => {
    setDeletingFile(file);
    setDeleteDialogOpen(true);
  }, []);

  const handleFileDeleteConfirm = useCallback(async () => {
    if (!deletingFile) return;
    try {
      await fileDelete(deletingFile.id);
      setDeleteDialogOpen(false);
      removeFile(deletingFile.id);
      setDeletingFile(null);
    } catch (error) {
      console.error('Failed to delete:', error);
      alert(`Failed to delete: ${error}`);
    }
  }, [deletingFile]);

  return {
    // Relation state + mutators
    categories,
    tags,
    authors,
    handleTagCreate,
    handleAuthorCreate,

    // Edit dialog
    editingFile,
    editDialogOpen,
    setEditDialogOpen,
    handleFileEdit,
    handleFileSave,

    // Delete dialog
    deletingFile,
    deleteDialogOpen,
    setDeleteDialogOpen,
    handleFileDeleteClick,
    handleFileDeleteConfirm,
  };
}
