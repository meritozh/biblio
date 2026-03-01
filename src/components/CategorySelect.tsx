import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import type { Category } from '@/types';

interface CategorySelectProps {
  categories: Category[];
  value?: number | null;
  onValueChange: (categoryId: number | null) => void;
  placeholder?: string;
}

export function CategorySelect({
  categories,
  value,
  onValueChange,
  placeholder = 'Select category',
}: CategorySelectProps) {
  return (
    <Select
      value={value?.toString() ?? ''}
      onValueChange={(v) => onValueChange(v ? parseInt(v, 10) : null)}
    >
      <SelectTrigger>
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="">Uncategorized</SelectItem>
        {categories.map((category) => (
          <SelectItem key={category.id} value={category.id.toString()}>
            {category.icon && <span className="mr-2">{category.icon}</span>}
            {category.name}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
