import { describe, expect, it } from 'vitest';
import {
  matchFieldKeyword,
  suggestFieldValues,
  suggestFields,
  suggestSearchList,
  textAfterTokenPick,
} from '@/lib/searchTokens';
import type { Author, Tag } from '@/types';

const tags: Tag[] = [
  { id: 1, name: 'sci-fi', color: null, created_at: '2026-01-01T00:00:00Z' },
  { id: 2, name: 'romance', color: null, created_at: '2026-01-01T00:00:00Z' },
];

const authors: Author[] = [
  { id: 10, name: '村上春树', created_at: '2026-01-01T00:00:00Z' },
  { id: 11, name: '卡尔维诺', created_at: '2026-01-01T00:00:00Z' },
];

describe('searchTokens', () => {
  it('suggests fields from aliases', () => {
    expect(suggestFields('tag').map((f) => f.field)).toContain('tags');
    expect(suggestFields('cache').map((f) => f.field)).toContain('local_cache');
    expect(suggestFields('fav').map((f) => f.field)).toContain('favorite');
  });

  it('matches exact field keywords', () => {
    expect(matchFieldKeyword('author')?.field).toBe('authors');
    expect(matchFieldKeyword('TAG')?.field).toBe('tags');
    expect(matchFieldKeyword('hello')).toBeNull();
  });

  it('lists a concrete tag value ahead of the field drill-in', () => {
    const hits = suggestSearchList('sci', { tags, authors });
    expect(hits[0]).toMatchObject({ kind: 'value', field: 'tags', label: 'sci-fi' });
  });

  it('consumes a keyword or a value prefix and keeps other phrases', () => {
    expect(textAfterTokenPick('tag', 'sci-fi', 'tags')).toBe('');
    expect(textAfterTokenPick('sci', 'sci-fi', 'tags')).toBe('');
    expect(textAfterTokenPick('star wars', 'Favorited', 'favorite')).toBe('star wars');
  });

  it('suggests author and tag values', () => {
    const authorHits = suggestFieldValues('authors', '村上', { authors, tags });
    expect(authorHits).toHaveLength(1);
    expect(authorHits[0]?.condition).toMatchObject({
      field: 'authors',
      op: 'includes',
      authorId: 10,
    });

    const tagHits = suggestFieldValues('tags', 'sci', { authors, tags });
    expect(tagHits[0]?.condition).toMatchObject({
      field: 'tags',
      op: 'includes',
      tagId: 1,
    });
  });

  it('suggests cache and favorite enums', () => {
    const cache = suggestFieldValues('local_cache', '');
    expect(cache.map((s) => s.label)).toEqual(['Cached', 'Not cached']);

    const fav = suggestFieldValues('favorite', 'fav');
    expect(fav[0]?.condition).toMatchObject({
      field: 'favorite',
      op: 'is',
      value: true,
    });
  });
});
