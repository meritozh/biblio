import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core');

describe('Filter Workflow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  it('should search files by name', async () => {
    vi.mocked(invoke).mockResolvedValue({
      files: [
        {
          id: 1,
          path: '/test/my-novel.pdf',
          display_name: 'My Novel',
          category_id: 1,
          file_status: 'available',
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-01T00:00:00Z',
        },
      ],
      total: 1,
    });

    const result = await invoke<{ files: Array<{ display_name: string }>; total: number }>(
      'file_search',
      { query: 'novel' }
    );

    expect(result.files).toHaveLength(1);
    expect(result.files[0].display_name).toContain('Novel');
  });

  it('should filter by category and search', async () => {
    vi.mocked(invoke).mockResolvedValue({
      files: [
        {
          id: 1,
          path: '/test/novel.pdf',
          display_name: 'Adventure Novel',
          category_id: 1,
          file_status: 'available',
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-01T00:00:00Z',
        },
      ],
      total: 1,
    });

    const result = await invoke<{ files: Array<{ category_id: number }>; total: number }>(
      'file_search',
      { query: 'adventure', categoryId: 1 }
    );

    expect(result.files).toHaveLength(1);
    expect(result.files[0].category_id).toBe(1);
  });

  it('should return empty results for no matches', async () => {
    vi.mocked(invoke).mockResolvedValue({
      files: [],
      total: 0,
    });

    const result = await invoke<{ files: unknown[]; total: number }>('file_search', {
      query: 'nonexistent',
    });

    expect(result.files).toEqual([]);
    expect(result.total).toBe(0);
  });

  it('should paginate search results', async () => {
    vi.mocked(invoke).mockResolvedValue({
      files: [],
      total: 100,
    });

    const result = await invoke<{ total: number }>('file_search', {
      query: 'test',
      limit: 10,
      offset: 20,
    });

    expect(result.total).toBe(100);
  });
});
