import { useState, useMemo } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { AuthorBadge } from '@/components/AuthorBadge';
import { Plus, X } from 'lucide-react';
import type { Author } from '@/types';

interface AuthorManagerProps {
  authors: Author[];
  selectedAuthorIds: number[];
  onAuthorAssign: (authorIds: number[]) => void;
  onAuthorCreate?: (name: string) => Promise<Author>;
}

export function AuthorManager({ authors, selectedAuthorIds, onAuthorAssign, onAuthorCreate }: AuthorManagerProps) {
  const [isCreating, setIsCreating] = useState(false);
  const [newAuthorName, setNewAuthorName] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  const [showDropdown, setShowDropdown] = useState(false);

  const selectedAuthors = useMemo(
    () => authors.filter((a) => selectedAuthorIds.includes(a.id)),
    [authors, selectedAuthorIds]
  );

  const filteredAuthors = useMemo(
    () => authors.filter((a) =>
      !selectedAuthorIds.includes(a.id) &&
      a.name.toLowerCase().includes(searchQuery.toLowerCase())
    ),
    [authors, selectedAuthorIds, searchQuery]
  );

  const handleAddAuthor = (author: Author) => {
    onAuthorAssign([...selectedAuthorIds, author.id]);
    setSearchQuery('');
    setShowDropdown(false);
  };

  const handleCreateAuthor = async () => {
    if (newAuthorName.trim() && onAuthorCreate) {
      const newAuthor = await onAuthorCreate(newAuthorName.trim());
      onAuthorAssign([...selectedAuthorIds, newAuthor.id]);
      setNewAuthorName('');
      setIsCreating(false);
    }
  };

  const handleRemoveAuthor = (author: Author) => {
    onAuthorAssign(selectedAuthorIds.filter((id) => id !== author.id));
  };

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap gap-2">
        {selectedAuthors.map((author) => (
          <AuthorBadge key={author.id} author={author} onRemove={handleRemoveAuthor} />
        ))}
      </div>

      {isCreating ? (
        <div className="flex gap-1">
          <Input
            value={newAuthorName}
            onChange={(e) => setNewAuthorName(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleCreateAuthor()}
            placeholder="Author name"
            className="h-7 w-32 text-xs"
            autoFocus
          />
          <Button size="sm" variant="ghost" onClick={handleCreateAuthor} className="h-7 px-2">
            <Plus className="h-3 w-3" />
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              setIsCreating(false);
              setNewAuthorName('');
            }}
            className="h-7 px-2"
          >
            <X className="h-3 w-3" />
          </Button>
        </div>
      ) : (
        <div className="relative">
          <Input
            value={searchQuery}
            onChange={(e) => {
              setSearchQuery(e.target.value);
              setShowDropdown(true);
            }}
            onFocus={() => setShowDropdown(true)}
            placeholder="Search or add author..."
            className="h-7 text-xs"
          />
          {showDropdown && (searchQuery || filteredAuthors.length > 0) && (
            <div className="absolute z-10 top-full left-0 right-0 mt-1 bg-popover border rounded-md shadow-md max-h-40 overflow-auto">
              {filteredAuthors.map((author) => (
                <button
                  key={author.id}
                  type="button"
                  className="w-full px-2 py-1 text-left text-sm hover:bg-accent"
                  onClick={() => handleAddAuthor(author)}
                >
                  {author.name}
                </button>
              ))}
              {searchQuery && !filteredAuthors.some(a => a.name.toLowerCase() === searchQuery.toLowerCase()) && onAuthorCreate && (
                <button
                  type="button"
                  className="w-full px-2 py-1 text-left text-sm hover:bg-accent text-muted-foreground"
                  onClick={() => {
                    setIsCreating(true);
                    setNewAuthorName(searchQuery);
                    setShowDropdown(false);
                  }}
                >
                  + Create "{searchQuery}"
                </button>
              )}
              {!searchQuery && filteredAuthors.length === 0 && (
                <div className="px-2 py-1 text-sm text-muted-foreground">No authors available</div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}