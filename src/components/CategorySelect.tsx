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

/** Pure picker over the seeded category list. New categories aren't created
 *  from the UI any more — the set is fixed by migration, and the Categories
 *  page is the only place to tune existing entries. */
export function CategorySelect({
  categories,
  value,
  onValueChange,
  placeholder = 'Select category',
}: CategorySelectProps) {
  return (
    <Select
      value={value != null ? value.toString() : undefined}
      onValueChange={(v) => onValueChange(parseInt(v, 10))}
    >
      <SelectTrigger>
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectContent>
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
