import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { useState } from 'react';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { SearchBar } from '@/components/SearchBar';

/**
 * Controlled wrapper that mirrors how the Library route wires SearchBar:
 * `value` is owned here, `onChange` updates it, and `onSearch` is the commit
 * callback (the route uses it to set `debouncedQuery`, which drives fetches).
 */
function Harness({
  onSearch,
  initialValue = '',
}: {
  onSearch: (q: string) => void;
  initialValue?: string;
}) {
  const [value, setValue] = useState(initialValue);
  return (
    <SearchBar value={value} onChange={setValue} onSearch={onSearch} debounceMs={300} />
  );
}

describe('SearchBar clearing parity', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it('commits onSearch("") when the input is deleted back to empty, matching the clear button', () => {
    const onSearch = vi.fn();
    render(<Harness onSearch={onSearch} />);
    const input = screen.getByRole('searchbox');

    // Type a query and let the debounce commit it.
    fireEvent.change(input, { target: { value: 'foo' } });
    act(() => {
      vi.advanceTimersByTime(300);
    });
    expect(onSearch).toHaveBeenLastCalledWith('foo');

    // Delete everything via the keyboard (input value -> ''). This is the
    // path that regressed: the commit must still fire even though we are
    // returning to the empty string the component mounted with.
    onSearch.mockClear();
    fireEvent.change(input, { target: { value: '' } });
    act(() => {
      vi.advanceTimersByTime(300);
    });
    expect(onSearch).toHaveBeenCalledWith('');
  });

  it('commits onSearch("") immediately when the clear button is clicked', () => {
    const onSearch = vi.fn();
    render(<Harness onSearch={onSearch} />);
    const input = screen.getByRole('searchbox');

    fireEvent.change(input, { target: { value: 'foo' } });
    act(() => {
      vi.advanceTimersByTime(300);
    });

    onSearch.mockClear();
    fireEvent.click(screen.getByRole('button', { name: /clear search/i }));
    // Clear button bypasses the debounce — fires synchronously.
    expect(onSearch).toHaveBeenCalledWith('');
  });

  it('does not commit on mount with a pre-filled value', () => {
    const onSearch = vi.fn();
    render(<Harness onSearch={onSearch} initialValue="preset" />);
    act(() => {
      vi.advanceTimersByTime(300);
    });
    expect(onSearch).not.toHaveBeenCalled();
  });
});
