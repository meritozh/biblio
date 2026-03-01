import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, configure } from '@testing-library/react';
import { FileCard } from '@/components/FileCard';
import { SearchBar } from '@/components/SearchBar';
import { TagBadge } from '@/components/TagBadge';
import type { FileEntry, Tag } from '@/types';

vi.mock('@tauri-apps/api/core');

configure({ testIdAttribute: 'id' });

const mockFile: FileEntry = {
  id: 1,
  path: '/test/file.pdf',
  displayName: 'Test File',
  categoryId: 1,
  fileStatus: 'available',
  createdAt: '2024-01-01T00:00:00Z',
  updatedAt: '2024-01-01T00:00:00Z',
};

const mockTag: Tag = {
  id: 1,
  name: 'favorite',
  color: '#FF5733',
  createdAt: '2024-01-01T00:00:00Z',
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
  });
});
