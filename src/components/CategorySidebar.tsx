import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Badge } from '@/components/ui/badge';
import { FolderOpen, Settings } from 'lucide-react';
import type { Category } from '@/types';

interface CategorySidebarProps {
  categories: Category[];
  selectedCategoryId: number | null;
  onCategorySelect: (categoryId: number | null) => void;
  onManageCategories?: () => void;
  fileCounts?: Record<number, number>;
}

export function CategorySidebar({
  categories,
  selectedCategoryId,
  onCategorySelect,
  onManageCategories,
  fileCounts = {},
}: CategorySidebarProps) {
  return (
    <div
      className="w-64 border-r border-sidebar-border bg-sidebar flex flex-col h-full"
      role="navigation"
      aria-label="Category navigation"
    >
      <div className="p-5 border-b border-sidebar-border">
        <h2 className="text-lg font-semibold text-sidebar-foreground" id="categories-heading">
          Categories
        </h2>
      </div>
      <ScrollArea className="flex-1">
        <div className="p-3" role="list" aria-labelledby="categories-heading">
          <button
            onClick={() => onCategorySelect(null)}
            className={`w-full text-left px-3 py-2.5 rounded-lg flex items-center justify-between transition-all duration-200 ${
              selectedCategoryId === null
                ? 'bg-primary text-primary-foreground'
                : 'hover:bg-secondary text-sidebar-foreground'
            }`}
            role="listitem"
            aria-current={selectedCategoryId === null ? 'page' : undefined}
          >
            <span className="flex items-center gap-2.5">
              <FolderOpen className="h-4 w-4" aria-hidden="true" />
              All Files
            </span>
          </button>
          {categories.map((category) => (
            <button
              key={category.id}
              onClick={() => onCategorySelect(category.id)}
              className={`w-full text-left px-3 py-2.5 rounded-lg flex items-center justify-between transition-all duration-200 ${
                selectedCategoryId === category.id
                  ? 'bg-primary text-primary-foreground'
                  : 'hover:bg-secondary text-sidebar-foreground'
              }`}
              role="listitem"
              aria-current={selectedCategoryId === category.id ? 'page' : undefined}
            >
              <span className="flex items-center gap-2.5">
                {category.icon && <span aria-hidden="true">{category.icon}</span>}
                {category.name}
              </span>
              {fileCounts[category.id] !== undefined && fileCounts[category.id]! > 0 && (
                <Badge
                  variant={selectedCategoryId === category.id ? 'outline' : 'secondary'}
                  className="text-xs"
                  aria-label={`${fileCounts[category.id]} files`}
                >
                  {fileCounts[category.id]}
                </Badge>
              )}
            </button>
          ))}
        </div>
      </ScrollArea>
      {onManageCategories && (
        <div className="p-4 border-t border-sidebar-border">
          <Button
            variant="outline"
            className="w-full"
            onClick={onManageCategories}
            aria-label="Manage categories"
          >
            <Settings className="h-4 w-4 mr-2" aria-hidden="true" />
            Manage Categories
          </Button>
        </div>
      )}
    </div>
  );
}