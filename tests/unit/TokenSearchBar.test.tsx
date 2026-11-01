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

  it('turns a field keyword + value into a condition chip', () => {
    const onFreeTextCommit = vi.fn();
    const onConditionsChange = vi.fn();
    render(
      <Harness onFreeTextCommit={onFreeTextCommit} onConditionsChange={onConditionsChange} />
    );
    const input = screen.getByRole('combobox');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'tag' } });
    // Exact keyword + Enter enters value mode for that field.
    fireEvent.keyDown(input, { key: 'Enter' });

    fireEvent.click(screen.getByRole('button', { name: 'sci-fi' }));

    expect(onConditionsChange).toHaveBeenCalled();
    const last = onConditionsChange.mock.calls.at(-1)?.[0] as Condition[];
    expect(last).toHaveLength(1);
    expect(last[0]).toMatchObject({ field: 'tags', op: 'includes', tagId: 1 });
    expect(screen.getByText(/Tags includes sci-fi/i)).toBeInTheDocument();
  });

  it('clears free text and conditions together', () => {
    const onFreeTextCommit = vi.fn();
    render(<Harness onFreeTextCommit={onFreeTextCommit} />);
    const input = screen.getByRole('combobox');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'tag' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    fireEvent.click(screen.getByRole('button', { name: 'sci-fi' }));

    fireEvent.click(screen.getByRole('button', { name: /clear search/i }));
    expect(onFreeTextCommit).toHaveBeenCalledWith('');
    expect(screen.queryByText(/Tags includes sci-fi/i)).not.toBeInTheDocument();
  });
});
