import { useState, useRef } from 'react';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Switch } from '@/components/ui/switch';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { AuthorManager } from '@/components/AuthorManager';
import { TagManager } from '@/components/TagManager';
import { Upload, X } from 'lucide-react';
import type { FieldConfig, Author, Tag, MetadataType } from '@/types';

interface MetadataFieldProps {
  field: FieldConfig;
  value: string | number | boolean | number[];
  onChange: (value: string | number | boolean | number[]) => void;
  authors?: Author[];
  tags?: Tag[];
  onAuthorCreate?: (name: string) => Promise<Author>;
  onTagCreate?: (name: string) => Promise<Tag>;
}

export function MetadataField({
  field,
  value,
  onChange,
  authors = [],
  tags = [],
  onAuthorCreate,
  onTagCreate,
}: MetadataFieldProps) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [imagePreview, setImagePreview] = useState<string | null>(null);

  const handleImageSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      const reader = new FileReader();
      reader.onload = () => {
        const result = reader.result as string;
        setImagePreview(result);
        // Store base64 data without the data URL prefix
        const base64Data = result.split(',')[1] ?? '';
        onChange(base64Data);
      };
      reader.readAsDataURL(file);
    }
  };

  const handleImageRemove = () => {
    setImagePreview(null);
    onChange('');
    if (fileInputRef.current) {
      fileInputRef.current.value = '';
    }
  };

  switch (field.type) {
    case 'text':
      return (
        <div className="space-y-1">
          <Label className="text-sm font-medium">{field.label}</Label>
          <Input
            type="text"
            value={value as string}
            onChange={(e) => onChange(e.target.value)}
            placeholder={field.placeholder}
          />
        </div>
      );

    case 'number':
      return (
        <div className="space-y-1">
          <Label className="text-sm font-medium">{field.label}</Label>
          <Input
            type="number"
            value={value as number}
            onChange={(e) => onChange(Number(e.target.value))}
          />
        </div>
      );

    case 'date':
      return (
        <div className="space-y-1">
          <Label className="text-sm font-medium">{field.label}</Label>
          <Input
            type="date"
            value={value as string}
            onChange={(e) => onChange(e.target.value)}
          />
        </div>
      );

    case 'boolean':
      return (
        <div className="flex items-center justify-between">
          <Label className="text-sm font-medium">{field.label}</Label>
          <Switch
            checked={value as boolean}
            onCheckedChange={(checked) => onChange(checked)}
          />
        </div>
      );

    case 'select':
      return (
        <div className="space-y-1">
          <Label className="text-sm font-medium">{field.label}</Label>
          <Select
            value={value as string}
            onValueChange={(val) => onChange(val)}
          >
            <SelectTrigger>
              <SelectValue placeholder={`Select ${field.label}`} />
            </SelectTrigger>
            <SelectContent>
              {field.options?.map((option) => (
                <SelectItem key={option} value={option}>
                  {option}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      );

    case 'authors':
      return (
        <div className="space-y-1">
          <Label className="text-sm font-medium">{field.label}</Label>
          <AuthorManager
            authors={authors}
            selectedAuthorIds={value as number[]}
            onAuthorAssign={(ids) => onChange(ids)}
            onAuthorCreate={onAuthorCreate}
          />
        </div>
      );

    case 'tags':
      return (
        <div className="space-y-1">
          <Label className="text-sm font-medium">{field.label}</Label>
          <TagManager
            tags={tags}
            selectedTagIds={value as number[]}
            onTagAssign={(ids) => onChange(ids)}
            onTagCreate={onTagCreate}
          />
        </div>
      );

    case 'image':
      return (
        <div className="space-y-1">
          <Label className="text-sm font-medium">{field.label}</Label>
          <div className="flex gap-2 items-start">
            {imagePreview || (value as string) ? (
              <div className="relative">
                <img
                  src={imagePreview || `data:image/png;base64,${value}`}
                  alt={field.label}
                  className="w-24 h-32 object-cover rounded border"
                />
                <Button
                  type="button"
                  variant="destructive"
                  size="icon"
                  className="absolute -top-2 -right-2 h-5 w-5"
                  onClick={handleImageRemove}
                >
                  <X className="h-3 w-3" />
                </Button>
              </div>
            ) : (
              <Button
                type="button"
                variant="outline"
                className="w-24 h-32 flex flex-col gap-1"
                onClick={() => fileInputRef.current?.click()}
              >
                <Upload className="h-6 w-6" />
                <span className="text-xs">Upload</span>
              </Button>
            )}
            <input
              ref={fileInputRef}
              type="file"
              accept="image/*"
              onChange={handleImageSelect}
              className="hidden"
            />
          </div>
        </div>
      );

    default:
      return (
        <div className="space-y-1">
          <Label className="text-sm font-medium">{field.label}</Label>
          <Input
            type="text"
            value={value as string}
            onChange={(e) => onChange(e.target.value)}
            placeholder={field.placeholder}
          />
        </div>
      );
  }
}

// Helper function to determine metadata data type from field config
export function getMetadataDataType(field: FieldConfig): MetadataType {
  switch (field.type) {
    case 'number':
      return 'number';
    case 'date':
      return 'date';
    case 'boolean':
      return 'boolean';
    default:
      return 'text';
  }
}