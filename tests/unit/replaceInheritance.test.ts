import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mergeReplaceParams } from '@/components/processing-pipeline/helpers';
import type { FileCreateRequest, FileWithDetails } from '@/types';

const mocks = vi.hoisted(() => ({
  coverGet: vi.fn(),
  fileGet: vi.fn(),
}));

vi.mock('@/lib/tauri', () => ({
  coverGet: mocks.coverGet,
  fileGet: mocks.fileGet,
}));

function existingFile(overrides: Partial<FileWithDetails> = {}): FileWithDetails {
  return {
    id: 7,
    path: '/library/story.epub',
    display_name: 'Existing Story',
    category_id: 1,
    file_status: 'available',
    in_storage: true,
    original_path: '/imports/story.epub',
    progress: '1',
    storage_kind: 'local',
    remote_provider: null,
    local_cache_path: null,
    is_favorite: true,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
    category: null,
    tags: [],
    authors: [],
    metadata: [],
    ...overrides,
  };
}

describe('replace import inheritance', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.coverGet.mockRejectedValue(new Error('no cover'));
  });

  it('inherits favorite state from the existing row', async () => {
    mocks.fileGet.mockResolvedValue(existingFile({ is_favorite: true }));
    const params: FileCreateRequest = {
      path: '/tmp/new-story.epub',
      display_name: 'New Story',
      category_id: 1,
    };

    const merged = await mergeReplaceParams(params, 7);

    expect(merged.is_favorite).toBe(true);
  });
});
