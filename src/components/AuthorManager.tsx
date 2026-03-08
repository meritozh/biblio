import { useState, useMemo } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { AuthorBadge } from '@/components/AuthorBadge';
import { Check, Plus, X } from 'lucide-react';
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
  const [isSaving, setIsSaving] = useState(false);

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
    if (!newAuthorName.trim() || !onAuthorCreate) return;

    setIsSaving(true);
    try {
      const newAuthor = await onAuthorCreate(newAuthorName.trim());
      onAuthorAssign([...selectedAuthorIds, newAuthor.id]);
      setNewAuthorName('');
      setIsCreating(false);
      setSearchQuery('');
    } catch (error) {
      console.error('Failed to create author:', error);
    } finally {
      setIsSaving(false);
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
      {selectedAuthors.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {selectedAuthors.map((author) => (
            <AuthorBadge key={author.id} author={author} onRemove={handleRemoveAuthor} />
          ))}
        </div>
      )}

      {isCreating ? (
        <div className="flex gap-2">
          <Input
            value={newAuthorName}
            onChange={(e) => setNewAuthorName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleCreateAuthor();
              if (e.key === 'Escape') handleCancelCreate();
            }}
            placeholder="New author name..."
            disabled={isSaving}
            className="h-9 flex-1"
            autoFocus
          />
          <Button size="sm" onClick={handleCreateAuthor} disabled={isSaving || !newAuthorName.trim()}>
            Create
          </Button>
          <Button size="sm" variant="ghost" onClick={handleCancelCreate} disabled={isSaving}>
            <X className="h-4 w-4" />
          </Button>
        </div>
      ) : (
        <Popover open={open} onOpenChange={setOpen}>
          <PopoverTrigger asChild>
            <Button variant="outline" size="sm" className="h-9 text-muted-foreground">
              <Plus className="h-4 w-4 mr-1" />
              {selectedAuthors.length > 0 ? 'Edit authors' : 'Add authors'}
            </Button>
          </PopoverTrigger>
          <PopoverContent className="w-72 p-0" align="start">
            <div className="p-3 border-b">
              <Input
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search or create author..."
                className="h-9"
              />
            </div>
            <div className="max-h-56 overflow-auto p-1">
              {filteredAuthors.length === 0 && !showCreateOption ? (
                <div className="px-3 py-6 text-center text-sm text-muted-foreground">
                  {searchQuery ? 'No matching authors' : 'No authors yet'}
                </div>
              ) : (
                <>
                  {filteredAuthors.map((author) => {
                    const isSelected = selectedAuthorIds.includes(author.id);
                    return (
                      <button
                        key={author.id}
                        type="button"
                        className="w-full flex items-center gap-3 px-3 py-2 text-sm rounded-md hover:bg-accent transition-colors"
                        onClick={() => handleToggleAuthor(author.id)}
                      >
                        <div className={`w-4 h-4 border rounded flex items-center justify-center transition-colors ${isSelected ? 'bg-primary border-primary text-primary-foreground' : 'border-muted-foreground/30'}`}>
                          {isSelected && <Check className="h-3 w-3" />}
                        </div>
                        <span className="flex-1 truncate text-left">{author.name}</span>
                      </button>
                    );
                  })}
                  {showCreateOption && onAuthorCreate && (
                    <button
                      type="button"
                      className="w-full flex items-center gap-3 px-3 py-2 text-sm rounded-md hover:bg-accent transition-colors text-primary"
                      onClick={handleStartCreate}
                    >
                      <Plus className="h-4 w-4" />
                      <span>Create "{searchQuery}"</span>
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