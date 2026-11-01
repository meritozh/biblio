import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Search, X } from 'lucide-react';
import { describeCondition, type Condition } from '@/lib/filters';
import {
  progressContainsCondition,
  suggestFieldValues,
  suggestFields,
  type SearchFieldKey,
  type SearchSuggestion,
} from '@/lib/searchTokens';
import { cn } from '@/lib/utils';
import type { Author, Tag } from '@/types';

interface TokenSearchBarProps {
  /** Structured tokens — same `Condition[]` owned by the Library filter. */
  conditions: Condition[];
  onConditionsChange: (next: Condition[]) => void;
  /** Live free-text buffer (not yet committed). */
  freeText: string;
  onFreeTextChange: (value: string) => void;
  /** Debounced / Enter / clear commit of free text. */
  onFreeTextCommit: (value: string) => void;
  tags?: ReadonlyArray<Tag>;
  authors?: ReadonlyArray<Author>;
  placeholder?: string;
  debounceMs?: number;
  className?: string;
}

/**
 * iOS Photos-style search: field keywords become filter chips via
 * autocomplete; leftover text is free-text search over title/path.
 */
export function TokenSearchBar({
  conditions,
  onConditionsChange,
  freeText,
  onFreeTextChange,
  onFreeTextCommit,
  tags = [],
  authors = [],
  placeholder = 'Search or filter: tag, author, cache…',
  debounceMs = 300,
  className,
}: TokenSearchBarProps) {
  const listboxId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [highlight, setHighlight] = useState(0);
  const [pendingField, setPendingField] = useState<SearchFieldKey | null>(null);
  // Arrow keys mark the listbox as "actively navigated". Enter only accepts
  // a highlighted suggestion after that — plain Enter always commits free
  // text, so aliases like `star` / `by` / `tag` can still be phrase search.
  const [listNavigated, setListNavigated] = useState(false);

  const onCommitRef = useRef(onFreeTextCommit);
  useEffect(() => {
    onCommitRef.current = onFreeTextCommit;
  }, [onFreeTextCommit]);

  const isFirstRun = useRef(true);
  useEffect(() => {
    if (isFirstRun.current) {
      isFirstRun.current = false;
      return;
    }
    // Don't debounce-commit while composing a structured token value.
    if (pendingField) return;
    const id = window.setTimeout(() => {
      onCommitRef.current(freeText);
    }, debounceMs);
    return () => window.clearTimeout(id);
  }, [freeText, debounceMs, pendingField]);

  const tagsById = useMemo(() => new Map(tags.map((t) => [t.id, t])), [tags]);
  const authorsById = useMemo(() => new Map(authors.map((a) => [a.id, a])), [authors]);

  const suggestions: SearchSuggestion[] = useMemo(() => {
    if (!open) return [];
    if (pendingField) {
      return suggestFieldValues(pendingField, freeText, { tags, authors });
    }
    return suggestFields(freeText);
  }, [open, pendingField, freeText, tags, authors]);

  useEffect(() => {
    setHighlight(0);
    setListNavigated(false);
  }, [suggestions, pendingField]);

  useEffect(() => {
    const onPointerDown = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    window.addEventListener('mousedown', onPointerDown);
    return () => window.removeEventListener('mousedown', onPointerDown);
  }, []);

  const addCondition = (condition: Condition) => {
    onConditionsChange([...conditions, condition]);
    setPendingField(null);
    onFreeTextChange('');
    onFreeTextCommit('');
    setOpen(false);
    inputRef.current?.focus();
  };

  const removeCondition = (id: string) => {
    onConditionsChange(conditions.filter((c) => c.id !== id));
  };

  const selectSuggestion = (suggestion: SearchSuggestion) => {
    if (suggestion.kind === 'field') {
      setPendingField(suggestion.field);
      onFreeTextChange('');
      setOpen(true);
      inputRef.current?.focus();
      return;
    }
    addCondition(suggestion.condition);
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Backspace' && freeText === '') {
      if (pendingField) {
        event.preventDefault();
        setPendingField(null);
        setOpen(false);
        return;
      }
      if (conditions.length > 0) {
        event.preventDefault();
        removeCondition(conditions[conditions.length - 1]!.id);
      }
      return;
    }

    if (event.key === 'Escape') {
      if (pendingField) {
        setPendingField(null);
        onFreeTextChange('');
        setOpen(false);
        return;
      }
      setOpen(false);
      return;
    }

    if (event.key === 'ArrowDown' && suggestions.length > 0) {
      event.preventDefault();
      setOpen(true);
      setListNavigated(true);
      setHighlight((h) => (listNavigated ? (h + 1) % suggestions.length : 0));
      return;
    }
    if (event.key === 'ArrowUp' && suggestions.length > 0) {
      event.preventDefault();
      setOpen(true);
      setListNavigated(true);
      setHighlight((h) =>
        listNavigated ? (h - 1 + suggestions.length) % suggestions.length : suggestions.length - 1
      );
      return;
    }

    // Tab explicitly accepts the highlighted suggestion (field or value).
    if (event.key === 'Tab' && open && suggestions[highlight]) {
      event.preventDefault();
      selectSuggestion(suggestions[highlight]!);
      return;
    }

    if (event.key === 'Enter') {
      event.preventDefault();

      // Value-mode: the user already picked a field chip, so Enter accepts
      // the highlighted value (no second arrow pass). `listNavigated` is
      // reset when suggestions swap to values after field selection — requiring
      // it here cleared the free-text buffer and then ate the confirming Enter.
      if (pendingField) {
        if (open && suggestions[highlight]) {
          selectSuggestion(suggestions[highlight]!);
          return;
        }
        if (pendingField === 'progress') {
          const condition = progressContainsCondition(freeText);
          if (condition) {
            addCondition(condition);
            return;
          }
        }
        return;
      }

      // Free-text mode: only accept a field suggestion after arrow / hover
      // navigation. Plain Enter commits the typed phrase — including words
      // that happen to be field aliases (`star wars`, `by the sea`, `tag team`).
      if (open && listNavigated && suggestions[highlight]) {
        selectSuggestion(suggestions[highlight]!);
        return;
      }

      onFreeTextCommit(freeText);
      setOpen(false);
    }
  };

  const handleClear = () => {
    setPendingField(null);
    onFreeTextChange('');
    onFreeTextCommit('');
    onConditionsChange([]);
    setOpen(false);
    inputRef.current?.focus();
  };

  const inputPlaceholder = pendingField
    ? pendingField === 'progress'
      ? 'Type progress text…'
      : `Choose ${pendingField.replace('_', ' ')}…`
    : conditions.length > 0
      ? 'Add filter or search…'
      : placeholder;

  const showClear = freeText.length > 0 || conditions.length > 0 || pendingField != null;

  return (
    <div ref={rootRef} className={cn('relative w-full', className)}>
      <div
        className={cn(
          'flex min-h-9 w-full flex-wrap items-center gap-1.5 rounded-md border border-input/70 bg-card px-2 py-1 shadow-[0_1px_2px_rgba(0,0,0,0.04)] transition-[border-color,box-shadow]',
          'focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/25'
        )}
        onClick={() => inputRef.current?.focus()}
      >
        <Search className="ml-1 h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />

        {conditions.map((c) => (
          <span
            key={c.id}
            className="inline-flex h-6 max-w-[180px] items-center gap-1 rounded-md bg-secondary px-1.5 text-[11px] font-medium text-secondary-foreground"
          >
            <span className="truncate">{describeCondition(c, tagsById, authorsById)}</span>
            <button
              type="button"
              className="rounded p-0.5 text-muted-foreground hover:text-foreground"
              aria-label={`Remove ${describeCondition(c, tagsById, authorsById)}`}
              onClick={(e) => {
                e.stopPropagation();
                removeCondition(c.id);
              }}
            >
              <X className="h-3 w-3" aria-hidden="true" />
            </button>
          </span>
        ))}

        {pendingField && (
          <span className="inline-flex h-6 items-center rounded-md bg-primary/10 px-1.5 text-[11px] font-medium text-primary">
            {pendingField === 'local_cache'
              ? 'Cached'
              : pendingField === 'file_status'
                ? 'Status'
                : pendingField === 'storage_kind'
                  ? 'Storage'
                  : pendingField.charAt(0).toUpperCase() + pendingField.slice(1)}
          </span>
        )}

        <input
          ref={inputRef}
          type="search"
          value={freeText}
          onChange={(e) => {
            onFreeTextChange(e.target.value);
            setOpen(true);
          }}
          onFocus={() => setOpen(true)}
          onKeyDown={handleKeyDown}
          placeholder={inputPlaceholder}
          className="min-w-[8rem] flex-1 bg-transparent py-1 text-[13px] outline-none placeholder:text-muted-foreground/80 [&::-webkit-search-cancel-button]:hidden [&::-webkit-search-decoration]:hidden"
          aria-label="Search files and filters"
          aria-autocomplete="list"
          aria-controls={listboxId}
          aria-expanded={open && suggestions.length > 0}
          role="combobox"
        />

        {showClear && (
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 shrink-0"
            onClick={handleClear}
            aria-label="Clear search"
          >
            <X className="h-4 w-4" aria-hidden="true" />
          </Button>
        )}
      </div>

      {open && suggestions.length > 0 && (
        <ul
          id={listboxId}
          role="listbox"
          className="absolute z-50 mt-1 max-h-64 w-full overflow-y-auto rounded-xl border border-border/60 bg-popover p-1 text-popover-foreground shadow-[0_12px_32px_rgba(0,0,0,0.14),0_2px_8px_rgba(0,0,0,0.05)]"
        >
          {suggestions.map((suggestion, index) => {
            const selected = index === highlight;
            const label =
              suggestion.kind === 'field'
                ? suggestion.label
                : suggestion.label;
            const hint =
              suggestion.kind === 'field'
                ? suggestion.aliases[0]
                : undefined;
            return (
              <li key={`${suggestion.kind}-${label}-${index}`} role="option" aria-selected={selected}>
                <button
                  type="button"
                  className={cn(
                    'flex w-full items-center justify-between rounded-lg px-2.5 py-1.5 text-left text-sm',
                    selected ? 'bg-accent text-accent-foreground' : 'hover:bg-secondary/70'
                  )}
                  onMouseEnter={() => {
                    setHighlight(index);
                    // Hover counts as explicit navigation so Enter can accept
                    // a field suggestion without a prior ArrowDown.
                    setListNavigated(true);
                  }}
                  onClick={() => selectSuggestion(suggestion)}
                >
                  <span className="truncate font-medium">{label}</span>
                  {hint && (
                    <span className="ml-2 shrink-0 text-[11px] text-muted-foreground">{hint}</span>
                  )}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
