import { useState, useCallback, useEffect, useRef } from 'react';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Badge } from '@/components/ui/badge';
import { FolderOpen, Settings, GripVertical } from 'lucide-react';
import type { Category } from '@/types';

interface CategorySidebarProps {
  categories: Category[];
  selectedCategoryId: number | null;
  onCategorySelect: (categoryId: number | null) => void;
  onManageCategories?: () => void;
  onOpenSettings?: () => void;
  fileCounts?: Record<number, number>;
}

const MIN_WIDTH = 180;
const MAX_WIDTH = 320;
const DEFAULT_WIDTH = 240;

export function CategorySidebar({
  categories,
  selectedCategoryId,
  onCategorySelect,
  onManageCategories,
  onOpenSettings,
  fileCounts = {},
}: CategorySidebarProps) {
  const [width, setWidth] = useState(DEFAULT_WIDTH);
  const [isResizing, setIsResizing] = useState(false);
  const sidebarRef = useRef<HTMLDivElement>(null);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsResizing(true);
  }, []);

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!isResizing) return;
    const newWidth = e.clientX;
    setWidth(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, newWidth)));
  }, [isResizing]);

  const handleMouseUp = useCallback(() => {
    setIsResizing(false);
  }, []);

  useEffect(() => {
    if (isResizing) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = 'col-resize';
      document.body.style.userSelect = 'none';
    }
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [isResizing, handleMouseMove, handleMouseUp]);

  return (
    <div
      ref={sidebarRef}
      className="bg-sidebar border-r border-sidebar-border flex flex-col h-full relative"
      style={{ width: `${width}px`, minWidth: `${MIN_WIDTH}px`, maxWidth: `${MAX_WIDTH}px` }}
      role="navigation"
      aria-label="Category navigation"
    >
      {/* Spacer for transparent title bar traffic lights */}
      <div className="h-14 flex items-end px-3 pb-1" data-tauri-drag-region>
        <h2 className="px-3 text-xs font-medium text-muted-foreground uppercase tracking-wider" id="categories-heading">
          Categories
        </h2>
      </div>
      <ScrollArea className="flex-1">
        <div className="px-2 pb-2" role="list" aria-labelledby="categories-heading">
          <button
            onClick={() => onCategorySelect(null)}
            className={`w-full text-left px-3 py-1.5 rounded flex items-center justify-between transition-colors duration-100 ${selectedCategoryId === null
              ? 'bg-secondary text-foreground'
              : 'hover:bg-secondary/60 text-sidebar-foreground'
              }`}
            role="listitem"
            aria-current={selectedCategoryId === null ? 'page' : undefined}
          >
            <span className="flex items-center gap-2 text-sm">
              <FolderOpen className="h-4 w-4 shrink-0" aria-hidden="true" />
              All Files
            </span>
          </button>
          {categories.map((category) => (
            <button
              key={category.id}
              onClick={() => onCategorySelect(category.id)}
              className={`w-full text-left px-3 py-1.5 rounded flex items-center justify-between transition-colors duration-100 ${selectedCategoryId === category.id
                ? 'bg-secondary text-foreground'
                : 'hover:bg-secondary/60 text-sidebar-foreground'
                }`}
              role="listitem"
              aria-current={selectedCategoryId === category.id ? 'page' : undefined}
            >
              <span className="flex items-center gap-2 text-sm">
                {category.icon && <span aria-hidden="true">{category.icon}</span>}
                <span className="truncate">{category.name}</span>
              </span>
              {fileCounts[category.id] !== undefined && fileCounts[category.id]! > 0 && (
                <Badge
                  variant="gray"
                  className="text-xs font-normal"
                  aria-label={`${fileCounts[category.id]} files`}
                >
                  {fileCounts[category.id]}
                </Badge>
              )}
            </button>
          ))}
        </div>
      </ScrollArea>
      <div className="border-t border-sidebar-border">
        {onOpenSettings && (
          <Button
            variant="ghost"
            size="sm"
            className="w-full justify-start rounded-none text-muted-foreground hover:text-foreground"
            onClick={onOpenSettings}
            aria-label="Settings"
          >
            <Settings className="h-4 w-4 mr-2" aria-hidden="true" />
            Settings
          </Button>
        )}
        {onManageCategories && (
          <Button
            variant="ghost"
            size="sm"
            className="w-full justify-start rounded-none text-muted-foreground hover:text-foreground"
            onClick={onManageCategories}
            aria-label="Manage categories"
          >
            Manage Categories
          </Button>
        )}
      </div>

      {/* Resize handle */}
      <div
        className={`absolute top-0 right-0 w-1 h-full cursor-col-resize group ${isResizing ? 'bg-primary/20' : 'hover:bg-primary/10'
          }`}
        onMouseDown={handleMouseDown}
      >
        <div className="absolute top-1/2 right-0 -translate-y-1/2 translate-x-1/2 opacity-0 group-hover:opacity-100 transition-opacity">
          <GripVertical className="h-4 w-4 text-muted-foreground" />
        </div>
      </div>
    </div>
  );
}
