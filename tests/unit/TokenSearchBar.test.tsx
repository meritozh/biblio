import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { useState } from 'react';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { TokenSearchBar } from '@/components/TokenSearchBar';
import type { Condition } from '@/lib/filters';
import type { Author, Tag } from '@/types';

const tags: Tag[] = [
  { id: 1, name: 'sci-fi', color: null, created_at: '2026-01-01T00:00:00Z' },
];
const authors: Author[] = [
  { id: 10, name: '村上春树', created_at: '2026-01-01T00:00:00Z' },
];

function Harness({
  onFreeTextCommit,
  onConditionsChange,
}: {
  onFreeTextCommit: (q: string) => void;
  onConditionsChange?: (c: Condition[]) => void;
}) {
  const [freeText, setFreeText] = useState('');
  const [conditions, setConditions] = useState<Condition[]>([]);
  return (
    <TokenSearchBar
      conditions={conditions}
      onConditionsChange={(next) => {
        setConditions(next);
        onConditionsChange?.(next);
      }}
      freeText={freeText}
      onFreeTextChange={setFreeText}
      onFreeTextCommit={onFreeTextCommit}
      tags={tags}
      authors={authors}
      debounceMs={300}
    />
  );
}

describe('TokenSearchBar', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it('commits free text after debounce', () => {
    const onFreeTextCommit = vi.fn();
    render(<Harness onFreeTextCommit={onFreeTextCommit} />);
    const input = screen.getByRole('combobox');

    fireEvent.change(input, { target: { value: 'kafka' } });
    act(() => {
      vi.advanceTimersByTime(300);
    });
    expect(onFreeTextCommit).toHaveBeenLastCalledWith('kafka');
  });

  it('turns a clicked field suggestion + value into a condition chip', () => {
    const onFreeTextCommit = vi.fn();
    const onConditionsChange = vi.fn();
    render(
      <Harness onFreeTextCommit={onFreeTextCommit} onConditionsChange={onConditionsChange} />
    );
    const input = screen.getByRole('combobox');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'tag' } });
    // Explicit click (or Tab) accepts a field — Space/Enter alone must not.
    fireEvent.click(screen.getByRole('button', { name: /Tags/i }));
    fireEvent.click(screen.getByRole('button', { name: 'sci-fi' }));

    expect(onConditionsChange).toHaveBeenCalled();
    const last = onConditionsChange.mock.calls.at(-1)?.[0] as Condition[];
    expect(last).toHaveLength(1);
    expect(last[0]).toMatchObject({ field: 'tags', op: 'includes', tagId: 1 });
    expect(screen.getByText(/Tags includes sci-fi/i)).toBeInTheDocument();
  });

  it('keeps alias words as free-text phrases on Space and Enter', () => {
    const onFreeTextCommit = vi.fn();
    const onConditionsChange = vi.fn();
    render(
      <Harness onFreeTextCommit={onFreeTextCommit} onConditionsChange={onConditionsChange} />
    );
    const input = screen.getByRole('combobox');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'star' } });
    // Space must not tokenize `star` into Favorite — allow "star wars".
    fireEvent.keyDown(input, { key: ' ' });
    fireEvent.change(input, { target: { value: 'star ' } });
    fireEvent.change(input, { target: { value: 'star wars' } });
    fireEvent.keyDown(input, { key: 'Enter' });

    expect(onFreeTextCommit).toHaveBeenCalledWith('star wars');
    expect(onConditionsChange).not.toHaveBeenCalled();
  });

  it('accepts a value with Enter after arrow-selecting a field token', () => {
    const onFreeTextCommit = vi.fn();
    const onConditionsChange = vi.fn();
    render(
      <Harness onFreeTextCommit={onFreeTextCommit} onConditionsChange={onConditionsChange} />
    );
    const input = screen.getByRole('combobox');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'tag' } });
    // Explicit arrow navigation unlocks Enter for field acceptance.
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    fireEvent.keyDown(input, { key: 'Enter' });
    // Pending Tags chip; value list is open with the first tag highlighted.
    // Enter must accept that value without requiring a second ArrowDown —
    // listNavigated resets when suggestions swap after field selection.
    fireEvent.keyDown(input, { key: 'Enter' });

    expect(onConditionsChange).toHaveBeenCalled();
    const last = onConditionsChange.mock.calls.at(-1)?.[0] as Condition[];
    expect(last).toHaveLength(1);
    expect(last[0]).toMatchObject({ field: 'tags', op: 'includes', tagId: 1 });
    expect(screen.getByText(/Tags includes sci-fi/i)).toBeInTheDocument();
  });

  it('accepts a hovered field suggestion with Enter', () => {
    const onConditionsChange = vi.fn();
    render(
      <Harness onFreeTextCommit={vi.fn()} onConditionsChange={onConditionsChange} />
    );
    const input = screen.getByRole('combobox');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'fav' } });
    fireEvent.mouseEnter(screen.getByRole('button', { name: /Favorite/i }));
    fireEvent.keyDown(input, { key: 'Enter' });
    // Favorite value mode — two options; Enter accepts the highlighted one.
    fireEvent.keyDown(input, { key: 'Enter' });

    const last = onConditionsChange.mock.calls.at(-1)?.[0] as Condition[];
    expect(last).toHaveLength(1);
    expect(last[0]).toMatchObject({ field: 'favorite', op: 'is' });
  });

  it('cancels a pending field token with Backspace', () => {
    const onFreeTextCommit = vi.fn();
    render(<Harness onFreeTextCommit={onFreeTextCommit} />);
    const input = screen.getByRole('combobox');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'tag' } });
    fireEvent.click(screen.getByRole('button', { name: /Tags/i }));
    // Pending chip uses the field label; value suggestions are separate buttons.
    expect(screen.getByText('sci-fi')).toBeInTheDocument();

    fireEvent.keyDown(input, { key: 'Backspace' });
    expect(screen.queryByText('sci-fi')).not.toBeInTheDocument();
  });

  it('clears free text and conditions together', () => {
    const onFreeTextCommit = vi.fn();
    render(<Harness onFreeTextCommit={onFreeTextCommit} />);
    const input = screen.getByRole('combobox');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'tag' } });
    fireEvent.click(screen.getByRole('button', { name: /Tags/i }));
    fireEvent.click(screen.getByRole('button', { name: 'sci-fi' }));

    fireEvent.click(screen.getByRole('button', { name: /clear search/i }));
    expect(onFreeTextCommit).toHaveBeenCalledWith('');
    expect(screen.queryByText(/Tags includes sci-fi/i)).not.toBeInTheDocument();
  });
});
