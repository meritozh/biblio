import { FileX, FolderOpen, Search } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { Button } from '@/components/ui/button';

const PRESET_ICONS = {
  files: FolderOpen,
  search: Search,
  category: FileX,
} as const;

const PRESET_MESSAGES = {
  files: 'No files in your library yet',
  search: 'No files match your search',
  category: 'No files in this category',
} as const;

interface EmptyStateProps {
  /** Library presets (icon + default copy). Management pages skip this
   *  and pass their own `icon` + `message` instead. */
  type?: 'files' | 'search' | 'category';
  icon?: LucideIcon;
  message?: string;
  action?: {
    label: string;
    onClick: () => void;
  };
}

export function EmptyState({ type = 'files', icon, message, action }: EmptyStateProps) {
  const Icon = icon ?? PRESET_ICONS[type];
  const text = message ?? PRESET_MESSAGES[type];

  return (
    <div className="flex flex-col items-center justify-center py-12 text-center">
      <Icon className="h-12 w-12 text-muted-foreground/50 mb-4" aria-hidden="true" />
      <p className="text-muted-foreground mb-4">{text}</p>
      {action && (
        <Button variant="outline" onClick={action.onClick}>
          {action.label}
        </Button>
      )}
    </div>
  );
}
