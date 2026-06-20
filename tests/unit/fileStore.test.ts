import { describe, expect, it, beforeEach } from 'vitest';
import { fileStore, patchFile, setView } from '@/stores/fileStore';
import type { Condition } from '@/lib/filters';
import type { FileEntry } from '@/types';

function makeFile(id: number, isFavorite: boolean): FileEntry {
  return {
    id,
    path: `/library/${id}.cbz`,
    display_name: `File ${id}`,
    category_id: 1,
    file_status: 'available',
    in_storage: true,
    original_path: null,
    progress: null,
    storage_kind: 'local',
    remote_provider: null,
    local_cache_path: null,
    is_favorite: isFavorite,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    category: null,
    tags: [],
    authors: [],
    metadata: [],
  };
}

describe('fileStore filtered view diffs', () => {
  beforeEach(() => {
    fileStore.setState(() => ({
      byId: new Map(),
      views: new Map(),
      refreshEpoch: 0,
    }));
  });

  it('removes a patched row from cached favorite-filtered views when it no longer matches', () => {
    const favoriteCondition: Condition = {
      id: 'favorite-filter',
      field: 'favorite',
      op: 'is',
      value: true,
    };
    const filteredKey = `home::filters=${JSON.stringify([favoriteCondition])}`;
    const unfilteredKey = 'home::filters=[]';

    setView(filteredKey, [makeFile(1, true), makeFile(2, true)], 2);
    setView(unfilteredKey, [makeFile(1, true), makeFile(2, true)], 2);

    patchFile(1, { is_favorite: false });

    expect(fileStore.state.views.get(filteredKey)).toMatchObject({
      ids: [2],
      total: 1,
    });
    expect(fileStore.state.views.get(unfilteredKey)).toMatchObject({
      ids: [1, 2],
      total: 2,
    });
    expect(fileStore.state.byId.get(1)?.is_favorite).toBe(false);
  });
});
