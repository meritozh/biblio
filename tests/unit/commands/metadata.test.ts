import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

interface MetadataSetResponse {
  id: number;
}

interface MetadataGetResponse {
  metadata: Array<{
    id: number;
    file_id: number;
    key: string;
    value: string;
    data_type: string;
  }>;
}

vi.mock('@tauri-apps/api/core');

describe('metadata_set command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should set metadata for a file', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 1 });

    const result = await invoke<MetadataSetResponse>('metadata_set', {
      fileId: 1,
      key: 'author',
      value: 'John Doe',
      dataType: 'text',
    });

    expect(invoke).toHaveBeenCalledWith('metadata_set', {
      fileId: 1,
      key: 'author',
      value: 'John Doe',
      dataType: 'text',
    });
    expect(result.id).toBe(1);
  });

  it('should update existing metadata', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 1 });

    await invoke<MetadataSetResponse>('metadata_set', {
      fileId: 1,
      key: 'author',
      value: 'Jane Doe',
    });

    expect(invoke).toHaveBeenCalledWith('metadata_set', {
      fileId: 1,
      key: 'author',
      value: 'Jane Doe',
    });
  });

  it('should handle different data types', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 1 });

    await invoke<MetadataSetResponse>('metadata_set', {
      fileId: 1,
      key: 'page_count',
      value: '150',
      dataType: 'number',
    });

    expect(invoke).toHaveBeenCalledWith('metadata_set', {
      fileId: 1,
      key: 'page_count',
      value: '150',
      dataType: 'number',
    });
  });

  it('should handle boolean data type', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 2 });

    await invoke<MetadataSetResponse>('metadata_set', {
      fileId: 1,
      key: 'is_read',
      value: 'true',
      dataType: 'boolean',
    });

    expect(invoke).toHaveBeenCalledWith('metadata_set', {
      fileId: 1,
      key: 'is_read',
      value: 'true',
      dataType: 'boolean',
    });
  });

  it('should handle date data type', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 3 });

    await invoke<MetadataSetResponse>('metadata_set', {
      fileId: 1,
      key: 'published_date',
      value: '2024-01-15',
      dataType: 'date',
    });

    expect(invoke).toHaveBeenCalledWith('metadata_set', {
      fileId: 1,
      key: 'published_date',
      value: '2024-01-15',
      dataType: 'date',
    });
  });
});

describe('metadata_get command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should get all metadata for a file', async () => {
    const mockResponse: MetadataGetResponse = {
      metadata: [
        { id: 1, file_id: 1, key: 'author', value: 'John Doe', data_type: 'text' },
        { id: 2, file_id: 1, key: 'page_count', value: '150', data_type: 'number' },
      ],
    };

    vi.mocked(invoke).mockResolvedValue(mockResponse);

    const result = await invoke<MetadataGetResponse>('metadata_get', { fileId: 1 });

    expect(invoke).toHaveBeenCalledWith('metadata_get', { fileId: 1 });
    expect(result.metadata).toHaveLength(2);
  });

  it('should return empty array when no metadata exists', async () => {
    const mockResponse: MetadataGetResponse = {
      metadata: [],
    };

    vi.mocked(invoke).mockResolvedValue(mockResponse);

    const result = await invoke<MetadataGetResponse>('metadata_get', { fileId: 999 });

    expect(result.metadata).toEqual([]);
  });
});

describe('metadata_delete command', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should delete a metadata field', async () => {
    vi.mocked(invoke).mockResolvedValue({ success: true });

    await invoke<{ success: boolean }>('metadata_delete', { fileId: 1, key: 'author' });

    expect(invoke).toHaveBeenCalledWith('metadata_delete', { fileId: 1, key: 'author' });
  });
});
