import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import {
  LUCKY_DIALOG_WIDTH,
  LuckyFileCards,
} from '@/components/LuckyFileCards';
import { CARD_WIDTH } from '@/components/cards/constants';
import { appStore } from '@/stores/appStore';
import { fileStore, hydrateFiles } from '@/stores/fileStore';
import type { Category, FileEntry } from '@/types';

const novelCategory: Category = {
  id: 7,
  name: 'novel',
  description: null,
  icon: null,
  is_default: true,
  folder_name: null,
  schema_slug: 'novel',
  view_config: null,
  created_at: '2026-01-01T00:00:00Z',
};

function makeFile(id: number, displayName: string): FileEntry {
  return {
    id,
    path: `/library/${displayName}.epub`,
    display_name: displayName,
    category_id: novelCategory.id,
    file_status: 'available',
    in_storage: true,
    original_path: null,
    progress: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    category: novelCategory,
    tags: [],
    authors: [],
    metadata: [],
    storage_kind: 'local',
    remote_provider: null,
    local_cache_path: null,
    is_favorite: false,
  };
}

describe('LuckyFileCards', () => {
  afterEach(() => {
    act(() => {
      appStore.setState(() => ({
        categories: [],
        selectedCategoryId: null,
        settingsOpen: false,
      }));
      fileStore.setState(() => ({
        byId: new Map(),
        views: new Map(),
        refreshEpoch: 0,
      }));
    });
    vi.restoreAllMocks();
  });

  it('renders lucky picks as three horizontal file cards and opens the clicked file', () => {
    const files = [
      makeFile(1, 'Book One'),
      makeFile(2, 'Book Two'),
      makeFile(3, 'Book Three'),
    ];
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const onFileClick = vi.fn();
    act(() => {
      appStore.setState(() => ({
        categories: [novelCategory],
        selectedCategoryId: novelCategory.id,
        settingsOpen: false,
      }));
      hydrateFiles(files);
    });

    render(<LuckyFileCards files={files} onFileClick={onFileClick} />);

    const strip = screen.getByTestId('lucky-file-cards');
    expect(strip).toHaveClass('flex', 'flex-row', 'gap-4');
    expect(strip).toHaveStyle({ minHeight: '280px' });
    expect(screen.getByRole('button', { name: 'View Book One' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'View Book Two' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'View Book Three' })).toBeInTheDocument();
    expect(
      within(screen.getByRole('button', { name: 'View Book One' })).queryByRole('button', {
        name: /favorites/i,
      })
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'View Book Two' }));

    expect(onFileClick).toHaveBeenCalledWith(files[1]);
    const errors = consoleError.mock.calls.flat().join('\n');
    expect(errors).not.toContain('cannot be a descendant of <button>');
  });

  it('keeps the dialog width to exactly three file cards plus normal spacing', () => {
    expect(LUCKY_DIALOG_WIDTH).toBe(CARD_WIDTH * 3 + 16 * 2 + 24 * 2);
  });

  it('keeps card actions out of the card button markup', () => {
    const files = [makeFile(1, 'Book One')];
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    act(() => {
      appStore.setState(() => ({
        categories: [novelCategory],
        selectedCategoryId: novelCategory.id,
        settingsOpen: false,
      }));
      hydrateFiles(files);
    });

    render(<LuckyFileCards files={files} onFileClick={() => {}} />);

    const errors = consoleError.mock.calls.flat().join('\n');
    expect(errors).not.toContain('cannot be a descendant of <button>');
  });
});
