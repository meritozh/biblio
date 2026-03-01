import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

interface Category {
  id: number;
  name: string;
  icon: string | null;
  is_default: boolean;
  created_at: string;
}

interface CategoryCreateResponse {
  id: number;
}

interface CategoryDeleteResponse {
  success: boolean;
  affected_files: number;
}

vi.mock('@tauri-apps/api/core');

describe('category_list command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should list all categories', async () => {
    const mockCategories: Category[] = [
      { id: 1, name: 'Novel', icon: 'book', is_default: true, created_at: '2024-01-01T00:00:00Z' },
      { id: 2, name: 'Comic', icon: 'comic', is_default: true, created_at: '2024-01-01T00:00:00Z' },
    ];

    vi.mocked(invoke).mockResolvedValue(mockCategories);

    const result = await invoke<Category[]>('category_list');

    expect(invoke).toHaveBeenCalledWith('category_list');
    expect(result).toHaveLength(2);
    expect(result[0].name).toBe('Novel');
  });

  it('should return empty array when no categories exist', async () => {
    vi.mocked(invoke).mockResolvedValue([]);

    const result = await invoke<Category[]>('category_list');

    expect(result).toEqual([]);
  });
});

describe('category_get command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should get a single category', async () => {
    const mockCategory: Category = {
      id: 1,
      name: 'Novel',
      icon: 'book',
      is_default: true,
      created_at: '2024-01-01T00:00:00Z',
    };

    vi.mocked(invoke).mockResolvedValue(mockCategory);

    const result = await invoke<Category>('category_get', { id: 1 });

    expect(invoke).toHaveBeenCalledWith('category_get', { id: 1 });
    expect(result.name).toBe('Novel');
  });

  it('should handle category not found', async () => {
    vi.mocked(invoke).mockRejectedValue('CATEGORY_NOT_FOUND');

    await expect(invoke<Category>('category_get', { id: 999 })).rejects.toBe('CATEGORY_NOT_FOUND');
  });
});

describe('category_create command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should create a new category', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 6 });

    const result = await invoke<CategoryCreateResponse>('category_create', {
      name: 'Manga',
      icon: 'book-open',
    });

    expect(invoke).toHaveBeenCalledWith('category_create', { name: 'Manga', icon: 'book-open' });
    expect(result.id).toBe(6);
  });

  it('should reject duplicate category names', async () => {
    vi.mocked(invoke).mockRejectedValue('CATEGORY_EXISTS');

    await expect(invoke<CategoryCreateResponse>('category_create', { name: 'Novel' })).rejects.toBe(
      'CATEGORY_EXISTS'
    );
  });
});

describe('category_update command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should update category name', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true });

    await invoke<{ success: boolean }>('category_update', { id: 1, name: 'Updated Name' });

    expect(invoke).toHaveBeenCalledWith('category_update', { id: 1, name: 'Updated Name' });
  });

  it('should update category icon', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true });

    await invoke<{ success: boolean }>('category_update', { id: 1, icon: 'new-icon' });

    expect(invoke).toHaveBeenCalledWith('category_update', { id: 1, icon: 'new-icon' });
  });
});

describe('category_delete command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should delete a non-default category', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true, affected_files: 5 });

    const result = await invoke<CategoryDeleteResponse>('category_delete', { id: 6 });

    expect(invoke).toHaveBeenCalledWith('category_delete', { id: 6 });
    expect(result.success).toBe(true);
    expect(result.affected_files).toBe(5);
  });

  it('should reject deleting default category', async () => {
    vi.mocked(invoke).mockRejectedValue('CANNOT_DELETE_DEFAULT');

    await expect(invoke<CategoryDeleteResponse>('category_delete', { id: 1 })).rejects.toBe(
      'CANNOT_DELETE_DEFAULT'
    );
  });
});
