import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

interface FileListResponse {
  files: Array<{
    id: number;
    path: string;
    display_name: string;
    category_id: number | null;
    file_status: string;
    created_at: string;
    updated_at: string;
  }>;
  total: number;
}

interface FileCreateResponse {
  id: number;
}

interface FileWithDetails {
  id: number;
  path: string;
  display_name: string;
  category_id: number | null;
  file_status: string;
  created_at: string;
  updated_at: string;
  category: { id: number; name: string } | null;
  tags: Array<{ id: number; name: string }>;
  metadata: Array<{ id: number; key: string; value: string }>;
}

interface FileCheckStatusResponse {
  updated: Array<{ id: number; status: string }>;
}

vi.mock('@tauri-apps/api/core');

describe('file_list command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should list files without filters', async () => {
    const mockResponse: FileListResponse = {
      files: [
        {
          id: 1,
          path: '/test/file1.pdf',
          display_name: 'File 1',
          category_id: null,
          file_status: 'available',
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-01T00:00:00Z',
        },
      ],
      total: 1,
    };

    vi.mocked(invoke).mockResolvedValue(mockResponse);

    const result = await invoke<FileListResponse>('file_list', {});

    expect(invoke).toHaveBeenCalledWith('file_list', {});
    expect(result).toEqual(mockResponse);
  });

  it('should list files with category filter', async () => {
    const mockResponse: FileListResponse = {
      files: [],
      total: 0,
    };

    vi.mocked(invoke).mockResolvedValue(mockResponse);

    const result = await invoke<FileListResponse>('file_list', { categoryId: 1 });

    expect(invoke).toHaveBeenCalledWith('file_list', { categoryId: 1 });
    expect(result.total).toBe(0);
  });

  it('should list files with pagination', async () => {
    const mockResponse: FileListResponse = {
      files: [],
      total: 100,
    };

    vi.mocked(invoke).mockResolvedValue(mockResponse);

    const result = await invoke<FileListResponse>('file_list', { limit: 10, offset: 20 });

    expect(invoke).toHaveBeenCalledWith('file_list', { limit: 10, offset: 20 });
    expect(result.total).toBe(100);
  });

  it('should list files with status filter', async () => {
    const mockResponse: FileListResponse = {
      files: [],
      total: 0,
    };

    vi.mocked(invoke).mockResolvedValue(mockResponse);

    await invoke<FileListResponse>('file_list', { status: 'missing' });

    expect(invoke).toHaveBeenCalledWith('file_list', { status: 'missing' });
  });
});

describe('file_get command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should get a file with full details', async () => {
    const mockResponse: FileWithDetails = {
      id: 1,
      path: '/test/file.pdf',
      display_name: 'Test File',
      category_id: 1,
      file_status: 'available',
      created_at: '2024-01-01T00:00:00Z',
      updated_at: '2024-01-01T00:00:00Z',
      category: { id: 1, name: 'Novel' },
      tags: [{ id: 1, name: 'favorite' }],
      metadata: [{ id: 1, key: 'author', value: 'John Doe' }],
    };

    vi.mocked(invoke).mockResolvedValue(mockResponse);

    const result = await invoke<FileWithDetails>('file_get', { id: 1 });

    expect(invoke).toHaveBeenCalledWith('file_get', { id: 1 });
    expect(result.id).toBe(1);
    expect(result.category?.name).toBe('Novel');
    expect(result.tags).toHaveLength(1);
  });

  it('should handle file not found', async () => {
    vi.mocked(invoke).mockRejectedValue('File not found');

    await expect(invoke<FileWithDetails>('file_get', { id: 999 })).rejects.toBe('File not found');
  });
});

describe('file_create command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should create a new file', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 1 });

    const result = await invoke<FileCreateResponse>('file_create', {
      path: '/test/new.pdf',
      displayName: 'New File',
      categoryId: 1,
    });

    expect(invoke).toHaveBeenCalledWith('file_create', {
      path: '/test/new.pdf',
      displayName: 'New File',
      categoryId: 1,
    });
    expect(result.id).toBe(1);
  });

  it('should reject duplicate file paths', async () => {
    vi.mocked(invoke).mockRejectedValue('FILE_ALREADY_EXISTS');

    await expect(
      invoke<FileCreateResponse>('file_create', {
        path: '/test/existing.pdf',
        displayName: 'Duplicate',
      })
    ).rejects.toBe('FILE_ALREADY_EXISTS');
  });

  it('should create file with tags and metadata', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 2 });

    const result = await invoke<FileCreateResponse>('file_create', {
      path: '/test/file.pdf',
      displayName: 'File with Meta',
      tagIds: [1, 2],
      metadata: [{ key: 'author', value: 'Jane', dataType: 'text' }],
    });

    expect(result.id).toBe(2);
  });
});

describe('file_search command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should search files by query', async () => {
    const mockResponse: FileListResponse = {
      files: [
        {
          id: 1,
          path: '/test/novel.pdf',
          display_name: 'My Novel',
          category_id: 1,
          file_status: 'available',
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-01T00:00:00Z',
        },
      ],
      total: 1,
    };

    vi.mocked(invoke).mockResolvedValue(mockResponse);

    const result = await invoke<FileListResponse>('file_search', { query: 'novel' });

    expect(invoke).toHaveBeenCalledWith('file_search', { query: 'novel' });
    expect(result.files).toHaveLength(1);
  });

  it('should search with category filter', async () => {
    const mockResponse: FileListResponse = {
      files: [],
      total: 0,
    };

    vi.mocked(invoke).mockResolvedValue(mockResponse);

    await invoke<FileListResponse>('file_search', { query: 'test', categoryId: 1 });

    expect(invoke).toHaveBeenCalledWith('file_search', { query: 'test', categoryId: 1 });
  });
});

describe('file_check_status command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should check status of specific files', async () => {
    const mockResponse: FileCheckStatusResponse = {
      updated: [{ id: 1, status: 'missing' }],
    };

    vi.mocked(invoke).mockResolvedValue(mockResponse);

    const result = await invoke<FileCheckStatusResponse>('file_check_status', {
      fileIds: [1, 2, 3],
    });

    expect(invoke).toHaveBeenCalledWith('file_check_status', { fileIds: [1, 2, 3] });
    expect(result.updated).toHaveLength(1);
  });

  it('should check all files when no ids provided', async () => {
    const mockResponse: FileCheckStatusResponse = {
      updated: [],
    };

    vi.mocked(invoke).mockResolvedValue(mockResponse);

    const result = await invoke<FileCheckStatusResponse>('file_check_status', {});

    expect(invoke).toHaveBeenCalledWith('file_check_status', {});
    expect(result.updated).toHaveLength(0);
  });
});

describe('file_update command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should update file display name', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true });

    await invoke<{ success: boolean }>('file_update', { id: 1, displayName: 'New Name' });

    expect(invoke).toHaveBeenCalledWith('file_update', { id: 1, displayName: 'New Name' });
  });

  it('should update file category', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true });

    await invoke<{ success: boolean }>('file_update', { id: 1, categoryId: 2 });

    expect(invoke).toHaveBeenCalledWith('file_update', { id: 1, categoryId: 2 });
  });
});

describe('file_delete command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should delete a file', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true });

    await invoke<{ success: boolean }>('file_delete', { id: 1 });

    expect(invoke).toHaveBeenCalledWith('file_delete', { id: 1 });
  });
});
