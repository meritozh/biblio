import { useState, useMemo } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { AuthorBadge } from '@/components/AuthorBadge';
import { Check, Plus, Search } from 'lucide-react';
import type { Author } from '@/types';

interface AuthorManagerProps {
  authors: Author[];
  selectedAuthorIds: number[];
  onAuthorAssign: (authorIds: number[]) => void;
  onAuthorCreate?: (name: string) => Promise<Author>;
}

export function AuthorManager({ authors, selectedAuthorIds, onAuthorAssign, onAuthorCreate }: AuthorManagerProps) {
  const [open, setOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [isCreating, setIsCreating] = useState(false);
  const [newAuthorName, setNewAuthorName] = useState('');

  const selectedAuthors = useMemo(
    () => authors.filter((a) => selectedAuthorIds.includes(a.id)),
    [authors, selectedAuthorIds]
  );

  const filteredAuthors = useMemo(
    () => authors.filter((a) =>
      a.name.toLowerCase().includes(searchQuery.toLowerCase())
    ),
    [authors, searchQuery]
  );

  const handleToggleAuthor = (authorId: number) => {
    if (selectedAuthorIds.includes(authorId)) {
      onAuthorAssign(selectedAuthorIds.filter((id) => id !== authorId));
    } else {
      onAuthorAssign([...selectedAuthorIds, authorId]);
    }
  };

  const handleRemoveAuthor = (author: Author) => {
    onAuthorAssign(selectedAuthorIds.filter((id) => id !== author.id));
  };

  const handleCreateAuthor = async () => {
    if (newAuthorName.trim() && onAuthorCreate) {
      const newAuthor = await onAuthorCreate(newAuthorName.trim());
      onAuthorAssign([...selectedAuthorIds, newAuthor.id]);
      setNewAuthorName('');
      setIsCreating(false);
      setSearchQuery('');
    }
  };

  const handleStartCreate = () => {
    setIsCreating(true);
    setNewAuthorName(searchQuery);
    setSearchQuery('');
  };

  const handleCancelCreate = () => {
    setIsCreating(false);
    setNewAuthorName('');
  };

  const showCreateOption = searchQuery && !filteredAuthors.some(a => a.name.toLowerCase() === searchQuery.toLowerCase());

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
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleCreateAuthor();
              if (e.key === 'Escape') handleCancelCreate();
            }}
            placeholder="Author name"
            className="h-8 flex-1 text-sm"
            autoFocus
          />
          <Button size="sm" onClick={handleCreateAuthor} disabled={!newAuthorName.trim()}>
            Create
          </Button>
          <Button size="sm" variant="outline" onClick={handleCancelCreate}>
            Cancel
          </Button>
        </div>
      ) : (
        <Popover open={open} onOpenChange={setOpen}>
          <PopoverTrigger asChild>
            <Button variant="outline" size="sm" className="h-8">
              <Plus className="h-3 w-3 mr-1" />
              Add Author
            </Button>
          </PopoverTrigger>
          <PopoverContent className="w-64 p-0" align="start">
            <div className="p-2 border-b">
              <div className="relative">
                <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground" />
                <Input
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search authors..."
                  className="h-8 pl-7 text-sm"
                />
              </div>
            </div>
            <div className="max-h-48 overflow-auto p-1">
              {filteredAuthors.length === 0 && !showCreateOption ? (
                <div className="px-2 py-4 text-center text-sm text-muted-foreground">
                  {searchQuery ? 'No authors found' : 'No authors available'}
                </div>
              ) : (
                <>
                  {filteredAuthors.map((author) => {
                    const isSelected = selectedAuthorIds.includes(author.id);
                    return (
                      <button
                        key={author.id}
                        type="button"
                        className="w-full flex items-center gap-2 px-2 py-1.5 text-sm rounded hover:bg-accent text-left"
                        onClick={() => handleToggleAuthor(author.id)}
                      >
                        <div className="w-4 h-4 border rounded flex items-center justify-center">
                          {isSelected && <Check className="h-3 w-3" />}
                        </div>
                        <span className="flex-1 truncate">{author.name}</span>
                      </button>
                    );
                  })}
                  {showCreateOption && onAuthorCreate && (
                    <button
                      type="button"
                      className="w-full flex items-center gap-2 px-2 py-1.5 text-sm rounded hover:bg-accent text-left text-muted-foreground"
                      onClick={handleStartCreate}
                    >
                      <Plus className="h-4 w-4" />
                      Create "{searchQuery}"
                    </button>
                  )}
                </>
              )}
            </div>
          </PopoverContent>
        </Popover>
      )}
    </div>
  );
}