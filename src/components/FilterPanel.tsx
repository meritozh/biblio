import { Button } from '@/components/ui/button';
import { CategorySelect } from '@/components/CategorySelect';
import { Filter, X } from 'lucide-react';
import type { Category, Tag } from '@/types';

interface FilterPanelProps {
  categories: Category[];
  tags: Tag[];
  selectedCategoryId: number | null;
  selectedTagIds: number[];
  onCategoryChange: (categoryId: number | null) => void;
  onTagToggle: (tagId: number) => void;
  onClear: () => void;
}

export function FilterPanel({
  categories,
  tags,
  selectedCategoryId,
  selectedTagIds,
  onCategoryChange,
  onTagToggle,
  onClear,
}: FilterPanelProps) {
  const hasFilters = selectedCategoryId !== null || selectedTagIds.length > 0;

  return (
    <div className="space-y-3 p-4 border rounded-lg bg-muted/30">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-sm font-medium">
          <Filter className="h-4 w-4" />
          Filters
        </div>
        {hasFilters && (
          <Button variant="ghost" size="sm" onClick={onClear}>
            <X className="h-3 w-3 mr-1" />
            Clear
          </Button>
        )}
      </div>
      <div className="space-y-3">
        <div className="space-y-1">
          <label className="text-xs text-muted-foreground">Category</label>
          <CategorySelect
            categories={categories}
            value={selectedCategoryId}
            onValueChange={onCategoryChange}
            placeholder="All categories"
          />
        </div>
        {tags.length > 0 && (
          <div className="space-y-1">
            <label className="text-xs text-muted-foreground">Tags</label>
            <div className="flex flex-wrap gap-1">
              {tags.slice(0, 10).map((tag) => {
                const isSelected = selectedTagIds.includes(tag.id);
                return (
                  <button
                    key={tag.id}
                    type="button"
                    onClick={() => onTagToggle(tag.id)}
                    className={`rounded-full px-2 py-1 text-xs transition-colors ${
                      isSelected
                        ? 'bg-primary text-primary-foreground'
                        : 'bg-secondary hover:bg-secondary/80'
                    }`}
                  >
                    {tag.name}
                  </button>
                );
              })}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
