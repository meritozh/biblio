import { useState, useMemo } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { TagBadge } from '@/components/TagBadge';
import { Check, Plus, Search } from 'lucide-react';
import type { Tag } from '@/types';

interface TagManagerProps {
  tags: Tag[];
  selectedTagIds: number[];
  onTagAssign: (tagIds: number[]) => void;
  onTagCreate?: (name: string) => Promise<Tag>;
}

export function TagManager({ tags, selectedTagIds, onTagAssign, onTagCreate }: TagManagerProps) {
  const [open, setOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [isCreating, setIsCreating] = useState(false);
  const [newTagName, setNewTagName] = useState('');

  const selectedTags = useMemo(
    () => tags.filter((t) => selectedTagIds.includes(t.id)),
    [tags, selectedTagIds]
  );

  const filteredTags = useMemo(
    () => tags.filter((t) =>
      t.name.toLowerCase().includes(searchQuery.toLowerCase())
    ),
    [tags, searchQuery]
  );

  const handleToggleTag = (tagId: number) => {
    if (selectedTagIds.includes(tagId)) {
      onTagAssign(selectedTagIds.filter((id) => id !== tagId));
    } else {
      onTagAssign([...selectedTagIds, tagId]);
    }
  };

  const handleRemoveTag = (tag: Tag) => {
    onTagAssign(selectedTagIds.filter((id) => id !== tag.id));
  };

  const handleCreateTag = async () => {
    if (newTagName.trim() && onTagCreate) {
      const newTag = await onTagCreate(newTagName.trim());
      onTagAssign([...selectedTagIds, newTag.id]);
      setNewTagName('');
      setIsCreating(false);
      setSearchQuery('');
    }
  };

  const handleStartCreate = () => {
    setIsCreating(true);
    setNewTagName(searchQuery);
    setSearchQuery('');
  };

  const handleCancelCreate = () => {
    setIsCreating(false);
    setNewTagName('');
  };

  const showCreateOption = searchQuery && !filteredTags.some(t => t.name.toLowerCase() === searchQuery.toLowerCase());

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap gap-2">
        {selectedTags.map((tag) => (
          <TagBadge key={tag.id} tag={tag} onRemove={handleRemoveTag} />
        ))}
      </div>

      {isCreating ? (
        <div className="flex gap-1">
          <Input
            value={newTagName}
            onChange={(e) => setNewTagName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleCreateTag();
              if (e.key === 'Escape') handleCancelCreate();
            }}
            placeholder="Tag name"
            className="h-8 flex-1 text-sm"
            autoFocus
          />
          <Button size="sm" onClick={handleCreateTag} disabled={!newTagName.trim()}>
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
              Add Tag
            </Button>
          </PopoverTrigger>
          <PopoverContent className="w-64 p-0" align="start">
            <div className="p-2 border-b">
              <div className="relative">
                <Search className="absolute left-2 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground" />
                <Input
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search tags..."
                  className="h-8 pl-7 text-sm"
                />
              </div>
            </div>
            <div className="max-h-48 overflow-auto p-1">
              {filteredTags.length === 0 && !showCreateOption ? (
                <div className="px-2 py-4 text-center text-sm text-muted-foreground">
                  {searchQuery ? 'No tags found' : 'No tags available'}
                </div>
              ) : (
                <>
                  {filteredTags.map((tag) => {
                    const isSelected = selectedTagIds.includes(tag.id);
                    return (
                      <button
                        key={tag.id}
                        type="button"
                        className="w-full flex items-center gap-2 px-2 py-1.5 text-sm rounded hover:bg-accent text-left"
                        onClick={() => handleToggleTag(tag.id)}
                      >
                        <div className="w-4 h-4 border rounded flex items-center justify-center">
                          {isSelected && <Check className="h-3 w-3" />}
                        </div>
                        {tag.color && (
                          <div
                            className="w-3 h-3 rounded-full"
                            style={{ backgroundColor: tag.color }}
                          />
                        )}
                        <span className="flex-1 truncate">{tag.name}</span>
                      </button>
                    );
                  })}
                  {showCreateOption && onTagCreate && (
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