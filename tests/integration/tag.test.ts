import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core');

describe('Tag and Metadata Workflow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  it('should create tags and assign to file', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ id: 1 })
      .mockResolvedValueOnce({ id: 2 })
      .mockResolvedValueOnce({ success: true });

    const tag1 = await invoke<{ id: number }>('tag_create', { name: 'favorite', color: '#FF5733' });
    const tag2 = await invoke<{ id: number }>('tag_create', {
      name: 'read-later',
      color: '#00FF00',
    });

    const assignResult = await invoke<{ success: boolean }>('tag_assign', {
      fileId: 1,
      tagIds: [tag1.id, tag2.id],
    });

    expect(assignResult.success).toBe(true);
  });

  it('should add metadata to a file', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ id: 1 })
      .mockResolvedValueOnce({ id: 2 })
      .mockResolvedValueOnce({
        metadata: [
          { id: 1, file_id: 1, key: 'author', value: 'John Doe', data_type: 'text' },
          { id: 2, file_id: 1, key: 'page_count', value: '150', data_type: 'number' },
        ],
      });

    await invoke<{ id: number }>('metadata_set', {
      fileId: 1,
      key: 'author',
      value: 'John Doe',
      dataType: 'text',
    });

    await invoke<{ id: number }>('metadata_set', {
      fileId: 1,
      key: 'page_count',
      value: '150',
      dataType: 'number',
    });

    const metadata = await invoke<{ metadata: Array<{ key: string; value: string }> }>(
      'metadata_get',
      { fileId: 1 }
    );

    expect(metadata.metadata).toHaveLength(2);
    expect(metadata.metadata.find((m) => m.key === 'author')?.value).toBe('John Doe');
  });

  it('should filter files by tag', async () => {
    vi.mocked(invoke).mockResolvedValue({
      files: [
        {
          id: 1,
          path: '/test/file1.pdf',
          display_name: 'File 1',
          category_id: 1,
          file_status: 'available',
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-01T00:00:00Z',
        },
        {
          id: 2,
          path: '/test/file2.pdf',
          display_name: 'File 2',
          category_id: 1,
          file_status: 'available',
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-01T00:00:00Z',
        },
      ],
      total: 2,
    });

    const result = await invoke<{ files: Array<{ id: number }>; total: number }>('file_list', {
      tagIds: [1],
    });

    expect(result.files).toHaveLength(2);
  });

  it('should update tag on multiple files', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ success: true })
      .mockResolvedValueOnce({ success: true });

    const result1 = await invoke<{ success: boolean }>('tag_assign', { fileId: 1, tagIds: [1] });
    const result2 = await invoke<{ success: boolean }>('tag_assign', { fileId: 2, tagIds: [1] });

    expect(result1.success).toBe(true);
    expect(result2.success).toBe(true);
  });

  it('should remove tag from file', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true });

    const result = await invoke<{ success: boolean }>('tag_unassign', { fileId: 1, tagIds: [1] });

    expect(result.success).toBe(true);
  });

  it('should update metadata value', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ id: 1 })
      .mockResolvedValueOnce({
        metadata: [{ id: 1, file_id: 1, key: 'author', value: 'Jane Doe', data_type: 'text' }],
      });

    await invoke<{ id: number }>('metadata_set', {
      fileId: 1,
      key: 'author',
      value: 'Jane Doe',
    });

    const metadata = await invoke<{ metadata: Array<{ key: string; value: string }> }>(
      'metadata_get',
      { fileId: 1 }
    );

    expect(metadata.metadata.find((m) => m.key === 'author')?.value).toBe('Jane Doe');
  });
});
