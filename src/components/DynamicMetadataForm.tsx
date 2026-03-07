import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { CategorySelect } from '@/components/CategorySelect';
import { TagManager } from '@/components/TagManager';
import { AuthorManager } from '@/components/AuthorManager';
import { getFieldsForCategory } from '@/config/categoryFields';
import type { Category, Tag, Author, MetadataType } from '@/types';

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
}: DynamicMetadataFormProps) {
  // Get the selected category name
  const selectedCategory = categories.find((c) => c.id === values.category_id);
  const categoryName = selectedCategory?.name || null;

  // Get fields for the selected category
  const dynamicFields = getFieldsForCategory(categoryName);

  // Helper to get metadata value by key
  const getMetadataValue = (key: string): string => {
    const meta = values.metadata.find((m) => m.key === key);
    return meta?.value ?? '';
  };

  // Handle display name change
  const handleDisplayNameChange = (display_name: string) => {
    onChange({ ...values, display_name });
  };

  // Handle category change
  const handleCategoryChange = (category_id: number | null) => {
    onChange({ ...values, category_id });
  };

  // Handle tag change
  const handleTagChange = (tag_ids: number[]) => {
    onChange({ ...values, tag_ids });
  };

  // Handle author change
  const handleAuthorChange = (author_ids: number[]) => {
    onChange({ ...values, author_ids });
  };

  // Handle metadata field change
  const handleMetadataFieldChange = (key: string, value: string | number | boolean, dataType: MetadataType) => {
    const newMetadata = values.metadata.filter((m) => m.key !== key);
    if (value !== '' && value !== null && value !== undefined) {
      newMetadata.push({
        key,
        value: String(value),
        data_type: dataType,
      });
    }
    onChange({ ...values, metadata: newMetadata });
  };

  // Render dynamic field based on type
  const renderDynamicField = (field: { key: string; label: string; type: string; placeholder?: string; options?: string[] }) => {
    switch (field.type) {
      case 'authors':
        return (
          <div className="space-y-2" key={field.key}>
            <Label className="text-sm font-medium">{field.label}</Label>
            <AuthorManager
              authors={authors}
              selectedAuthorIds={values.author_ids}
              onAuthorAssign={handleAuthorChange}
              onAuthorCreate={onAuthorCreate}
            />
          </div>
        );

      case 'tags':
        return (
          <div className="space-y-2" key={field.key}>
            <Label className="text-sm font-medium">{field.label}</Label>
            <TagManager
              tags={tags}
              selectedTagIds={values.tag_ids}
              onTagAssign={handleTagChange}
              onTagCreate={onTagCreate}
            />
          </div>
        );

      case 'text':
        return (
          <div className="space-y-2" key={field.key}>
            <Label className="text-sm font-medium">{field.label}</Label>
            <Input
              type="text"
              value={getMetadataValue(field.key)}
              onChange={(e) => handleMetadataFieldChange(field.key, e.target.value, 'text')}
              placeholder={field.placeholder}
            />
          </div>
        );

      case 'number':
        return (
          <div className="space-y-2" key={field.key}>
            <Label className="text-sm font-medium">{field.label}</Label>
            <Input
              type="number"
              value={getMetadataValue(field.key)}
              onChange={(e) => handleMetadataFieldChange(field.key, e.target.value, 'number')}
            />
          </div>
        );

      case 'date':
        return (
          <div className="space-y-2" key={field.key}>
            <Label className="text-sm font-medium">{field.label}</Label>
            <Input
              type="date"
              value={getMetadataValue(field.key)}
              onChange={(e) => handleMetadataFieldChange(field.key, e.target.value, 'date')}
            />
          </div>
        );

      case 'image':
        // For now, skip image field in the simplified version
        return null;

      default:
        return null;
    }
  };

  return (
    <div className="space-y-4">
      {/* Display Name */}
      <div className="space-y-2">
        <Label className="text-sm font-medium">Display Name</Label>
        <Input
          value={values.display_name}
          onChange={(e) => handleDisplayNameChange(e.target.value)}
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
      {dynamicFields.map((field) => renderDynamicField(field))}
    </div>
  );
}