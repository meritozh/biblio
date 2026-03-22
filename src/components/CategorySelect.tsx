import { useState } from 'react';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  SelectSeparator,
} from '@/components/ui/select';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { categoryCreate } from '@/lib/tauri';
import type { Category } from '@/types';

interface CategorySelectProps {
  categories: Category[];
  value?: number | null;
  onValueChange: (categoryId: number | null) => void;
  onCategoryCreated?: (category: Category) => void;
  placeholder?: string;
}

const NONE_VALUE = 'none';
const CREATE_NEW_VALUE = '__create_new__';

export function CategorySelect({
  categories,
  value,
  onValueChange,
  onCategoryCreated,
  placeholder = 'Select category',
}: CategorySelectProps) {
  const [isCreating, setIsCreating] = useState(false);
  const [newCategoryName, setNewCategoryName] = useState('');
  const [isSaving, setIsSaving] = useState(false);

  const handleValueChange = (v: string) => {
    if (v === CREATE_NEW_VALUE) {
      setIsCreating(true);
      return;
    }
    onValueChange(v === NONE_VALUE ? null : parseInt(v, 10));
  };

  const handleCreateCategory = async () => {
    if (!newCategoryName.trim()) return;

    setIsSaving(true);
    try {
      const result = await categoryCreate(newCategoryName.trim());
      const newCategory: Category = {
        id: result.id,
        name: newCategoryName.trim(),
        icon: null,
        is_default: false,
        folder_name: null,
        created_at: new Date().toISOString(),
      };
      onCategoryCreated?.(newCategory);
      onValueChange(result.id);
      setNewCategoryName('');
      setIsCreating(false);
    } catch (error) {
      console.error('Failed to create category:', error);
    } finally {
      setIsSaving(false);
    }
  };

  const handleCancelCreate = () => {
    setNewCategoryName('');
    setIsCreating(false);
  };

  if (isCreating) {
    return (
      <div className="flex gap-2">
        <Input
          placeholder="Category name"
          value={newCategoryName}
          onChange={(e) => setNewCategoryName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              handleCreateCategory();
            } else if (e.key === 'Escape') {
              handleCancelCreate();
            }
          }}
          disabled={isSaving}
          className="flex-1"
        />
        <Button size="sm" onClick={handleCreateCategory} disabled={isSaving || !newCategoryName.trim()}>
          Save
        </Button>
        <Button size="sm" variant="outline" onClick={handleCancelCreate} disabled={isSaving}>
          Cancel
        </Button>
      </div>
    );
  }

  return (
    <Select
      value={value?.toString() ?? NONE_VALUE}
      onValueChange={handleValueChange}
    >
      <SelectTrigger>
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={NONE_VALUE}>Uncategorized</SelectItem>
        {categories.length > 0 && <SelectSeparator />}
        {categories.map((category) => (
          <SelectItem key={category.id} value={category.id.toString()}>
            {category.icon && <span className="mr-2">{category.icon}</span>}
            {category.name}
          </SelectItem>
        ))}
        <SelectSeparator />
        <SelectItem value={CREATE_NEW_VALUE}>+ Create new category</SelectItem>
      </SelectContent>
    </Select>
  );
}
