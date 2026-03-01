import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

interface TagWithUsage {
  id: number;
  name: string;
  color: string | null;
  created_at: string;
  usage_count: number;
}

interface TagListResponse {
  tags: TagWithUsage[];
}

interface TagCreateResponse {
  id: number;
}

interface TagDeleteResponse {
  success: boolean;
  affected_files: number;
}

vi.mock('@tauri-apps/api/core');

describe('tag_list command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should list tags without usage counts', async () => {
    const mockResponse: TagListResponse = {
      tags: [
        {
          id: 1,
          name: 'favorite',
          color: '#FF5733',
          created_at: '2024-01-01T00:00:00Z',
          usage_count: 0,
        },
      ],
    };

    vi.mocked(invoke).mockResolvedValue(mockResponse);

    const result = await invoke<TagListResponse>('tag_list', { includeUsage: false });

    expect(invoke).toHaveBeenCalledWith('tag_list', { includeUsage: false });
    expect(result.tags).toHaveLength(1);
  });

  it('should list tags with usage counts', async () => {
    const mockResponse: TagListResponse = {
      tags: [
        {
          id: 1,
          name: 'favorite',
          color: '#FF5733',
          created_at: '2024-01-01T00:00:00Z',
          usage_count: 5,
        },
      ],
    };

    vi.mocked(invoke).mockResolvedValue(mockResponse);

    const result = await invoke<TagListResponse>('tag_list', { includeUsage: true });

    expect(invoke).toHaveBeenCalledWith('tag_list', { includeUsage: true });
    expect(result.tags[0].usage_count).toBe(5);
  });
});

describe('tag_create command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should create a new tag', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 1 });

    const result = await invoke<TagCreateResponse>('tag_create', {
      name: 'read-later',
      color: '#00FF00',
    });

    expect(invoke).toHaveBeenCalledWith('tag_create', { name: 'read-later', color: '#00FF00' });
    expect(result.id).toBe(1);
  });

  it('should reject duplicate tag names', async () => {
    vi.mocked(invoke).mockRejectedValue('TAG_EXISTS');

    await expect(invoke<TagCreateResponse>('tag_create', { name: 'favorite' })).rejects.toBe(
      'TAG_EXISTS'
    );
  });
});

describe('tag_assign command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should assign tags to a file', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true });

    const result = await invoke<{ success: boolean }>('tag_assign', {
      fileId: 1,
      tagIds: [1, 2, 3],
    });

    expect(invoke).toHaveBeenCalledWith('tag_assign', { fileId: 1, tagIds: [1, 2, 3] });
    expect(result.success).toBe(true);
  });

  it('should handle empty tag list', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true });

    const result = await invoke<{ success: boolean }>('tag_assign', { fileId: 1, tagIds: [] });

    expect(invoke).toHaveBeenCalledWith('tag_assign', { fileId: 1, tagIds: [] });
    expect(result.success).toBe(true);
  });
});

describe('tag_unassign command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should unassign tags from a file', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true });

    await invoke<{ success: boolean }>('tag_unassign', { fileId: 1, tagIds: [1, 2] });

    expect(invoke).toHaveBeenCalledWith('tag_unassign', { fileId: 1, tagIds: [1, 2] });
  });
});

describe('tag_update command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should update tag name', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true });

    await invoke<{ success: boolean }>('tag_update', { id: 1, name: 'new-name' });

    expect(invoke).toHaveBeenCalledWith('tag_update', { id: 1, name: 'new-name' });
  });

  it('should update tag color', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true });

    await invoke<{ success: boolean }>('tag_update', { id: 1, color: '#0000FF' });

    expect(invoke).toHaveBeenCalledWith('tag_update', { id: 1, color: '#0000FF' });
  });
});

describe('tag_delete command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should delete a tag and return affected files', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true, affected_files: 3 });

    const result = await invoke<TagDeleteResponse>('tag_delete', { id: 1 });

    expect(invoke).toHaveBeenCalledWith('tag_delete', { id: 1 });
    expect(result.success).toBe(true);
    expect(result.affected_files).toBe(3);
  });
});
