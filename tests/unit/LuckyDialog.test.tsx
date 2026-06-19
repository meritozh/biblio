import { afterEach, describe, expect, it } from 'vitest';
import { act, render, screen } from '@testing-library/react';
import { LuckyDialog } from '@/components/LuckyDialog';
import { LUCKY_DIALOG_WIDTH } from '@/components/LuckyFileCards';
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
  };
}

const files = [
  makeFile(1, 'Book One'),
  makeFile(2, 'Book Two'),
  makeFile(3, 'Book Three'),
];

describe('LuckyDialog', () => {
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
  });

  it('keeps existing cards mounted while a shuffle refresh is in flight', () => {
    act(() => {
      appStore.setState(() => ({
        categories: [novelCategory],
        selectedCategoryId: novelCategory.id,
        settingsOpen: false,
      }));
      hydrateFiles(files);
    });

    render(
      <LuckyDialog
        open
        onOpenChange={() => {}}
        files={files}
        loading={false}
        refreshing
        error={null}
        canShuffle
        onShuffle={() => {}}
        onFileClick={() => {}}
      />
    );

    expect(screen.queryByText('Picking...')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'View Book One' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'View Book Two' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'View Book Three' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /shuffle again/i })).toBeDisabled();
  });

  it('uses the three-card dialog width contract', () => {
    render(
      <LuckyDialog
        open
        onOpenChange={() => {}}
        files={[]}
        loading
        refreshing={false}
        error={null}
        canShuffle
        onShuffle={() => {}}
        onFileClick={() => {}}
      />
    );

    expect(screen.getByRole('dialog', { name: 'Lucky' })).toHaveStyle({
      width: `min(calc(100vw - 2rem), ${LUCKY_DIALOG_WIDTH}px)`,
    });
  });
});
