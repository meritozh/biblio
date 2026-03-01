import { useState, useRef, useEffect } from 'react';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import type { Tag } from '@/types';

interface TagInputProps {
  availableTags: Tag[];
  selectedTags: Tag[];
  onTagSelect: (tag: Tag) => void;
  onTagCreate?: (name: string) => Promise<Tag>;
  placeholder?: string;
}

export function TagInput({
  availableTags,
  selectedTags,
  onTagSelect,
  onTagCreate,
  placeholder = 'Add a tag...',
}: TagInputProps) {
  const [inputValue, setInputValue] = useState('');
  const [showSuggestions, setShowSuggestions] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const filteredTags = availableTags.filter(
    (tag) =>
      !selectedTags.some((t) => t.id === tag.id) &&
      tag.name.toLowerCase().includes(inputValue.toLowerCase())
  );

  useEffect(() => {
    setShowSuggestions(inputValue.length > 0);
  }, [inputValue]);

  const handleSelect = (tag: Tag) => {
    onTagSelect(tag);
    setInputValue('');
    setShowSuggestions(false);
  };

  const handleKeyDown = async (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && inputValue.trim()) {
      e.preventDefault();
      const existingTag = availableTags.find(
        (t) => t.name.toLowerCase() === inputValue.toLowerCase()
      );
      if (existingTag) {
        handleSelect(existingTag);
      } else if (onTagCreate) {
        const newTag = await onTagCreate(inputValue.trim());
        handleSelect(newTag);
      }
    }
  };

  return (
    <div className="relative">
      <Input
        ref={inputRef}
        value={inputValue}
        onChange={(e) => setInputValue(e.target.value)}
        onKeyDown={handleKeyDown}
        onFocus={() => inputValue && setShowSuggestions(true)}
        onBlur={() => setTimeout(() => setShowSuggestions(false), 200)}
        placeholder={placeholder}
      />
      {showSuggestions && filteredTags.length > 0 && (
        <div className="absolute z-10 w-full mt-1 bg-popover border rounded-md shadow-md">
          {filteredTags.slice(0, 5).map((tag) => (
            <button
              key={tag.id}
              type="button"
              className="w-full px-3 py-2 text-left hover:bg-muted text-sm"
              onClick={() => handleSelect(tag)}
            >
              <Badge
                variant="secondary"
                style={tag.color ? { backgroundColor: tag.color, color: '#fff' } : undefined}
              >
                {tag.name}
              </Badge>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
