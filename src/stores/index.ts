import { invoke } from '@tauri-apps/api/core';
import type { FileEntry, Category } from '@/types';
import type { Condition } from '@/lib/filters';

export type SortKey = 'name' | 'created' | 'updated';

// Wire-format the filter editor's conditions into the loose object the
// backend's `FilterCondition` struct expects. Local `id`s are dropped (the
// server doesn't track them), and value-bearing variants project their value
// onto the appropriate column. Conditions whose value is missing are sent as
// no-op rows; the server skips those, matching the editor's "half-built row"
// rule.
function serializeConditions(conditions: ReadonlyArray<Condition>): unknown[] {
  return conditions.map((c) => {
    const base: { field: string; op: string } = { field: c.field, op: c.op };
    if (c.field === 'tags' && (c.op === 'count_gte' || c.op === 'count_lt')) {
      return { ...base, n: c.n };
    }
    if (c.field === 'tags' && (c.op === 'includes' || c.op === 'excludes')) {
      return { ...base, tag_id: c.tagId };
    }
    if (c.field === 'tags' && (c.op === 'includes_any' || c.op === 'excludes_any')) {
      return { ...base, tag_ids: c.tagIds };
    }
    if (c.field === 'authors' && (c.op === 'count_gte' || c.op === 'count_lt')) {
      return { ...base, n: c.n };
    }
    if (c.field === 'authors' && c.op === 'includes') {
      return { ...base, author_id: c.authorId };
    }
    if (c.field === 'progress' && c.op === 'contains') {
      return { ...base, text: c.text };
    }
    if (c.field === 'file_status' && c.op === 'is') {
      return { ...base, value: c.value };
    }
    if (c.field === 'storage_kind' && c.op === 'is') {
      return { ...base, value: c.value };
    }
    return base;
  });
}

export async function fetchFiles(params?: {
  category_id?: number | null;
  /** Free-text search query. When non-empty, routes through the FTS-backed
   *  file_search command; otherwise the default paginated file_list. */
  query?: string;
  /** Server-side sort. Falls back to ('created', desc=true) on the backend. */
  sort_by?: SortKey;
  sort_desc?: boolean;
  /** Server-side filter pills, evaluated in SQL alongside category/search. */
  conditions?: ReadonlyArray<Condition>;
  limit?: number;
  offset?: number;
}): Promise<{ files: FileEntry[]; total: number }> {
  const query = params?.query?.trim();
  const conditions = params?.conditions && params.conditions.length > 0
    ? serializeConditions(params.conditions)
    : undefined;
  try {
    if (query) {
      return await invoke<{ files: FileEntry[]; total: number }>('file_search', {
        query,
        categoryId: params?.category_id,
        sortBy: params?.sort_by,
        sortDesc: params?.sort_desc,
        conditions,
        limit: params?.limit,
        offset: params?.offset,
      });
    }
    return await invoke<{ files: FileEntry[]; total: number }>('file_list', {
      categoryId: params?.category_id,
      sortBy: params?.sort_by,
      sortDesc: params?.sort_desc,
      conditions,
      limit: params?.limit,
      offset: params?.offset,
    });
  } catch (error) {
    console.error('Failed to fetch files:', error);
    return { files: [], total: 0 };
  }
}

export async function fetchCategories(): Promise<Category[]> {
  try {
    return await invoke('category_list');
  } catch (error) {
    console.error('Failed to fetch categories:', error);
    return [];
  }
}
