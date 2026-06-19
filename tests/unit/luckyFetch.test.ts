import { describe, expect, it, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { fetchLuckyFiles } from '@/stores';
import type { Condition } from '@/lib/filters';
import type { FileEntry } from '@/types';

vi.mock('@tauri-apps/api/core');

describe('fetchLuckyFiles', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('invokes file_lucky with the current category, trimmed query, serialized filters, and limit', async () => {
    const files: FileEntry[] = [
      {
        id: 1,
        path: '/library/a.zip',
        display_name: 'A',
        category_id: 2,
        file_status: 'available',
        in_storage: true,
        original_path: null,
        created_at: '2026-01-01T00:00:00Z',
        updated_at: '2026-01-01T00:00:00Z',
      },
    ];
    vi.mocked(invoke).mockResolvedValue(files);
    const conditions: Condition[] = [
      { id: 'c1', field: 'tags', op: 'includes', tagId: 7 },
    ];

    const result = await fetchLuckyFiles({
      category_id: 2,
      query: '  needle  ',
      conditions,
      limit: 3,
    });

    expect(invoke).toHaveBeenCalledWith('file_lucky', {
      categoryId: 2,
      query: 'needle',
      conditions: [{ field: 'tags', op: 'includes', tag_id: 7 }],
      limit: 3,
    });
    expect(result).toEqual(files);
  });
});
