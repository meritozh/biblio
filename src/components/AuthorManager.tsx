import { useState, useMemo, useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { AuthorBadge } from '@/components/AuthorBadge';
import { PaginatedPicker, type PickerPage } from '@/components/PaginatedPicker';
import { Plus } from 'lucide-react';
import { authorList } from '@/lib/tauri';
import type { Author } from '@/types';

interface AuthorManagerProps {
  /** Display-only lookup for the selected-chips row. See `TagManager`
   *  for the same pattern. */
  authors: Author[];
  selectedAuthorIds: number[];
  onAuthorAssign: (authorIds: number[]) => void;
  onAuthorCreate?: (name: string) => Promise<Author>;
}

export function AuthorManager({
  authors,
  selectedAuthorIds,
  onAuthorAssign,
  onAuthorCreate,
}: AuthorManagerProps) {
  const [open, setOpen] = useState(false);

  const selectedAuthors = useMemo(
    () => authors.filter((a) => selectedAuthorIds.includes(a.id)),
    [authors, selectedAuthorIds]
  );

  const handleRemoveAuthor = (author: Author) => {
    onAuthorAssign(selectedAuthorIds.filter((id) => id !== author.id));
  };

  const fetcher = useCallback(
    async ({
      query,
      offset,
      limit,
    }: {
      query: string;
      offset: number;
      limit: number;
    }): Promise<PickerPage> => {
      const { authors: page } = await authorList({
        limit,
        offset,
        nameQuery: query.length > 0 ? query : undefined,
      });
      const total = page.length < limit ? offset + page.length : offset + page.length + 1;
      return {
        items: page.map((a) => ({ id: a.id, name: a.name })),
        total,
      };
    },
    []
  );

  const handleCreate = useCallback(
    async (name: string) => {
      if (!onAuthorCreate) {
        throw new Error('onAuthorCreate not provided');
      }
      const created = await onAuthorCreate(name);
      return { id: created.id, name: created.name };
    },
    [onAuthorCreate]
  );

  return (
    <div className="space-y-2">
      {selectedAuthors.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {selectedAuthors.map((author) => (
            <AuthorBadge key={author.id} author={author} onRemove={handleRemoveAuthor} />
          ))}
        </div>
      )}

      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button variant="outline" size="sm" className="h-9 text-muted-foreground">
            <Plus className="h-4 w-4 mr-1" />
            {selectedAuthors.length > 0 ? 'Edit authors' : 'Add authors'}
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-72 p-0" align="start">
          <PaginatedPicker
            mode="multi"
            selectedIds={selectedAuthorIds}
            fetcher={fetcher}
            onToggle={onAuthorAssign}
            onCreate={onAuthorCreate ? handleCreate : undefined}
            searchPlaceholder="Search or create author…"
            emptyLabel="No authors yet"
            noMatchLabel="No matching authors"
          />
        </PopoverContent>
      </Popover>
    </div>
  );
}
