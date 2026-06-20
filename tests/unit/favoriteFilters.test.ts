import { describe, expect, it } from 'vitest';
import {
  applyConditions,
  describeCondition,
  newCondition,
  type Condition,
  type Field,
} from '@/lib/filters';
import type { FileEntry, Tag } from '@/types';

function file(id: number, isFavorite: boolean): FileEntry {
  return {
    id,
    path: `/library/${id}.txt`,
    display_name: `File ${id}`,
    category_id: 1,
    file_status: 'available',
    in_storage: true,
    original_path: null,
    is_favorite: isFavorite,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  };
}

describe('favorite filter conditions', () => {
  it('creates a default condition for favorited files', () => {
    expect(newCondition('favorite' as Field)).toMatchObject({
      field: 'favorite',
      op: 'is',
      value: true,
    });
  });

  it('matches favorite and non-favorite rows client-side', () => {
    const files = [file(1, true), file(2, false)];

    expect(
      applyConditions(files, [
        { id: 'favorite', field: 'favorite', op: 'is', value: true } as Condition,
      ]).map((f) => f.id)
    ).toEqual([1]);

    expect(
      applyConditions(files, [
        { id: 'not-favorite', field: 'favorite', op: 'is', value: false } as Condition,
      ]).map((f) => f.id)
    ).toEqual([2]);
  });

  it('describes favorite conditions for filter chips', () => {
    expect(
      describeCondition(
        { id: 'favorite', field: 'favorite', op: 'is', value: true } as Condition,
        new Map<number, Tag>()
      )
    ).toBe('Favorites only');
    expect(
      describeCondition(
        { id: 'not-favorite', field: 'favorite', op: 'is', value: false } as Condition,
        new Map<number, Tag>()
      )
    ).toBe('Not favorited');
  });
});
