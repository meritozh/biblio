import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ProcessingPipeline } from '@/components/ProcessingPipeline';
import type { Category, FilePreparedImport } from '@/types';

const mocks = vi.hoisted(() => ({
  authorList: vi.fn(),
  coverGet: vi.fn(),
  fileCreate: vi.fn(),
  fileGet: vi.fn(),
  fileReplace: vi.fn(),
  fileDeleteSource: vi.fn(),
  cancelProcessing: vi.fn(),
  preparedCoverClear: vi.fn(),
  listenProcessingProgress: vi.fn(),
  listenFilePrepared: vi.fn(),
  importFinalize: vi.fn(),
  tagList: vi.fn(),
  vndbSearch: vi.fn(),
  vndbFetchCover: vi.fn(),
  preparedHandler: null as null | ((result: FilePreparedImport) => void),
}));

vi.mock('@/lib/tauri', () => ({
  authorList: mocks.authorList,
  coverGet: mocks.coverGet,
  fileCreate: mocks.fileCreate,
  fileGet: mocks.fileGet,
  fileReplace: mocks.fileReplace,
  fileDeleteSource: mocks.fileDeleteSource,
  cancelProcessing: mocks.cancelProcessing,
  preparedCoverClear: mocks.preparedCoverClear,
  listenProcessingProgress: mocks.listenProcessingProgress,
  listenFilePrepared: mocks.listenFilePrepared,
  importFinalize: mocks.importFinalize,
  tagList: mocks.tagList,
  vndbSearch: mocks.vndbSearch,
  vndbFetchCover: mocks.vndbFetchCover,
}));

vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count, getItemKey }: { count: number; getItemKey: (index: number) => string }) => ({
    getTotalSize: () => count * 160,
    getVirtualItems: () =>
      Array.from({ length: count }, (_, index) => ({
        index,
        key: getItemKey(index),
        start: index * 160,
      })),
    measureElement: vi.fn(),
  }),
}));

vi.mock('@/components/DynamicMetadataForm', () => ({
  DynamicMetadataForm: () => <div data-testid="metadata-form" />,
}));

const categories: Category[] = [
  {
    id: 1,
    name: 'Novel',
    description: null,
    icon: null,
    is_default: true,
    folder_name: 'novel',
    schema_slug: 'novel',
    view_config: null,
    created_at: '2026-01-01T00:00:00Z',
  },
];

function preparedDuplicate(path: string): FilePreparedImport {
  return {
    path,
    file_name: 'new.epub',
    display_name: 'Story',
    category_id: 1,
    tag_ids: [],
    author_ids: [],
    metadata: [],
    unresolved_author_names: [],
    progress: '2',
    suggested_tags: [],
    cover_mime_type: undefined,
    duplicate_of: {
      existing_file_id: 7,
      existing_display_name: 'Story',
      existing_progress: '1',
      existing_size: 100,
      new_size: 200,
      existing_author_names: [],
      recommendation: 'Replace',
    },
    batch_duplicate_group: null,
    source_is_directory: false,
  };
}

function renderPipeline(path = '/tmp/new.epub') {
  return render(
    <ProcessingPipeline
      open
      onOpenChange={vi.fn()}
      minimized={false}
      onMinimize={vi.fn()}
      onExpand={vi.fn()}
      paths={[path]}
      categories={categories}
      tags={[]}
      authors={[]}
      onTagCreate={vi.fn()}
      onAuthorCreate={vi.fn()}
      onImportComplete={vi.fn()}
    />
  );
}

describe('ProcessingPipeline duplicate actions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.preparedHandler = null;
    mocks.fileCreate.mockResolvedValue({ id: 10 });
    mocks.fileReplace.mockResolvedValue({ id: 11 });
    mocks.fileDeleteSource.mockResolvedValue(undefined);
    mocks.cancelProcessing.mockResolvedValue(undefined);
    mocks.preparedCoverClear.mockResolvedValue(undefined);
    mocks.listenProcessingProgress.mockResolvedValue(vi.fn());
    mocks.listenFilePrepared.mockImplementation(async (handler) => {
      mocks.preparedHandler = handler;
      return vi.fn();
    });
  });

  it('requires an explicit duplicate action before committing import-anyway', async () => {
    const path = '/tmp/new.epub';
    renderPipeline(path);

    await waitFor(() => expect(mocks.preparedHandler).toBeTruthy());
    await act(async () => {
      mocks.preparedHandler?.(preparedDuplicate(path));
    });

    await screen.findAllByText(/Duplicate of/);
    const replaceRadio = await screen.findByRole('radio', { name: /replace existing/i });
    const deleteRadio = screen.getByRole('radio', { name: /^delete/i });
    const importAnywayRadio = screen.getByRole('radio', { name: /import anyway/i });

    expect(replaceRadio).not.toBeChecked();
    expect(deleteRadio).not.toBeChecked();
    expect(importAnywayRadio).not.toBeChecked();
    expect(
      screen.getByRole('button', { name: /choose 1 duplicate action/i })
    ).toBeDisabled();

    fireEvent.click(importAnywayRadio);

    expect(importAnywayRadio).toBeChecked();
    fireEvent.click(screen.getByRole('button', { name: /import 1/i }));

    await waitFor(() => expect(mocks.fileCreate).toHaveBeenCalledTimes(1));
    expect(mocks.fileReplace).not.toHaveBeenCalled();
    expect(mocks.fileDeleteSource).not.toHaveBeenCalled();
  });
});
