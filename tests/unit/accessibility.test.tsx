import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/react';
import { SearchBar } from '@/components/SearchBar';
import { TagBadge } from '@/components/TagBadge';
import type { Tag } from '@/types';

// NOTE: the former flat `FileCard` component was refactored into the
// schema-based `components/cards/` renderers; its accessibility cases were
// removed here rather than repointed (the new cards have a different API and
// would need fresh tests). SearchBar and TagBadge cases remain.

vi.mock('@tauri-apps/api/core');

const mockTag: Tag = {
  id: 1,
  name: 'favorite',
  color: '#FF5733',
  created_at: '2024-01-01T00:00:00Z',
};

describe('Accessibility Tests', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('SearchBar Accessibility', () => {
    it('should have accessible search input', () => {
      const { getByLabelText } = render(
        <SearchBar value="" onChange={() => {}} onSearch={() => {}} />
      );
      expect(getByLabelText('Search files')).toBeTruthy();
    });

    it('should use search input type', () => {
      const { getByLabelText } = render(
        <SearchBar value="" onChange={() => {}} onSearch={() => {}} />
      );
      const input = getByLabelText('Search files');
      expect(input.getAttribute('type')).toBe('search');
    });

    it('should have accessible clear button', () => {
      const { getByLabelText } = render(
        <SearchBar value="test" onChange={() => {}} onSearch={() => {}} />
      );
      expect(getByLabelText('Clear search')).toBeTruthy();
    });

    it('should call onSearch when Enter is pressed', () => {
      const onSearch = vi.fn();
      const { getByLabelText } = render(
        <SearchBar value="test query" onChange={() => {}} onSearch={onSearch} />
      );
      fireEvent.keyDown(getByLabelText('Search files'), { key: 'Enter' });
      expect(onSearch).toHaveBeenCalledWith('test query');
    });

    it('should call onChange and onSearch when clear button is clicked', () => {
      const onChange = vi.fn();
      const onSearch = vi.fn();
      const { getByLabelText } = render(
        <SearchBar value="test" onChange={onChange} onSearch={onSearch} />
      );
      fireEvent.click(getByLabelText('Clear search'));
      expect(onChange).toHaveBeenCalledWith('');
      expect(onSearch).toHaveBeenCalledWith('');
    });

    it('should call onChange when input value changes', () => {
      const onChange = vi.fn();
      const { getByLabelText } = render(
        <SearchBar value="" onChange={onChange} onSearch={() => {}} />
      );
      fireEvent.change(getByLabelText('Search files'), { target: { value: 'new value' } });
      expect(onChange).toHaveBeenCalledWith('new value');
    });

    it('should not show clear button when value is empty', () => {
      const { queryByLabelText } = render(
        <SearchBar value="" onChange={() => {}} onSearch={() => {}} />
      );
      expect(queryByLabelText('Clear search')).toBeNull();
    });

    it('should focus the input on focus event', () => {
      // The focus ring lives on the Input primitive itself via the
      // `focus-visible:ring-2` modifier — there is no `.ring-2` class on
      // the wrapper (an earlier wrapper-level ring caused a double outline
      // and was removed; see the comment in SearchBar.tsx). The semantic
      // assertion is just "focus reaches the input".
      const { getByLabelText } = render(
        <SearchBar value="" onChange={() => {}} onSearch={() => {}} />
      );
      const input = getByLabelText('Search files');
      input.focus();
      expect(document.activeElement).toBe(input);
    });

    it('should blur the input on blur event', () => {
      const { getByLabelText } = render(
        <SearchBar value="" onChange={() => {}} onSearch={() => {}} />
      );
      const input = getByLabelText('Search files') as HTMLInputElement;
      input.focus();
      input.blur();
      expect(document.activeElement).not.toBe(input);
    });
  });

  describe('TagBadge Accessibility', () => {
    it('should have accessible remove button', () => {
      const { getByLabelText } = render(<TagBadge tag={mockTag} onRemove={() => {}} />);
      expect(getByLabelText('Remove tag favorite')).toBeTruthy();
    });

    it('should be keyboard accessible when clickable', () => {
      const { getByRole } = render(<TagBadge tag={mockTag} clickable onClick={() => {}} />);
      const badge = getByRole('button');
      expect(badge).toBeTruthy();
      expect(badge.getAttribute('tabIndex')).toBe('0');
    });

    it('should call onClick when clicked and clickable', () => {
      const onClick = vi.fn();
      const { getByRole } = render(<TagBadge tag={mockTag} clickable onClick={onClick} />);
      fireEvent.click(getByRole('button'));
      expect(onClick).toHaveBeenCalledWith(mockTag);
    });

    it('should call onClick when Enter is pressed on clickable badge', () => {
      const onClick = vi.fn();
      const { getByRole } = render(<TagBadge tag={mockTag} clickable onClick={onClick} />);
      fireEvent.keyDown(getByRole('button'), { key: 'Enter' });
      expect(onClick).toHaveBeenCalledWith(mockTag);
    });

    it('should call onRemove when remove button is clicked', () => {
      const onRemove = vi.fn();
      const { getByLabelText } = render(<TagBadge tag={mockTag} onRemove={onRemove} />);
      fireEvent.click(getByLabelText('Remove tag favorite'));
      expect(onRemove).toHaveBeenCalledWith(mockTag);
    });

    it('should display tag name', () => {
      const { getByText } = render(<TagBadge tag={mockTag} />);
      expect(getByText('favorite')).toBeTruthy();
    });

    it('should apply custom color when provided', () => {
      const { container } = render(<TagBadge tag={mockTag} />);
      const badge = container.querySelector('[style*="background-color"]');
      expect(badge).toBeTruthy();
    });
  });
});