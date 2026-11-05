import { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Search, X } from 'lucide-react';
import { describeCondition, type Condition } from '@/lib/filters';
import {
  matchFieldKeyword,
  progressContainsCondition,
  suggestFieldValues,
  suggestSearchList,
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
 *
 * Combobox keyboard model:
 * - `highlight === -1`: no active candidate → Enter commits free text
 * - ArrowUp/Down or hover moves `highlight` onto a candidate → Enter accepts it
 * - A value candidate becomes a filter chip. The typed text stays in the box
 *   and is committed with the chip, so the list refreshes and nothing is wiped
 * - A field candidate opens that field's values without clearing the box
 * - Aliases like `star` / `tag` stay searchable as phrases unless the user
 *   explicitly navigates the list (or clicks / Tab)
 */

/** Value-list filter. A bare field keyword (`tag`) must not hide every value. */
function valueQueryFor(field: SearchFieldKey, text: string): string {
  const trimmed = text.trim();
  if (matchFieldKeyword(trimmed)?.field === field) return '';
  return trimmed;
}
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
  const setInputRef = useCallback((node: HTMLInputElement | null) => {
    inputRef.current = node;
    // Not in React's input typings. macOS uses this to show the typo /
    // writing-suggestion popup that swallows ArrowDown.
    node?.setAttribute('writingsuggestions', 'false');
  }, []);
  const rootRef = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  /** `-1` = no active candidate (free-text Enter). `>= 0` = list selection. */
  const [highlight, setHighlight] = useState(-1);
  /** True only after arrow/hover. Auto-arming the first value for Enter must
   *  not make Tab accept that row and trap focus. */
  const [highlightChosen, setHighlightChosen] = useState(false);
  const [pendingField, setPendingField] = useState<SearchFieldKey | null>(null);

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
    const id = window.setTimeout(() => {
      onCommitRef.current(freeText);
    }, debounceMs);
    return () => window.clearTimeout(id);
  }, [freeText, debounceMs]);

  const tagsById = useMemo(() => new Map(tags.map((t) => [t.id, t])), [tags]);
  const authorsById = useMemo(() => new Map(authors.map((a) => [a.id, a])), [authors]);

  const suggestions: SearchSuggestion[] = useMemo(() => {
    if (!open) return [];
    if (pendingField) {
      return suggestFieldValues(pendingField, valueQueryFor(pendingField, freeText), {
        tags,
        authors,
      });
    }
    return suggestSearchList(freeText, { tags, authors });
  }, [open, pendingField, freeText, tags, authors]);

  // Clamp highlight if the list shrinks under us (e.g. value filter narrows).
  // Do not reset highlight when `suggestions` gets a new array identity —
  // parent re-renders were clearing the armed candidate so Enter committed
  // free text (or wiped the box) instead of accepting the highlight.
  useEffect(() => {
    if (highlight >= suggestions.length) {
      setHighlight(suggestions.length > 0 ? suggestions.length - 1 : -1);
    }
  }, [suggestions.length, highlight]);

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
    // Keep whatever the user typed. The chip is added beside it, and the
    // current text is committed so the list refreshes with both.
    onFreeTextCommit(freeText);
    setOpen(false);
    setHighlight(-1);
    setHighlightChosen(false);
    inputRef.current?.focus();
  };

  const removeCondition = (id: string) => {
    onConditionsChange(conditions.filter((c) => c.id !== id));
  };

  const selectSuggestion = (suggestion: SearchSuggestion) => {
    if (suggestion.kind === 'field') {
      // Open the value list. Do not clear the box — that was the "Enter reset
      // the search" bug. A bare keyword is ignored when filtering values.
      setPendingField(suggestion.field);
      setOpen(true);
      setHighlight(-1);
      setHighlightChosen(false);
      inputRef.current?.focus();
      return;
    }
    addCondition(suggestion.condition);
  };

  const acceptHighlight = (): boolean => {
    if (!open || highlight < 0) return false;
    const suggestion = suggestions[highlight];
    if (!suggestion) return false;
    selectSuggestion(suggestion);
    return true;
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    // Don't steal Enter/Backspace while an IME candidate window is open.
    if (event.nativeEvent.isComposing || event.keyCode === 229) {
      return;
    }

    if (event.key === 'Backspace' && freeText === '') {
      if (pendingField) {
        event.preventDefault();
        setPendingField(null);
        setOpen(false);
        setHighlight(-1);
        setHighlightChosen(false);
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
        setOpen(false);
        setHighlight(-1);
        setHighlightChosen(false);
        return;
      }
      setOpen(false);
      setHighlight(-1);
      setHighlightChosen(false);
      return;
    }

    if (event.key === 'ArrowDown' && suggestions.length > 0) {
      event.preventDefault();
      setOpen(true);
      setHighlightChosen(true);
      setHighlight((h) => (h < 0 ? 0 : (h + 1) % suggestions.length));
      return;
    }
    if (event.key === 'ArrowUp' && suggestions.length > 0) {
      event.preventDefault();
      setOpen(true);
      setHighlightChosen(true);
      setHighlight((h) =>
        h < 0 ? suggestions.length - 1 : (h - 1 + suggestions.length) % suggestions.length
      );
      return;
    }

    // Tab accepts only a row the user arrowed to or hovered. Opening a field
    // does not arm a value, so Tab leaves the combobox.
    if (
      event.key === 'Tab' &&
      open &&
      highlightChosen &&
      highlight >= 0 &&
      suggestions[highlight]
    ) {
      event.preventDefault();
      selectSuggestion(suggestions[highlight]!);
      return;
    }

    if (event.key === 'Enter') {
      event.preventDefault();
      event.stopPropagation();

      if (pendingField) {
        // Only a row the user arrowed to or hovered. Do not apply index 0
        // just because the value list opened, and do not clear the text.
        if (acceptHighlight()) return;
        if (pendingField === 'progress') {
          const condition = progressContainsCondition(freeText);
          if (condition) {
            addCondition(condition);
            return;
          }
        }
        return;
      }

      // Free-text mode: only accept a candidate after arrow/hover moved
      // highlight off -1. Plain Enter commits the typed phrase.
      if (acceptHighlight()) return;

      onFreeTextCommit(freeText);
      setOpen(false);
      setHighlight(-1);
      setHighlightChosen(false);
    }
  };

  const handleClear = () => {
    setPendingField(null);
    onFreeTextChange('');
    onFreeTextCommit('');
    onConditionsChange([]);
    setOpen(false);
    setHighlight(-1);
    setHighlightChosen(false);
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
          ref={setInputRef}
          // `type="text"` — WebKit's search input can clear itself on Enter;
          // we already hide the native clear control and own commit ourselves.
          // Spellcheck is off: on macOS the typo tip consumes ArrowDown
          // before this combobox sees it. `writingsuggestions` is set in the ref.
          type="text"
          spellCheck={false}
          autoCorrect="off"
          autoCapitalize="off"
          autoComplete="off"
          value={freeText}
          onChange={(e) => {
            onFreeTextChange(e.target.value);
            // Typing abandons the armed row so Enter searches the phrase.
            setHighlight(-1);
            setHighlightChosen(false);
            setOpen(true);
          }}
          onFocus={() => setOpen(true)}
          onKeyDown={handleKeyDown}
          placeholder={inputPlaceholder}
          className="min-w-[8rem] flex-1 bg-transparent py-1 text-[13px] outline-none placeholder:text-muted-foreground/80"
          aria-label="Search files and filters"
          aria-autocomplete="list"
          aria-controls={listboxId}
          aria-expanded={open && suggestions.length > 0}
          aria-activedescendant={highlight >= 0 ? `${listboxId}-option-${highlight}` : undefined}
          role="combobox"
        />

        {showClear && (
          <Button
            type="button"
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
            const label = suggestion.label;
            const hint =
              suggestion.kind === 'field'
                ? suggestion.aliases[0]
                : suggestion.field === 'tags'
                  ? 'Tag'
                  : suggestion.field === 'authors'
                    ? 'Author'
                    : suggestion.field === 'favorite'
                      ? 'Favorite'
                      : suggestion.field === 'local_cache'
                        ? 'Cache'
                        : suggestion.field === 'file_status'
                          ? 'Status'
                          : suggestion.field === 'storage_kind'
                            ? 'Storage'
                            : undefined;
            return (
              <li
                key={`${suggestion.kind}-${label}-${index}`}
                id={`${listboxId}-option-${index}`}
                role="option"
                aria-selected={selected}
              >
                <button
                  type="button"
                  className={cn(
                    'flex w-full items-center justify-between rounded-lg px-2.5 py-1.5 text-left text-sm',
                    selected ? 'bg-accent text-accent-foreground' : 'hover:bg-secondary/70'
                  )}
                  onMouseEnter={() => {
                    setHighlight(index);
                    setHighlightChosen(true);
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
