import { Input } from '@/components/ui/input';
import type { MetadataType } from '@/types';

interface MetadataFieldProps {
  label: string;
  value: string;
  dataType?: MetadataType;
  onChange: (value: string) => void;
  placeholder?: string;
}

export function MetadataField({
  label,
  value,
  dataType = 'text',
  onChange,
  placeholder,
}: MetadataFieldProps) {
  const type = dataType === 'number' ? 'number' : dataType === 'date' ? 'date' : 'text';

  return (
    <div className="space-y-1">
      <label className="text-sm font-medium">{label}</label>
      <Input
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
      />
    </div>
  );
}
