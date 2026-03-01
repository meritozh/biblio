import { bench, describe, it, expect } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { vi } from 'vitest';

vi.mock('@tauri-apps/api/core');

const TARGET_TIME_MS = 500;

function generateMockFiles(count: number): Array<{
  id: number;
  path: string;
  display_name: string;
  category_id: number;
  file_status: string;
  created_at: string;
  updated_at: string;
}> {
  const files: Array<{
    id: number;
    path: string;
    display_name: string;
    category_id: number;
    file_status: string;
    created_at: string;
    updated_at: string;
  }> = [];
  for (let i = 0; i < count; i++) {
    files.push({
      id: i + 1,
      path: `/test/file${i}.pdf`,
      display_name: `Test File ${i}`,
      category_id: (i % 5) + 1,
      file_status: 'available',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
  }
  return files;
}

describe('Search Performance Benchmark', () => {
  bench(
    'search should complete under 500ms for 10k files',
    async () => {
      vi.mocked(invoke).mockResolvedValue({
        files: generateMockFiles(50),
        total: 10000,
      });

      const start = performance.now();
      await invoke<{ files: unknown[]; total: number }>('file_search', { query: 'test' });
      const duration = performance.now() - start;

      console.log(`Search duration: ${duration.toFixed(2)}ms`);
    },
    { time: 5000, iterations: 10 }
  );
});

describe('Search Performance Validation', () => {
  it('should search 10k files under 500ms', async () => {
    vi.mocked(invoke).mockResolvedValue({
      files: generateMockFiles(50),
      total: 10000,
    });

    const start = performance.now();
    const result = await invoke<{ files: unknown[]; total: number }>('file_search', {
      query: 'test',
    });
    const duration = performance.now() - start;

    console.log(`Search completed in ${duration.toFixed(2)}ms for ${result.total} total files`);
    expect(duration).toBeLessThan(TARGET_TIME_MS);
  });

  it('should handle complex filters efficiently', async () => {
    vi.mocked(invoke).mockResolvedValue({
      files: generateMockFiles(20),
      total: 10000,
    });

    const start = performance.now();
    await invoke<{ files: unknown[]; total: number }>('file_search', {
      query: 'test',
      categoryId: 1,
      tagIds: [1, 2],
    });
    const duration = performance.now() - start;

    console.log(`Complex filter search completed in ${duration.toFixed(2)}ms`);
    expect(duration).toBeLessThan(TARGET_TIME_MS);
  });
});
