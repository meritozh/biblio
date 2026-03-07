import type { FieldConfig } from '@/types';

// Fields shown for all files regardless of category
export const DEFAULT_FIELDS: FieldConfig[] = [
  { key: 'authors', label: 'Authors', type: 'authors' },
];

// Category-specific fields (merge with DEFAULT_FIELDS when category matches)
export const CATEGORY_FIELDS: Record<string, FieldConfig[]> = {
  'Novels': [
    { key: 'progress', label: 'Progress', type: 'text', placeholder: 'e.g., 16/100, 连载中, 完结' },
  ],
  'Comics': [
    { key: 'volume', label: 'Volume', type: 'number' },
    { key: 'cover', label: 'Cover Image', type: 'image' },
  ],
  // Categories not listed will only show DEFAULT_FIELDS
};

// Helper function to get fields for a category
export function getFieldsForCategory(categoryName: string | null): FieldConfig[] {
  if (!categoryName || !CATEGORY_FIELDS[categoryName]) {
    return DEFAULT_FIELDS;
  }
  // Merge default fields with category-specific, avoiding duplicates by key
  const categoryFields = CATEGORY_FIELDS[categoryName];
  const defaultKeys = new Set(DEFAULT_FIELDS.map(f => f.key));
  const merged = [...DEFAULT_FIELDS];
  for (const field of categoryFields) {
    if (!defaultKeys.has(field.key)) {
      merged.push(field);
    }
  }
  return merged;
}