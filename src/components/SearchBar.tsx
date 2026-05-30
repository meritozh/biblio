import { useEffect, useRef } from 'react';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Search, X } from 'lucide-react';

interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
  onSearch: (query: string) => void;
  placeholder?: string;
  /** Delay before auto-committing the typed value via `onSearch`. Enter and
   *  the clear button bypass this and fire immediately. */
  debounceMs?: number;
}

export function SearchBar({
  value,
  onChange,
  onSearch,
  placeholder = 'Search files...',
  debounceMs = 300,
}: SearchBarProps) {
  // Latest onSearch in a ref so the debounce effect doesn't re-arm just
  // because the parent re-rendered with a new function identity.
  const onSearchRef = useRef(onSearch);
  useEffect(() => {
    onSearchRef.current = onSearch;
  }, [onSearch]);

  // Skip the initial commit so mounting with a pre-filled value doesn't
  // fire a redundant search before the user has typed anything. Track the
  // first effect run rather than comparing against the mount value:
  // comparing meant that editing back to the initial value (e.g. deleting
  // all input down to the empty string the search mounts with) short-
  // circuited the commit, so `onSearch('')` never fired and the results
  // stayed stuck on the last query — diverging from the clear button,
  // which calls `onSearch('')` directly.
  const isFirstRun = useRef(true);
  useEffect(() => {
    if (isFirstRun.current) {
      isFirstRun.current = false;
      return;
    }
    const id = window.setTimeout(() => {
      onSearchRef.current(value);
    }, debounceMs);
    return () => window.clearTimeout(id);
  }, [value, debounceMs]);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      onSearch(value);
    }
  };

  const handleClear = () => {
    onChange('');
    onSearch('');
  };

  // Deliberately no focus-ring on this wrapper — the underlying <Input>
  // primitive already applies a ring that matches its own border radius
  // and border color. Layering a second ring here produced a double
  // outline at a different radius.
  return (
    <div className="relative flex items-center">
      <Search
        className="absolute left-3 h-4 w-4 text-muted-foreground pointer-events-none"
        aria-hidden="true"
      />
      <Input
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        // Hide WebKit's native ::-webkit-search-cancel-button and the
        // related search decoration. `type="search"` is retained for the
        // semantic role + keyboard affordances (Escape clears in Safari),
        // but the visible clear button is the custom <X> below — the
        // browser-native glyph would render alongside it as a duplicate.
        className="pl-9 pr-9 [&::-webkit-search-cancel-button]:hidden [&::-webkit-search-decoration]:hidden"
        aria-label="Search files"
      />
      {value && (
        <Button
          variant="ghost"
          size="icon"
          className="absolute right-1 h-7 w-7"
          onClick={handleClear}
          aria-label="Clear search"
        >
          <X className="h-4 w-4" aria-hidden="true" />
        </Button>
      )}
    </div>
  );
}
