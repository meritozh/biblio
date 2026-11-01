import { beforeEach, describe, expect, it } from 'vitest';
import {
  clonePresetConditions,
  deleteSavedFilter,
  listSavedFilters,
  saveFilterPreset,
} from '@/lib/savedFilters';
import type { Condition } from '@/lib/filters';

const sample: Condition[] = [
  { id: 'c1', field: 'favorite', op: 'is', value: true },
  { id: 'c2', field: 'local_cache', op: 'not_empty' },
];

describe('savedFilters', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('saves and lists named presets', () => {
    saveFilterPreset('Remote favorites', sample);
    const listed = listSavedFilters();
    expect(listed).toHaveLength(1);
    expect(listed[0]?.name).toBe('Remote favorites');
    expect(listed[0]?.conditions).toHaveLength(2);
  });

  it('overwrites a preset with the same name (case-insensitive)', () => {
    saveFilterPreset('Mine', sample);
    saveFilterPreset('mine', [{ id: 'c3', field: 'favorite', op: 'is', value: false }]);
    const listed = listSavedFilters();
    expect(listed).toHaveLength(1);
    expect(listed[0]?.conditions).toEqual([
      { id: 'c3', field: 'favorite', op: 'is', value: false },
    ]);
  });

  it('rejects empty name or empty conditions', () => {
    expect(() => saveFilterPreset('  ', sample)).toThrow(/name/i);
    expect(() => saveFilterPreset('Empty', [])).toThrow(/empty/i);
  });

  it('deletes presets and clones conditions with fresh ids', () => {
    const saved = saveFilterPreset('Temp', sample);
    deleteSavedFilter(saved.id);
    expect(listSavedFilters()).toHaveLength(0);

    const cloned = clonePresetConditions(sample);
    expect(cloned).toHaveLength(2);
    expect(cloned[0]?.id).not.toBe(sample[0]?.id);
    expect(cloned[0]?.field).toBe('favorite');
  });
});
