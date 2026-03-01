import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { TagBadge } from '@/components/TagBadge';
import { Plus, X } from 'lucide-react';
import type { Tag } from '@/types';

interface TagManagerProps {
  tags: Tag[];
  selectedTagIds: number[];
  onTagAssign: (tagIds: number[]) => void;
  onTagCreate?: (name: string) => Promise<Tag>;
}

export function TagManager({ tags, selectedTagIds, onTagAssign, onTagCreate }: TagManagerProps) {
  const [isCreating, setIsCreating] = useState(false);
  const [newTagName, setNewTagName] = useState('');

  const selectedTags = tags.filter((t) => selectedTagIds.includes(t.id));

  const handleAddTag = async () => {
    if (newTagName.trim()) {
      if (onTagCreate) {
        const newTag = await onTagCreate(newTagName.trim());
        onTagAssign([...selectedTagIds, newTag.id]);
      }
      setNewTagName('');
      setIsCreating(false);
    }
  };

  const handleRemoveTag = (tag: Tag) => {
    onTagAssign(selectedTagIds.filter((id) => id !== tag.id));
  };

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap gap-2">
        {selectedTags.map((tag) => (
          <TagBadge key={tag.id} tag={tag} onRemove={handleRemoveTag} />
        ))}
        {isCreating ? (
          <div className="flex gap-1">
            <Input
              value={newTagName}
              onChange={(e) => setNewTagName(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleAddTag()}
              placeholder="Tag name"
              className="h-6 w-24 text-xs"
              autoFocus
            />
            <Button size="sm" variant="ghost" onClick={handleAddTag} className="h-6 px-2">
              <Plus className="h-3 w-3" />
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                setIsCreating(false);
                setNewTagName('');
              }}
              className="h-6 px-2"
            >
              <X className="h-3 w-3" />
            </Button>
          </div>
        ) : (
          <Button
            size="sm"
            variant="outline"
            onClick={() => setIsCreating(true)}
            className="h-6 text-xs"
          >
            <Plus className="h-3 w-3 mr-1" />
            Add Tag
          </Button>
        )}
      </div>
    </div>
  );
}
