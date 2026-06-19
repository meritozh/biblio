import { describe, expect, it } from 'vitest';
import { filterCollectionsForQuery } from '@/lib/collectionSearch';
import type { Collection, FileEntry } from '@/types';

const baseCollection: Collection = {
  mode: 'name_prefix',
  key: 'series',
  title: 'Visible Series',
  file_ids: [1, 2],
  cover_file_id: 1,
  schema_slug: 'comic',
};

function file(id: number, displayName: string, path = `/library/${displayName}.zip`): FileEntry {
  return {
    id,
    path,
    display_name: displayName,
    category_id: 1,
    file_status: 'available',
    in_storage: true,
    original_path: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  };
}

describe('filterCollectionsForQuery', () => {
  it('keeps a collection when the query matches a member file title', () => {
    const byId = new Map<number, FileEntry>([
      [1, file(1, 'Ordinary Volume 1')],
      [2, file(2, 'Hidden Needle Volume 2')],
    ]);

    expect(filterCollectionsForQuery([baseCollection], 'needle', byId)).toEqual([
      baseCollection,
    ]);
  });

  it('keeps a collection when the query matches the collection title', () => {
    expect(filterCollectionsForQuery([baseCollection], 'visible', new Map())).toEqual([
      baseCollection,
    ]);
  });

  it('drops collections whose title and hydrated members do not match', () => {
    const byId = new Map<number, FileEntry>([
      [1, file(1, 'Ordinary Volume 1')],
      [2, file(2, 'Ordinary Volume 2')],
    ]);

    expect(filterCollectionsForQuery([baseCollection], 'needle', byId)).toEqual([]);
  });
});
