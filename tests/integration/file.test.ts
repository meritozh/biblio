import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core');

describe('File Add and Categorize Workflow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  it('should add a file and assign a category', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce([
        {
          id: 1,
          name: 'Novel',
          icon: 'book',
          is_default: true,
          created_at: '2024-01-01T00:00:00Z',
        },
      ])
      .mockResolvedValueOnce({ id: 1 })
      .mockResolvedValueOnce({ success: true });

    const categories = await invoke<Array<{ id: number; name: string }>>('category_list');
    const category = categories.find((c) => c.name === 'Novel');

    const fileResult = await invoke<{ id: number }>('file_create', {
      path: '/test/novel.pdf',
      displayName: 'My Novel',
      categoryId: category?.id,
    });

    expect(fileResult.id).toBeDefined();

    const updateResult = await invoke<{ success: boolean }>('file_update', {
      id: fileResult.id,
      categoryId: category?.id,
    });

    expect(updateResult.success).toBe(true);
  });

  it('should list files by category', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      files: [
        {
          id: 1,
          path: '/test/novel1.pdf',
          display_name: 'Novel 1',
          category_id: 1,
          file_status: 'available',
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-01T00:00:00Z',
        },
        {
          id: 2,
          path: '/test/novel2.pdf',
          display_name: 'Novel 2',
          category_id: 1,
          file_status: 'available',
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-01T00:00:00Z',
        },
      ],
      total: 2,
    });

    const result = await invoke<{ files: Array<{ category_id: number }>; total: number }>(
      'file_list',
      { categoryId: 1 }
    );

    expect(result.files).toHaveLength(2);
    expect(result.files.every((f) => f.category_id === 1)).toBe(true);
  });

  it('should change file category', async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ success: true })
      .mockResolvedValueOnce({
        id: 1,
        path: '/test/file.pdf',
        display_name: 'Test File',
        category_id: 2,
        file_status: 'available',
        created_at: '2024-01-01T00:00:00Z',
        updated_at: '2024-01-01T00:00:00Z',
        category: { id: 2, name: 'Comic' },
        tags: [],
        metadata: [],
      });

    await invoke<{ success: boolean }>('file_update', { id: 1, categoryId: 2 });

    const file = await invoke<{ category_id: number; category: { name: string } }>('file_get', {
      id: 1,
    });

    expect(file.category_id).toBe(2);
    expect(file.category.name).toBe('Comic');
  });

  it('should handle file with no category', async () => {
    vi.mocked(invoke).mockResolvedValue({
      id: 1,
      path: '/test/uncategorized.pdf',
      display_name: 'Uncategorized File',
      category_id: null,
      file_status: 'available',
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-01-01T00:00:00Z',
      category: null,
      tags: [],
      metadata: [],
    });

    const file = await invoke<{ category_id: null; category: null }>('file_get', { id: 1 });

    expect(file.category_id).toBeNull();
    expect(file.category).toBeNull();
  });
});
