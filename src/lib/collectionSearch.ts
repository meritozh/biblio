import type { Collection, FileEntry } from '@/types';

type SearchableFile = Pick<FileEntry, 'display_name' | 'path' | 'original_path'>;

function matchesQuery(value: string | null | undefined, query: string): boolean {
  return value != null && value.toLowerCase().includes(query);
}

export function filterCollectionsForQuery(
  collections: Collection[] | null,
  rawQuery: string,
  byId: ReadonlyMap<number, SearchableFile>
): Collection[] | null {
  const query = rawQuery.trim().toLowerCase();
  if (!collections || query.length === 0) return collections;

  return collections.filter((collection) => {
    if (matchesQuery(collection.title, query)) return true;
    return collection.file_ids.some((id) => {
      const file = byId.get(id);
      return (
        matchesQuery(file?.display_name, query) ||
        matchesQuery(file?.path, query) ||
        matchesQuery(file?.original_path, query)
      );
    });
  });
}
