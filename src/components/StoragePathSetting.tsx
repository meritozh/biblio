import { useState, useEffect } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { settingsGet, settingsSet, translateError } from '@/lib/tauri';
import { FolderOpen, AlertCircle, Check } from 'lucide-react';

export function StoragePathSetting() {
  const [storagePath, setStoragePath] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  useEffect(() => {
    settingsGet('storage_path').then((path) => {
      setStoragePath(path);
      setLoading(false);
    });
  }, []);

  const handleSelectFolder = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: 'Select Storage Folder',
    });

    if (selected && typeof selected === 'string') {
      setSaving(true);
      setError(null);
      setSuccess(false);

      try {
        await settingsSet('storage_path', selected);
        setStoragePath(selected);
        setSuccess(true);
        setTimeout(() => setSuccess(false), 3000);
      } catch (err) {
        const errorMsg = err instanceof Error ? err.message : String(err);
        setError(translateError(errorMsg));
      } finally {
        setSaving(false);
      }
    }
  };

  if (loading) {
    return <div className="text-sm text-muted-foreground min-h-[100px]">Loading...</div>;
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-medium">Storage Path</h3>
          <p className="text-xs text-muted-foreground">
            Files will be organized in category folders
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={handleSelectFolder}
          disabled={saving}
        >
          <FolderOpen className="h-4 w-4 mr-2" />
          {storagePath ? 'Change' : 'Select Folder'}
        </Button>
      </div>

      {storagePath ? (
        <div className="flex items-center gap-2">
          <Input
            value={storagePath}
            readOnly
            className="text-sm bg-muted"
          />
          {success && (
            <Check className="h-4 w-4 text-green-500" />
          )}
        </div>
      ) : (
        <div className="flex items-center gap-2 p-3 bg-yellow-50 dark:bg-yellow-950/20 rounded-md border border-yellow-200 dark:border-yellow-800">
          <AlertCircle className="h-4 w-4 text-yellow-600 dark:text-yellow-500" />
          <span className="text-sm text-yellow-800 dark:text-yellow-200">
            Select a storage folder to start adding files
          </span>
        </div>
      )}

      {error && (
        <p className="text-sm text-red-500">{error}</p>
      )}
    </div>
  );
}