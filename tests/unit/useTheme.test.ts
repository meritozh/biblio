import { beforeEach, describe, expect, it } from 'vitest';
import { initTheme } from '@/lib/useTheme';

describe('initTheme', () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.classList.remove('dark');
    delete document.documentElement.dataset.theme;
  });

  it('defaults to porcelain design theme without dark class', () => {
    initTheme();
    expect(document.documentElement.dataset.theme).toBe('porcelain');
    expect(document.documentElement.classList.contains('dark')).toBe(false);
  });

  it('restores graphite design theme and dark appearance', () => {
    localStorage.setItem('design-theme', 'graphite');
    localStorage.setItem('theme', 'dark');
    initTheme();
    expect(document.documentElement.dataset.theme).toBe('graphite');
    expect(document.documentElement.classList.contains('dark')).toBe(true);
  });

  it('ignores unknown design theme values', () => {
    localStorage.setItem('design-theme', 'neon');
    initTheme();
    expect(document.documentElement.dataset.theme).toBe('porcelain');
  });
});
