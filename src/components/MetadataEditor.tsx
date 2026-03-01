import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Plus, Trash2 } from 'lucide-react';
import type { Metadata } from '@/types';

interface MetadataEditorProps {
  metadata: Metadata[];
  onUpdate: (key: string, value: string, dataType?: string) => Promise<void>;
  onDelete: (key: string) => Promise<void>;
}

export function MetadataEditor({ metadata, onUpdate, onDelete }: MetadataEditorProps) {
  const [newKey, setNewKey] = useState('');
  const [newValue, setNewValue] = useState('');
  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [editValue, setEditValue] = useState('');

  const handleAdd = async () => {
    if (newKey.trim() && newValue.trim()) {
      await onUpdate(newKey.trim(), newValue.trim());
      setNewKey('');
      setNewValue('');
    }
  };

  const handleUpdate = async (key: string) => {
    if (editValue.trim()) {
      await onUpdate(key, editValue.trim());
      setEditingKey(null);
      setEditValue('');
    }
  };

  return (
    <div className="space-y-2">
      <div className="text-sm font-medium">Metadata</div>
      <div className="space-y-1">
        {metadata.map((m) => (
          <div key={m.id} className="flex items-center gap-2 text-sm">
            {editingKey === m.key ? (
              <>
                <span className="text-muted-foreground w-24 truncate">{m.key}:</span>
                <Input
                  value={editValue}
                  onChange={(e) => setEditValue(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleUpdate(m.key)}
                  className="h-6 flex-1 text-xs"
                  autoFocus
                />
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => handleUpdate(m.key)}
                  className="h-6 px-2"
                >
                  Save
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => {
                    setEditingKey(null);
                    setEditValue('');
                  }}
                  className="h-6 px-2"
                >
                  Cancel
                </Button>
              </>
            ) : (
              <>
                <span className="text-muted-foreground w-24 truncate">{m.key}:</span>
                <span className="flex-1 truncate">{m.value}</span>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => {
                    setEditingKey(m.key);
                    setEditValue(m.value);
                  }}
                  className="h-6 px-2"
                >
                  Edit
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => onDelete(m.key)}
                  className="h-6 px-2"
                >
                  <Trash2 className="h-3 w-3" />
                </Button>
              </>
            )}
          </div>
        ))}
      </div>
      <div className="flex gap-2">
        <Input
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
          placeholder="Key"
          className="h-7 w-24 text-xs"
        />
        <Input
          value={newValue}
          onChange={(e) => setNewValue(e.target.value)}
          placeholder="Value"
          className="h-7 flex-1 text-xs"
          onKeyDown={(e) => e.key === 'Enter' && handleAdd()}
        />
        <Button size="sm" variant="outline" onClick={handleAdd} className="h-7 px-2">
          <Plus className="h-3 w-3" />
        </Button>
      </div>
    </div>
  );
}
