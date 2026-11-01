import { beforeEach, describe, expect, it } from 'vitest';
import { mockInvoke } from '../setup';
import { fetchFiles } from '@/stores';
import { fileReplace, fileSetFavorite } from '@/lib/tauri';
import type { Condition } from '@/lib/filters';

describe('favorite IPC contracts', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('serializes favorite filters for file_list', async () => {
    mockInvoke.mockResolvedValue({ files: [], total: 0 });
    const conditions: Condition[] = [
      { id: 'favorite', field: 'favorite', op: 'is', value: true } as Condition,
    ];

    await fetchFiles({ category_id: 1, conditions });

    expect(mockInvoke).toHaveBeenCalledWith('file_list', {
      categoryId: 1,
      query: undefined,
      sortBy: undefined,
      sortDesc: undefined,
      conditions: [{ field: 'favorite', op: 'is', value: true }],
      limit: undefined,
      offset: undefined,
    });
  });

  it('passes free-text query through the unified file_list command', async () => {
    mockInvoke.mockResolvedValue({ files: [], total: 0 });

    await fetchFiles({ category_id: 2, query: '  kafka  ' });

    expect(mockInvoke).toHaveBeenCalledWith('file_list', {
      categoryId: 2,
      query: 'kafka',
      sortBy: undefined,
      sortDesc: undefined,
      conditions: undefined,
      limit: undefined,
      offset: undefined,
    });
    expect(mockInvoke).not.toHaveBeenCalledWith(
      'file_search',
      expect.anything(),
    );
  });

  it('toggles favorite state through one narrow command', async () => {
    mockInvoke.mockResolvedValue({ success: true });

    await expect(fileSetFavorite(42, true)).resolves.toEqual({ success: true });

    expect(mockInvoke).toHaveBeenCalledWith('file_set_favorite', {
      id: 42,
      isFavorite: true,
    });
  });

  it('sends favorite state through replace imports', async () => {
    mockInvoke.mockResolvedValue({ id: 9 });

    await expect(
      fileReplace(7, {
        path: '/tmp/new.epub',
        display_name: 'New',
        category_id: 1,
        is_favorite: true,
      })
    ).resolves.toEqual({ id: 9 });

    expect(mockInvoke).toHaveBeenCalledWith('file_replace', {
      existingFileId: 7,
      path: '/tmp/new.epub',
      displayName: 'New',
      categoryId: 1,
      tagIds: undefined,
      authorIds: undefined,
      metadata: undefined,
      progress: undefined,
      isFavorite: true,
      coverData: null,
      coverMimeType: null,
      stagedCoverPath: null,
    });
  });
});
