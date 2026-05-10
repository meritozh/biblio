import { FileList } from '@/components/FileList';
import type { FileEntry } from '@/types';

interface SearchResultsProps {
  ids: number[];
  query: string;
  total: number;
  onFileClick?: (file: FileEntry) => void;
}

export function SearchResults({ ids, query, total, onFileClick }: SearchResultsProps) {
  if (!query) {
    return null;
  }

  return (
    <div className="space-y-4">
      <div className="text-sm text-muted-foreground">
        {total} result{total !== 1 ? 's' : ''} for "{query}"
      </div>
      <FileList ids={ids} total={total} onFileClick={onFileClick} />
    </div>
  );
}
