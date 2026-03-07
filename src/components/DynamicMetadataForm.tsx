import { useState, useEffect } from 'react';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { CategorySelect } from '@/components/CategorySelect';
import { TagManager } from '@/components/TagManager';
import { MetadataField, getMetadataDataType } from '@/components/MetadataField';
import { getFieldsForCategory } from '@/config/categoryFields';
import type { Category, Tag, Author, Metadata, MetadataType } from '@/types';

export interface DynamicMetadataFormValues {
  display_name: string;
  category_id: number | null;
  tag_ids: number[];
  author_ids: number[];
  metadata: Array<{ key: string; value: string; data_type: MetadataType }>;
  cover_data?: string;
}

interface DynamicMetadataFormProps {
  values: DynamicMetadataFormValues;
  onChange: (values: DynamicMetadataFormValues) => void;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onCategoryCreated?: (category: Category) => void;
  onTagCreate?: (name: string) => Promise<Tag>;
  onAuthorCreate?: (name: string) => Promise<Author>;
  existingMetadata?: Metadata[];
  existingCover?: string;
}

export function DynamicMetadataForm({
  values,
  onChange,
  categories,
  tags,
  authors,
  onCategoryCreated,
  onTagCreate,
  onAuthorCreate,
  existingMetadata = [],
  existingCover,
}: DynamicMetadataFormProps) {
  const [fieldValues, setFieldValues] = useState<Record<string, unknown>>({});

  // Get the selected category name
  const selectedCategory = categories.find((c) => c.id === values.category_id);
  const categoryName = selectedCategory?.name || null;

  // Get fields for the selected category
  const dynamicFields = getFieldsForCategory(categoryName);

  // Initialize field values from existing metadata
  useEffect(() => {
    const initial: Record<string, unknown> = {};

    // Initialize from existing metadata
    for (const meta of existingMetadata) {
      initial[meta.key] = meta.value;
    }

    // Initialize authors field
    initial['authors'] = values.author_ids;

    // Initialize with default values
    for (const field of dynamicFields) {
      if (initial[field.key] === undefined) {
        if (field.type === 'authors') {
          initial[field.key] = values.author_ids;
        } else if (field.type === 'tags') {
          initial[field.key] = values.tag_ids;
        } else if (field.type === 'boolean') {
          initial[field.key] = false;
        } else if (field.type === 'number') {
          initial[field.key] = field.defaultValue ? Number(field.defaultValue) : 0;
        } else if (field.type === 'image') {
          initial[field.key] = existingCover || '';
        } else {
          initial[field.key] = field.defaultValue || '';
        }
      }
    }

    setFieldValues(initial);
  }, [dynamicFields, existingMetadata, existingCover, values.author_ids, values.tag_ids]);

  // Handle field value changes
  const handleFieldChange = (key: string, value: unknown) => {
    setFieldValues((prev) => ({
      ...prev,
      [key]: value,
    }));
  };

  // Update parent when field values change
  useEffect(() => {
    const metadata: Array<{ key: string; value: string; data_type: MetadataType }> = [];
    let cover_data: string | undefined;

    for (const field of dynamicFields) {
      const fieldValue = fieldValues[field.key];

      // Skip authors and tags (handled separately)
      if (field.type === 'authors') {
        continue;
      }

      if (field.type === 'tags') {
        continue;
      }

      // Handle image field
      if (field.type === 'image') {
        if (fieldValue && typeof fieldValue === 'string') {
          cover_data = fieldValue;
        }
        continue;
      }

      // Convert value to string for metadata
      let stringValue = '';
      if (field.type === 'boolean') {
        stringValue = fieldValue ? 'true' : 'false';
      } else if (fieldValue !== undefined && fieldValue !== null) {
        stringValue = String(fieldValue);
      }

      if (stringValue) {
        metadata.push({
          key: field.key,
          value: stringValue,
          data_type: getMetadataDataType(field),
        });
      }
    }

    // Get author_ids from field values
    const author_ids = (fieldValues['authors'] as number[]) || [];

    onChange({
      ...values,
      author_ids,
      metadata,
      cover_data,
    });
  }, [fieldValues, dynamicFields]);

  // Handle category change
  const handleCategoryChange = (category_id: number | null) => {
    onChange({
      ...values,
      category_id,
    });
  };

  // Handle tag change
  const handleTagChange = (tag_ids: number[]) => {
    onChange({
      ...values,
      tag_ids,
    });
  };

  return (
    <div className="space-y-4">
      {/* Display Name */}
      <div className="space-y-2">
        <Label className="text-sm font-medium">Display Name</Label>
        <Input
          value={values.display_name}
          onChange={(e) => onChange({ ...values, display_name: e.target.value })}
          placeholder="File name"
        />
      </div>

      {/* Category */}
      <div className="space-y-2">
        <Label className="text-sm font-medium">Category</Label>
        <CategorySelect
          categories={categories}
          value={values.category_id}
          onValueChange={handleCategoryChange}
          onCategoryCreated={onCategoryCreated}
        />
      </div>

      {/* Tags */}
      <div className="space-y-2">
        <Label className="text-sm font-medium">Tags</Label>
        <TagManager
          tags={tags}
          selectedTagIds={values.tag_ids}
          onTagAssign={handleTagChange}
          onTagCreate={onTagCreate}
        />
      </div>

      {/* Dynamic Fields */}
      {dynamicFields.map((field) => (
        <MetadataField
          key={field.key}
          field={field}
          value={fieldValues[field.key] as string | number | boolean | number[]}
          onChange={(value) => handleFieldChange(field.key, value)}
          authors={authors}
          tags={tags}
          onAuthorCreate={onAuthorCreate}
          onTagCreate={onTagCreate}
        />
      ))}
    </div>
  );
}