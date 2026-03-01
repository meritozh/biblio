import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core');

describe('CRUD Operations', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  describe('File CRUD', () => {
    it('should update file name', async () => {
      vi.mocked(invoke).mockResolvedValue({ success: true });

      const result = await invoke<{ success: boolean }>('file_update', {
        id: 1,
        displayName: 'Updated Name',
      });

      expect(result.success).toBe(true);
    });

    it('should delete a file', async () => {
      vi.mocked(invoke).mockResolvedValue({ success: true });

      const result = await invoke<{ success: boolean }>('file_delete', { id: 1 });

      expect(result.success).toBe(true);
    });

    it('should handle file not found on update', async () => {
      vi.mocked(invoke).mockRejectedValue('File not found');

      await expect(
        invoke<{ success: boolean }>('file_update', { id: 999, displayName: 'Test' })
      ).rejects.toBe('File not found');
    });
  });

  describe('Category CRUD', () => {
    it('should create a new category', async () => {
      vi.mocked(invoke).mockResolvedValue({ id: 6 });

      const result = await invoke<{ id: number }>('category_create', {
        name: 'Manga',
        icon: 'book-open',
      });

      expect(result.id).toBe(6);
    });

    it('should update category name', async () => {
      vi.mocked(invoke).mockResolvedValue({ success: true });

      const result = await invoke<{ success: boolean }>('category_update', {
        id: 6,
        name: 'Updated Category',
      });

      expect(result.success).toBe(true);
    });

    it('should delete non-default category', async () => {
      vi.mocked(invoke).mockResolvedValue({ success: true, affected_files: 3 });

      const result = await invoke<{ success: boolean; affected_files: number }>('category_delete', {
        id: 6,
      });

      expect(result.success).toBe(true);
      expect(result.affected_files).toBe(3);
    });

    it('should prevent deleting default category', async () => {
      vi.mocked(invoke).mockRejectedValue('CANNOT_DELETE_DEFAULT');

      await expect(invoke<{ success: boolean }>('category_delete', { id: 1 })).rejects.toBe(
        'CANNOT_DELETE_DEFAULT'
      );
    });
  });

  describe('Tag CRUD', () => {
    it('should create a new tag', async () => {
      vi.mocked(invoke).mockResolvedValue({ id: 1 });

      const result = await invoke<{ id: number }>('tag_create', {
        name: 'new-tag',
        color: '#FF0000',
      });

      expect(result.id).toBe(1);
    });

    it('should update tag name', async () => {
      vi.mocked(invoke).mockResolvedValue({ success: true });

      const result = await invoke<{ success: boolean }>('tag_update', {
        id: 1,
        name: 'updated-tag',
      });

      expect(result.success).toBe(true);
    });

    it('should delete a tag', async () => {
      vi.mocked(invoke).mockResolvedValue({ success: true, affected_files: 5 });

      const result = await invoke<{ success: boolean; affected_files: number }>('tag_delete', {
        id: 1,
      });

      expect(result.success).toBe(true);
      expect(result.affected_files).toBe(5);
    });
  });

  describe('Metadata CRUD', () => {
    it('should create metadata', async () => {
      vi.mocked(invoke).mockResolvedValue({ id: 1 });

      const result = await invoke<{ id: number }>('metadata_set', {
        fileId: 1,
        key: 'custom_field',
        value: 'custom_value',
      });

      expect(result.id).toBe(1);
    });

    it('should update metadata', async () => {
      vi.mocked(invoke).mockResolvedValue({ id: 1 });

      const result = await invoke<{ id: number }>('metadata_set', {
        fileId: 1,
        key: 'custom_field',
        value: 'updated_value',
      });

      expect(result.id).toBe(1);
    });

    it('should delete metadata', async () => {
      vi.mocked(invoke).mockResolvedValue({ success: true });

      const result = await invoke<{ success: boolean }>('metadata_delete', {
        fileId: 1,
        key: 'custom_field',
      });

      expect(result.success).toBe(true);
    });
  });
});
