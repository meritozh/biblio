import { Badge } from '@/components/ui/badge';
import { X } from 'lucide-react';
import type { Author } from '@/types';

interface AuthorBadgeProps {
  author: Author;
  onRemove?: (author: Author) => void;
  clickable?: boolean;
  onClick?: (author: Author) => void;
}

export function AuthorBadge({ author, onRemove, clickable = false, onClick }: AuthorBadgeProps) {
  const handleClick = () => {
    if (clickable && onClick) {
      onClick(author);
    }
  };

  return (
    <Badge
      variant="outline"
      className={`gap-1 ${clickable ? 'cursor-pointer hover:bg-secondary/80' : ''}`}
      onClick={handleClick}
      role={clickable ? 'button' : undefined}
      tabIndex={clickable ? 0 : undefined}
      aria-label={clickable ? `Filter by author ${author.name}` : undefined}
      onKeyDown={(e) => {
        if (e.key === 'Enter' && clickable && onClick) {
          onClick(author);
        }
      }}
    >
      {author.name}
      {onRemove && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onRemove(author);
          }}
          className="ml-1 hover:bg-black/10 rounded-full p-0.5"
          aria-label={`Remove author ${author.name}`}
        >
          <X className="h-3 w-3" aria-hidden="true" />
        </button>
      )}
    </Badge>
  );
}