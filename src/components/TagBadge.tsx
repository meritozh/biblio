import { Badge } from '@/components/ui/badge';
import { X } from 'lucide-react';
import type { Tag } from '@/types';

interface TagBadgeProps {
  tag: Tag;
  onRemove?: (tag: Tag) => void;
  clickable?: boolean;
  onClick?: (tag: Tag) => void;
}

export function TagBadge({ tag, onRemove, clickable = false, onClick }: TagBadgeProps) {
  const handleClick = () => {
    if (clickable && onClick) {
      onClick(tag);
    }
  };

  return (
    <Badge
      variant="secondary"
      className={`gap-1 ${clickable ? 'cursor-pointer hover:bg-secondary/80' : ''}`}
      style={tag.color ? { backgroundColor: tag.color, color: '#fff' } : undefined}
      onClick={handleClick}
      role={clickable ? 'button' : undefined}
      tabIndex={clickable ? 0 : undefined}
      aria-label={clickable ? `Filter by tag ${tag.name}` : undefined}
      onKeyDown={(e) => {
        if (e.key === 'Enter' && clickable && onClick) {
          onClick(tag);
        }
      }}
    >
      {tag.name}
      {onRemove && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onRemove(tag);
          }}
          className="ml-1 hover:bg-black/10 rounded-full p-0.5"
          aria-label={`Remove tag ${tag.name}`}
        >
          <X className="h-3 w-3" aria-hidden="true" />
        </button>
      )}
    </Badge>
  );
}
