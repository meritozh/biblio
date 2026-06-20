import { act, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FileList } from '@/components/FileList';
import { fileStore, hydrateFiles, patchFile } from '@/stores/fileStore';
import type { FileEntry } from '@/types';

const renderCounters = vi.hoisted(() => ({
  header: 0,
  content: 0,
}));

vi.mock('@/components/FileListHeader', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/components/FileListHeader')>();
  return {
    ...actual,
    FileListHeader: () => {
      renderCounters.header += 1;
      return <div data-testid="file-list-header" />;
    },
  };
});

vi.mock('@/components/FileListContent', () => ({
  FileListContent: () => {
    renderCounters.content += 1;
    return <div data-testid="file-list-content" />;
  },
}));

function makeFile(id: number): FileEntry {
  return {
    id,
    path: `/library/${id}.cbz`,
    display_name: `File ${id}`,
    category_id: 1,
    file_status: 'available',
    in_storage: true,
    original_path: null,
    progress: null,
    storage_kind: 'local',
    remote_provider: null,
    local_cache_path: null,
    is_favorite: false,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    category: null,
    tags: [],
    authors: [],
    metadata: [],
  };
}

describe('FileList render boundaries', () => {
  afterEach(() => {
    act(() => {
      fileStore.setState(() => ({
        byId: new Map(),
        views: new Map(),
        refreshEpoch: 0,
      }));
    });
    renderCounters.header = 0;
    renderCounters.content = 0;
  });

  it('does not rerender the header when a row patch only changes file contents', () => {
    act(() => {
      hydrateFiles([makeFile(1)]);
    });

    render(<FileList ids={[1]} />);
    const headerRendersAfterMount = renderCounters.header;

    act(() => {
      patchFile(1, { updated_at: '2026-01-02T00:00:00Z' });
    });

    expect(renderCounters.header).toBe(headerRendersAfterMount);
  });
});
