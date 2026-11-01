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
  unstableLists = false,
}: {
  onFreeTextCommit: (q: string) => void;
  onConditionsChange?: (c: Condition[]) => void;
  unstableLists?: boolean;
}) {
  const [freeText, setFreeText] = useState('');
  const [conditions, setConditions] = useState<Condition[]>([]);
  // Simulate parent re-renders that pass new tags/authors array identities
  // (e.g. after a debounced search fetch). Should not kill arrow navigation.
  const tagProp = unstableLists ? tags.map((t) => ({ ...t })) : tags;
  const authorProp = unstableLists ? authors.map((a) => ({ ...a })) : authors;
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
      tags={tagProp}
      authors={authorProp}
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
    fireEvent.keyDown(input, { key: ' ' });
    fireEvent.change(input, { target: { value: 'star ' } });
    fireEvent.change(input, { target: { value: 'star wars' } });
    fireEvent.keyDown(input, { key: 'Enter' });

    expect(onFreeTextCommit).toHaveBeenCalledWith('star wars');
    expect(onConditionsChange).not.toHaveBeenCalled();
    // Plain Enter must not wipe the typed phrase.
    expect((input as HTMLInputElement).value).toBe('star wars');
  });

  it('selects the arrow-highlighted field with Enter and keeps a pending chip', () => {
    const onFreeTextCommit = vi.fn();
    const onConditionsChange = vi.fn();
    render(
      <Harness onFreeTextCommit={onFreeTextCommit} onConditionsChange={onConditionsChange} />
    );
    const input = screen.getByRole('combobox');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'tag' } });
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    fireEvent.keyDown(input, { key: 'Enter' });

    // Field accepted → pending Tags chip; free text cleared for value entry.
    expect(screen.getByText('Tags')).toBeInTheDocument();
    expect((input as HTMLInputElement).value).toBe('');
    expect(onConditionsChange).not.toHaveBeenCalled();
  });

  it('keeps arrow highlight across parent re-renders then accepts with Enter', () => {
    const onFreeTextCommit = vi.fn();
    const onConditionsChange = vi.fn();
    const { rerender } = render(
      <Harness
        onFreeTextCommit={onFreeTextCommit}
        onConditionsChange={onConditionsChange}
        unstableLists
      />
    );
    const input = screen.getByRole('combobox');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'tag' } });
    fireEvent.keyDown(input, { key: 'ArrowDown' });

    // Debounced search / store updates re-render the parent with new array
    // identities — highlight must survive so Enter still accepts.
    rerender(
      <Harness
        onFreeTextCommit={onFreeTextCommit}
        onConditionsChange={onConditionsChange}
        unstableLists
      />
    );

    fireEvent.keyDown(input, { key: 'Enter' });
    expect(screen.getByText('Tags')).toBeInTheDocument();

    fireEvent.keyDown(input, { key: 'Enter' });
    const last = onConditionsChange.mock.calls.at(-1)?.[0] as Condition[];
    expect(last).toHaveLength(1);
    expect(last[0]).toMatchObject({ field: 'tags', op: 'includes', tagId: 1 });
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
    fireEvent.keyDown(input, { key: 'ArrowDown' });
    fireEvent.keyDown(input, { key: 'Enter' });
    // Value list open; Arrow then Enter picks sci-fi (or sole suggestion fallback).
    fireEvent.keyDown(input, { key: 'ArrowDown' });
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
    fireEvent.keyDown(input, { key: 'ArrowDown' });
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
