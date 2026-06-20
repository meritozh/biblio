import { coverGet, fileGet } from '@/lib/tauri';
import type { FileCreateRequest } from '@/types';
import type { Bucket, FileItemState } from './types';

/** For each empty field in the new-file's import params, fall back to the
 *  existing row's value. Called only when the user picks `Replace` on a
 *  duplicate — that action's natural reading is "keep what was there +
 *  override what I changed", not "wipe and rebuild from my new file
 *  alone." The most common case this addresses: LLM extraction missed a
 *  field on the new file, so it's blank; without the merge the
 *  user-curated tags/authors/cover on the existing row would be lost.
 *
 *  Trade-off: the merge can't tell "extraction missed" apart from
 *  "user explicitly cleared in the form" — both look empty. Users who
 *  deliberately clear and then pick Replace will see the existing value
 *  re-populate; they have to clear again on the resulting row. Rare
 *  enough vs. the common case to be the right default.
 *
 *  Cover handling: file_replace cascades-deletes the existing row's
 *  cover. If the new params have neither inline `cover_data` nor a
 *  pipeline-staged path, we fetch the existing cover via `coverGet` and
 *  stash it as `cover_data` so the new row inherits the cover bytes. The
 *  one exception is when the user explicitly removed the cover in the form
 *  (`cover_removed`): that's a deliberate clear, not a missed extraction,
 *  so we skip the inherit and let the new row land without a cover. */
export async function mergeReplaceParams<T extends FileCreateRequest>(
  newParams: T,
  existingId: number
): Promise<T> {
  const existing = await fileGet(existingId);
  const merged: T = { ...newParams };

  if (!newParams.display_name?.trim()) {
    merged.display_name = existing.display_name;
  }
  if (newParams.category_id == null) {
    merged.category_id = existing.category_id;
  }
  if (!newParams.tag_ids || newParams.tag_ids.length === 0) {
    merged.tag_ids = existing.tags.map((t) => t.id);
  }
  if (!newParams.author_ids || newParams.author_ids.length === 0) {
    merged.author_ids = existing.authors.map((a) => a.id);
  }
  if (!newParams.metadata || newParams.metadata.length === 0) {
    // Strip id + file_id off the existing metadata rows — FileCreateRequest
    // takes the writable subset only.
    merged.metadata = existing.metadata.map((m) => ({
      key: m.key,
      value: m.value,
      data_type: m.data_type,
    }));
  }
  if (!newParams.progress || !newParams.progress.trim()) {
    merged.progress = existing.progress ?? undefined;
  }
  if (!newParams.cover_removed && !newParams.cover_data && !newParams.staged_cover_path) {
    try {
      const c = await coverGet(existingId);
      merged.cover_data = c.data;
      merged.cover_mime_type = c.mime_type;
    } catch {
      // No existing cover row — leave merged.cover_data undefined; the
      // new row simply won't have a cover, same as today.
    }
  }
  return merged;
}

export function bucketOf(item: FileItemState): Bucket {
  if (item.status === 'error') return 'failed';
  if (
    item.status === 'pending' ||
    item.status === 'extracting_name' ||
    item.status === 'analyzing_content'
  ) {
    return 'processing';
  }
  // status is 'ready' or 'partial' — decide between review and ready
  if (item.status === 'partial' || item.preparedImport?.duplicate_of) {
    return 'review';
  }
  return 'ready';
}

export function needsDuplicateDecision(item: FileItemState): boolean {
  return !!item.preparedImport?.duplicate_of && item.duplicateAction == null;
}

export function normalizeCatalogName(name: string): string {
  return name.normalize('NFC').trim().toLowerCase();
}
