import { FileList } from '@/components/FileList';
import type { FileEntry } from '@/types';

interface SearchResultsProps {
  files: FileEntry[];
  query: string;
  total: number;
  onFileClick?: (file: FileEntry) => void;
}

export function SearchResults({ files, query, total, onFileClick }: SearchResultsProps) {
  if (!query) {
    return null;
  }

  return (
    <div className="space-y-4">
      <div className="text-sm text-muted-foreground">
        {total} result{total !== 1 ? 's' : ''} for "{query}"
      </div>
      <FileList files={files} onFileClick={onFileClick} />
    </div>
  );
}
