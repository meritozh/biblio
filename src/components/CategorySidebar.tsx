import { useState, useCallback, useEffect, useRef } from 'react';
import { Link } from '@tanstack/react-router';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Badge } from '@/components/ui/badge';
import {
  BookOpen,
  LayoutGrid,
  MessageSquare,
  Settings,
  Shapes,
  Sparkles,
  Tag,
  User,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import type { Category } from '@/types';

interface CategorySidebarProps {
  categories: Category[];
  selectedCategoryId: number | null;
  onCategorySelect: (categoryId: number | null) => void;
  onManageCategories?: () => void;
  onOpenSettings?: () => void;
  onNavigate?: () => void;
  fileCounts?: Record<number, number>;
  currentPath?: string;
  className?: string;
  autoFocus?: boolean;
  resizable?: boolean;
}

const NAV_GROUPS = [
  {
    label: 'Manage',
    links: [
      { to: '/categories', label: 'Categories', icon: LayoutGrid },
      { to: '/tags', label: 'Tags', icon: Tag },
      { to: '/authors', label: 'Authors', icon: User },
      { to: '/cleanup', label: 'Cleanup', icon: Sparkles },
    ],
  },
  {
    label: 'Advanced',
    links: [
      { to: '/schemas', label: 'Schemas', icon: Shapes },
      { to: '/prompts', label: 'Prompts', icon: MessageSquare },
    ],
  },
] as const;

const MIN_WIDTH = 200;
const MAX_WIDTH = 320;
const DEFAULT_WIDTH = 232;

export function CategorySidebar({
  categories,
  selectedCategoryId,
  onCategorySelect,
  onManageCategories,
  onOpenSettings,
  onNavigate,
  fileCounts = {},
  currentPath,
  className,
  autoFocus = false,
  resizable = true,
}: CategorySidebarProps) {
  const [width, setWidth] = useState(DEFAULT_WIDTH);
  const [isResizing, setIsResizing] = useState(false);
  const sidebarRef = useRef<HTMLDivElement>(null);

  const handleMouseDown = useCallback((event: React.MouseEvent) => {
    event.preventDefault();
    setIsResizing(true);
  }, []);

  const handleMouseMove = useCallback(
    (event: MouseEvent) => {
      if (!isResizing) return;
      setWidth(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, event.clientX)));
    },
    [isResizing]
  );

  const handleMouseUp = useCallback(() => {
    setIsResizing(false);
  }, []);

  useEffect(() => {
    if (autoFocus) sidebarRef.current?.querySelector<HTMLElement>('button, a')?.focus();
  }, [autoFocus]);

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

  const navigationItemClass = (active: boolean) =>
    cn(
      'flex h-8 w-full items-center gap-2.5 rounded-md px-2.5 text-left text-[13px] font-medium outline-none transition-[background-color,color,scale] duration-100 focus-visible:ring-2 focus-visible:ring-ring/40 active:scale-[0.98]',
      active
        ? 'bg-primary/10 text-primary'
        : 'text-sidebar-foreground/80 hover:bg-foreground/[0.055] hover:text-foreground'
    );

  return (
    <div
      ref={sidebarRef}
      tabIndex={-1}
      className={cn(
        'relative flex h-full flex-col border-r border-sidebar-border/80 bg-sidebar text-sidebar-foreground outline-none',
        className
      )}
      style={
        resizable
          ? { width: `${width}px`, minWidth: `${MIN_WIDTH}px`, maxWidth: `${MAX_WIDTH}px` }
          : undefined
      }
      role="navigation"
      aria-label="Main navigation"
    >
      <div className="flex h-16 shrink-0 items-end px-4 pb-3" data-tauri-drag-region>
        <div className={cn('flex items-center gap-2', resizable && 'pl-[58px]')}>
          <BookOpen className="h-4 w-4 text-primary" strokeWidth={1.9} aria-hidden="true" />
          <span className="text-[13px] font-semibold tracking-[-0.01em]">Biblio</span>
        </div>
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <div className="px-3 pb-5">
          <section aria-labelledby={resizable ? 'library-navigation' : 'mobile-library-navigation'}>
            <h2
              id={resizable ? 'library-navigation' : 'mobile-library-navigation'}
              className="px-2.5 pb-1.5 text-[11px] font-semibold text-muted-foreground"
            >
              Library
            </h2>
            <div className="space-y-0.5">
              {categories.map((category) => {
                const active = selectedCategoryId === category.id;
                return (
                  <button
                    key={category.id}
                    type="button"
                    onClick={() => {
                      onCategorySelect(category.id);
                      onNavigate?.();
                    }}
                    className={cn(navigationItemClass(active), 'justify-between')}
                    aria-current={active ? 'page' : undefined}
                  >
                    <span className="flex min-w-0 items-center gap-2">
                      <span
                        className="w-5 shrink-0 text-center text-[15px] leading-none"
                        aria-hidden="true"
                      >
                        {category.icon ?? <BookOpen className="mx-auto h-4 w-4 opacity-60" />}
                      </span>
                      <span className="truncate">{category.name}</span>
                    </span>
                    {fileCounts[category.id] !== undefined && fileCounts[category.id]! > 0 && (
                      <Badge
                        variant="gray"
                        className="text-[11px] font-normal [font-variant-numeric:tabular-nums]"
                        aria-label={`${fileCounts[category.id]} files`}
                      >
                        {fileCounts[category.id]}
                      </Badge>
                    )}
                  </button>
                );
              })}
            </div>
          </section>

          {NAV_GROUPS.map((group, groupIndex) => {
            const headingId = `${resizable ? 'sidebar' : 'mobile-sidebar'}-${group.label.toLowerCase()}`;
            return (
              <section key={group.label} className="mt-6" aria-labelledby={headingId}>
                <h2
                  id={headingId}
                  className="px-2.5 pb-1.5 text-[11px] font-semibold text-muted-foreground"
                >
                  {group.label}
                </h2>
                <div className="space-y-0.5">
                  {group.links.map(({ to, label, icon: Icon }) => (
                    <Link
                      key={to}
                      to={to}
                      onClick={onNavigate}
                      aria-current={currentPath === to ? 'page' : undefined}
                      className={navigationItemClass(currentPath === to)}
                    >
                      <Icon
                        className="h-4 w-4 shrink-0 opacity-75"
                        strokeWidth={1.8}
                        aria-hidden="true"
                      />
                      <span className="truncate">{label}</span>
                    </Link>
                  ))}
                </div>
                {groupIndex === 0 && (
                  <div className="mx-2.5 mt-5 border-t border-sidebar-border/60" />
                )}
              </section>
            );
          })}
        </div>
      </ScrollArea>

      <div className="shrink-0 border-t border-sidebar-border/80 p-3">
        {onOpenSettings && (
          <Button
            variant="ghost"
            size="sm"
            className="h-8 w-full justify-start text-muted-foreground hover:bg-foreground/[0.055] hover:text-foreground"
            onClick={() => {
              onOpenSettings();
              onNavigate?.();
            }}
            aria-label="Settings"
          >
            <Settings className="h-4 w-4 opacity-75" strokeWidth={1.8} aria-hidden="true" />
            Settings
          </Button>
        )}
        {onManageCategories && (
          <Button
            variant="ghost"
            size="sm"
            className="h-8 w-full justify-start text-muted-foreground hover:bg-foreground/[0.055] hover:text-foreground"
            onClick={onManageCategories}
            aria-label="Manage categories"
          >
            Manage Categories
          </Button>
        )}
      </div>

      {resizable && (
        <div
          className={cn(
            'absolute right-0 top-0 hidden h-full w-1.5 cursor-col-resize md:block',
            isResizing ? 'bg-primary/15' : 'hover:bg-primary/10'
          )}
          onMouseDown={handleMouseDown}
          aria-hidden="true"
        />
      )}
    </div>
  );
}
