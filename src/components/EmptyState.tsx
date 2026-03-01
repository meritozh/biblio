import { FileX, FolderOpen, Search } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface EmptyStateProps {
  type: 'files' | 'search' | 'category';
  message?: string;
  action?: {
    label: string;
    onClick: () => void;
  };
}

export function EmptyState({ type, message, action }: EmptyStateProps) {
  const icons = {
    files: FolderOpen,
    search: Search,
    category: FileX,
  };

  const defaultMessages = {
    files: 'No files in your library yet',
    search: 'No files match your search',
    category: 'No files in this category',
  };

  const Icon = icons[type];

  return (
    <div className="flex flex-col items-center justify-center py-12 text-center">
      <Icon className="h-12 w-12 text-muted-foreground/50 mb-4" />
      <p className="text-muted-foreground mb-4">{message || defaultMessages[type]}</p>
      {action && (
        <Button variant="outline" onClick={action.onClick}>
          {action.label}
        </Button>
      )}
    </div>
  );
}
