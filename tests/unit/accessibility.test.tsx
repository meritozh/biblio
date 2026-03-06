import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, fireEvent } from '@testing-library/react';
import { FileCard } from '@/components/FileCard';
import { SearchBar } from '@/components/SearchBar';
import { TagBadge } from '@/components/TagBadge';
import type { FileEntry, Tag } from '@/types';

vi.mock('@tauri-apps/api/core');

const mockFile: FileEntry = {
  id: 1,
  path: '/test/file.pdf',
  display_name: 'Test File',
  category_id: 1,
  file_status: 'available',
  created_at: '2024-01-01T00:00:00Z',
  updated_at: '2024-01-01T00:00:00Z',
};

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

  describe('FileCard Accessibility', () => {
    it('should have accessible status indicator', () => {
      const { container } = render(<FileCard file={mockFile} />);
      const statusIndicator = container.querySelector('[role="status"]');
      expect(statusIndicator).toBeTruthy();
      expect(statusIndicator?.getAttribute('aria-label')).toBe('File available');
    });

    it('should have accessible action buttons', () => {
      const { container } = render(
        <FileCard file={mockFile} onEdit={() => {}} onDelete={() => {}} />
      );
      const actionGroup = container.querySelector('[role="group"]');
      expect(actionGroup?.getAttribute('aria-label')).toBe('File actions');
    });

    it('should have aria-labels for edit and delete buttons', () => {
      const { getByLabelText } = render(
        <FileCard file={mockFile} onEdit={() => {}} onDelete={() => {}} />
      );
      expect(getByLabelText('Edit Test File')).toBeTruthy();
      expect(getByLabelText('Delete Test File')).toBeTruthy();
    });

    it('should have aria-hidden on decorative icons', () => {
      const { container } = render(<FileCard file={mockFile} />);
      const decorativeIcons = container.querySelectorAll('[aria-hidden="true"]');
      expect(decorativeIcons.length).toBeGreaterThan(0);
    });

    it('should call onEdit when edit button is clicked', () => {
      const onEdit = vi.fn();
      const { getByLabelText } = render(
        <FileCard file={mockFile} onEdit={onEdit} onDelete={() => {}} />
      );
      fireEvent.click(getByLabelText('Edit Test File'));
      expect(onEdit).toHaveBeenCalledWith(mockFile);
    });

    it('should call onDelete when delete button is clicked', () => {
      const onDelete = vi.fn();
      const { getByLabelText } = render(
        <FileCard file={mockFile} onEdit={() => {}} onDelete={onDelete} />
      );
      fireEvent.click(getByLabelText('Delete Test File'));
      expect(onDelete).toHaveBeenCalledWith(mockFile);
    });

    it('should display tags when file has tags', () => {
      const fileWithTags: FileEntry = {
        ...mockFile,
        tags: [mockTag],
      };
      const { getByText } = render(<FileCard file={fileWithTags} />);
      expect(getByText('favorite')).toBeTruthy();
    });

    it('should show correct status for missing file', () => {
      const missingFile: FileEntry = {
        ...mockFile,
        file_status: 'missing',
      };
      const { container } = render(<FileCard file={missingFile} />);
      const statusIndicator = container.querySelector('[role="status"]');
      expect(statusIndicator?.getAttribute('aria-label')).toBe('File not found');
    });

    it('should show correct status for moved file', () => {
      const movedFile: FileEntry = {
        ...mockFile,
        file_status: 'moved',
      };
      const { container } = render(<FileCard file={movedFile} />);
      const statusIndicator = container.querySelector('[role="status"]');
      expect(statusIndicator?.getAttribute('aria-label')).toBe('File has been moved');
    });
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

    it('should apply focus ring when input is focused', () => {
      const { getByLabelText, container } = render(
        <SearchBar value="" onChange={() => {}} onSearch={() => {}} />
      );
      const input = getByLabelText('Search files');
      fireEvent.focus(input);
      const wrapper = container.querySelector('.ring-2');
      expect(wrapper).toBeTruthy();
    });

    it('should remove focus ring when input is blurred', () => {
      const { getByLabelText, container } = render(
        <SearchBar value="" onChange={() => {}} onSearch={() => {}} />
      );
      const input = getByLabelText('Search files');
      fireEvent.focus(input);
      fireEvent.blur(input);
      const wrapper = container.querySelector('.ring-2');
      expect(wrapper).toBeNull();
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