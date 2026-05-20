import { useState, useMemo, useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { TagBadge } from '@/components/TagBadge';
import { PaginatedPicker, type PickerPage } from '@/components/PaginatedPicker';
import { Plus } from 'lucide-react';
import { tagList } from '@/lib/tauri';
import type { Tag } from '@/types';

interface TagManagerProps {
  /** Display-only lookup for the selected-chips row. The picker body
   *  fetches its own data so this list can be small / stale — only the
   *  rows the parent already had cached. */
  tags: Tag[];
  selectedTagIds: number[];
  onTagAssign: (tagIds: number[]) => void;
  onTagCreate?: (name: string) => Promise<Tag>;
}

/** Tag picker with selected-chips display + Popover containing a
 *  paginated, virtualized, searchable list. Compatible drop-in for the
 *  old all-tags-in-memory shape: parents keep passing the cached `tags`
 *  array for chip rendering, the popover fetches independently. */
export function TagManager({
  tags,
  selectedTagIds,
  onTagAssign,
  onTagCreate,
}: TagManagerProps) {
  const [open, setOpen] = useState(false);

  const selectedTags = useMemo(
    () => tags.filter((t) => selectedTagIds.includes(t.id)),
    [tags, selectedTagIds]
  );

  const handleRemoveTag = (tag: Tag) => {
    onTagAssign(selectedTagIds.filter((id) => id !== tag.id));
  };

  // Adapter from PaginatedPicker's generic fetcher contract to tagList.
  // Stable identity so the picker's effect deps don't churn.
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
      const { tags: page } = await tagList({
        limit,
        offset,
        nameQuery: query.length > 0 ? query : undefined,
      });
      // The picker's total is used for load-more cutoff. Approximate it
      // from the page size: if we got a full page, assume there's more;
      // if we got less, this is the last page. Avoids a parallel
      // tag_count call on every keystroke.
      const total = page.length < limit ? offset + page.length : offset + page.length + 1;
      return {
        items: page.map((t) => ({ id: t.id, name: t.name, color: t.color })),
        total,
      };
    },
    []
  );

  const handleCreate = useCallback(
    async (name: string) => {
      if (!onTagCreate) {
        throw new Error('onTagCreate not provided');
      }
      const created = await onTagCreate(name);
      return { id: created.id, name: created.name, color: created.color };
    },
    [onTagCreate]
  );

  return (
    <div className="space-y-2">
      {selectedTags.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {selectedTags.map((tag) => (
            <TagBadge key={tag.id} tag={tag} onRemove={handleRemoveTag} />
          ))}
        </div>
      )}

      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button variant="outline" size="sm" className="h-9 text-muted-foreground">
            <Plus className="h-4 w-4 mr-1" />
            {selectedTags.length > 0 ? 'Edit tags' : 'Add tags'}
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-72 p-0" align="start" disablePortal>
          <PaginatedPicker
            mode="multi"
            selectedIds={selectedTagIds}
            fetcher={fetcher}
            onToggle={onTagAssign}
            onCreate={onTagCreate ? handleCreate : undefined}
            searchPlaceholder="Search or create tag…"
            emptyLabel="No tags yet"
            noMatchLabel="No matching tags"
          />
        </PopoverContent>
      </Popover>
    </div>
  );
}
