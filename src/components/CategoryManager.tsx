import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Plus, Pencil, Trash2 } from 'lucide-react';
import type { Category } from '@/types';

interface CategoryManagerProps {
  categories: Category[];
  onCreate: (name: string, icon?: string) => Promise<void>;
  onUpdate: (id: number, name?: string, icon?: string) => Promise<void>;
  onDelete: (id: number) => Promise<void>;
}

export function CategoryManager({
  categories,
  onCreate,
  onUpdate,
  onDelete,
}: CategoryManagerProps) {
  const [isCreating, setIsCreating] = useState(false);
  const [newName, setNewName] = useState('');
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editName, setEditName] = useState('');

  const handleCreate = async () => {
    if (newName.trim()) {
      await onCreate(newName.trim());
      setNewName('');
      setIsCreating(false);
    }
  };

  const handleUpdate = async (id: number) => {
    if (editName.trim()) {
      await onUpdate(id, editName.trim());
      setEditingId(null);
      setEditName('');
    }
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between">
        <h3 className="font-medium">Categories</h3>
        <Button size="sm" variant="outline" onClick={() => setIsCreating(true)}>
          <Plus className="h-4 w-4 mr-1" />
          Add
        </Button>
      </div>
      <div className="space-y-1">
        {categories.map((cat) => (
          <div
            key={cat.id}
            className="flex items-center justify-between p-2 rounded hover:bg-muted/50"
          >
            {editingId === cat.id ? (
              <div className="flex gap-2 flex-1">
                <Input
                  value={editName}
                  onChange={(e) => setEditName(e.target.value)}
                  className="h-8"
                />
                <Button size="sm" onClick={() => handleUpdate(cat.id)}>
                  Save
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => {
                    setEditingId(null);
                    setEditName('');
                  }}
                >
                  Cancel
                </Button>
              </div>
            ) : (
              <>
                <span className="flex items-center gap-2">
                  {cat.icon && <span>{cat.icon}</span>}
                  <span>{cat.name}</span>
                  {cat.is_default && (
                    <span className="text-xs text-muted-foreground">(default)</span>
                  )}
                </span>
                {!cat.is_default && (
                  <div className="flex gap-1">
                    <Button
                      size="icon"
                      variant="ghost"
                      className="h-8 w-8"
                      onClick={() => {
                        setEditingId(cat.id);
                        setEditName(cat.name);
                      }}
                    >
                      <Pencil className="h-4 w-4" />
                    </Button>
                    <Button
                      size="icon"
                      variant="ghost"
                      className="h-8 w-8 text-destructive"
                      onClick={() => onDelete(cat.id)}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                )}
              </>
            )}
          </div>
        ))}
      </div>

      <Dialog open={isCreating} onOpenChange={setIsCreating}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create Category</DialogTitle>
          </DialogHeader>
          <div className="space-y-4 py-4">
            <Input
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="Category name"
              onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
            />
            <div className="flex justify-end gap-2">
              <Button variant="outline" onClick={() => setIsCreating(false)}>
                Cancel
              </Button>
              <Button onClick={handleCreate}>Create</Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </div>
  );
}
