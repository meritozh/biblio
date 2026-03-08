import { useState, useMemo } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { TagBadge } from '@/components/TagBadge';
import { Check, Plus, X } from 'lucide-react';
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
  const [isSaving, setIsSaving] = useState(false);

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
    if (!newTagName.trim() || !onTagCreate) return;

    setIsSaving(true);
    try {
      const newTag = await onTagCreate(newTagName.trim());
      onTagAssign([...selectedTagIds, newTag.id]);
      setNewTagName('');
      setIsCreating(false);
      setSearchQuery('');
    } catch (error) {
      console.error('Failed to create tag:', error);
    } finally {
      setIsSaving(false);
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
      {selectedTags.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {selectedTags.map((tag) => (
            <TagBadge key={tag.id} tag={tag} onRemove={handleRemoveTag} />
          ))}
        </div>
      )}

      {isCreating ? (
        <div className="flex gap-2">
          <Input
            value={newTagName}
            onChange={(e) => setNewTagName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleCreateTag();
              if (e.key === 'Escape') handleCancelCreate();
            }}
            placeholder="New tag name..."
            disabled={isSaving}
            className="h-9 flex-1"
            autoFocus
          />
          <Button size="sm" onClick={handleCreateTag} disabled={isSaving || !newTagName.trim()}>
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
              {selectedTags.length > 0 ? 'Edit tags' : 'Add tags'}
            </Button>
          </PopoverTrigger>
          <PopoverContent className="w-72 p-0" align="start">
            <div className="p-3 border-b">
              <Input
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search or create tag..."
                className="h-9"
              />
            </div>
            <div className="max-h-56 overflow-auto p-1">
              {filteredTags.length === 0 && !showCreateOption ? (
                <div className="px-3 py-6 text-center text-sm text-muted-foreground">
                  {searchQuery ? 'No matching tags' : 'No tags yet'}
                </div>
              ) : (
                <>
                  {filteredTags.map((tag) => {
                    const isSelected = selectedTagIds.includes(tag.id);
                    return (
                      <button
                        key={tag.id}
                        type="button"
                        className="w-full flex items-center gap-3 px-3 py-2 text-sm rounded-md hover:bg-accent transition-colors"
                        onClick={() => handleToggleTag(tag.id)}
                      >
                        <div className={`w-4 h-4 border rounded flex items-center justify-center transition-colors ${isSelected ? 'bg-primary border-primary text-primary-foreground' : 'border-muted-foreground/30'}`}>
                          {isSelected && <Check className="h-3 w-3" />}
                        </div>
                        {tag.color && (
                          <div
                            className="w-3 h-3 rounded-full shrink-0"
                            style={{ backgroundColor: tag.color }}
                          />
                        )}
                        <span className="flex-1 truncate text-left">{tag.name}</span>
                      </button>
                    );
                  })}
                  {showCreateOption && onTagCreate && (
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