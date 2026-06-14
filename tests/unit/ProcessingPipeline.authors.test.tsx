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

function preparedWithAuthorHint(path: string, fileName: string, author: string): FilePreparedImport {
  return {
    path,
    file_name: fileName,
    display_name: fileName,
    category_id: 1,
    tag_ids: [],
    author_ids: [],
    metadata: [],
    unresolved_author_names: [author],
    progress: '',
    suggested_tags: [],
    cover_mime_type: undefined,
    duplicate_of: null,
    batch_duplicate_group: null,
    source_is_directory: false,
  };
}

function renderPipeline(paths: string[], onAuthorCreate = vi.fn()) {
  return render(
    <ProcessingPipeline
      open
      onOpenChange={vi.fn()}
      minimized={false}
      onMinimize={vi.fn()}
      onExpand={vi.fn()}
      paths={paths}
      categories={categories}
      tags={[]}
      authors={[]}
      onTagCreate={vi.fn()}
      onAuthorCreate={onAuthorCreate}
      onImportComplete={vi.fn()}
    />
  );
}

describe('ProcessingPipeline suggested authors', () => {
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

  it('applies one approved author hint to every matching file in the import session', async () => {
    const paths = ['/tmp/first.epub', '/tmp/second.epub'];
    const onAuthorCreate = vi.fn().mockResolvedValue({
      id: 42,
      name: 'Shared Author',
      created_at: '2026-01-01T00:00:00Z',
    });
    renderPipeline(paths, onAuthorCreate);

    await waitFor(() => expect(mocks.preparedHandler).toBeTruthy());
    await act(async () => {
      mocks.preparedHandler?.(
        preparedWithAuthorHint(paths[0]!, 'first.epub', 'Shared Author')
      );
      mocks.preparedHandler?.(
        preparedWithAuthorHint(paths[1]!, 'second.epub', '  shared author  ')
      );
    });

    fireEvent.click(await screen.findByRole('tab', { name: /ready/i }));
    fireEvent.click(await screen.findByText('first.epub'));
    fireEvent.click(
      await screen.findByTitle('Add "Shared Author" as a new author')
    );

    await waitFor(() => expect(onAuthorCreate).toHaveBeenCalledTimes(1));
    await waitFor(() =>
      expect(
        screen.queryByTitle('Add "Shared Author" as a new author')
      ).not.toBeInTheDocument()
    );

    fireEvent.click(screen.getByRole('button', { name: /import 2/i }));

    await waitFor(() => expect(mocks.fileCreate).toHaveBeenCalledTimes(2));
    expect(mocks.fileCreate).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({ path: paths[0], author_ids: [42] })
    );
    expect(mocks.fileCreate).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ path: paths[1], author_ids: [42] })
    );
  });
});
