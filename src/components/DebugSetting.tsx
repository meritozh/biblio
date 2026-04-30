import { useState, useEffect } from 'react';
import {
  settingsGet,
  settingsSet,
  recategorizeUncategorizedAsNovel,
} from '@/lib/tauri';
import { Separator } from '@/components/ui/separator';
import { Button } from '@/components/ui/button';

export function DebugSetting() {
  const [importMode, setImportMode] = useState<'move' | 'copy'>('move');
  const [remoteUpload, setRemoteUpload] = useState(true);
  const [loading, setLoading] = useState(true);
  const [migrating, setMigrating] = useState(false);
  const [migrateResult, setMigrateResult] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([
      settingsGet('import_mode'),
      settingsGet('debug_remote_upload_enabled'),
    ])
      .then(([mode, upload]) => {
        if (mode === 'copy') setImportMode('copy');
        setRemoteUpload(upload !== 'false');
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, []);

  const handleImportModeChange = async (newMode: 'move' | 'copy') => {
    setImportMode(newMode);
    await settingsSet('import_mode', newMode);
  };

  const handleRemoteUploadChange = async (enabled: boolean) => {
    setRemoteUpload(enabled);
    await settingsSet('debug_remote_upload_enabled', enabled ? 'true' : 'false');
  };

  const handleRecategorize = async () => {
    if (migrating) return;
    setMigrating(true);
    setMigrateResult(null);
    try {
      const count = await recategorizeUncategorizedAsNovel();
      setMigrateResult(
        count === 0
          ? 'No uncategorized files to migrate.'
          : `Moved ${count} file${count === 1 ? '' : 's'} to the novel category.`
      );
    } catch (e) {
      const msg = String(e);
      setMigrateResult(
        msg.includes('NOVEL_CATEGORY_NOT_FOUND')
          ? 'Create a category named "novel" first, then try again.'
          : `Migration failed: ${msg}`
      );
    } finally {
      setMigrating(false);
    }
  };

  if (loading) {
    return <div className="text-sm text-muted-foreground min-h-[60px]">Loading...</div>;
  }

  return (
    <div className="space-y-6">
      <div className="space-y-3">
        <div>
          <h3 className="text-sm font-semibold">Import Behavior</h3>
          <p className="text-xs text-muted-foreground">
            Choose whether to move or copy files into the storage folder
          </p>
        </div>
        <div className="flex gap-2">
          {(['move', 'copy'] as const).map((m) => (
            <button
              key={m}
              type="button"
              onClick={() => handleImportModeChange(m)}
              className={`flex-1 px-3 py-2 rounded-xl text-xs font-medium transition-colors duration-200 capitalize ${
                importMode === m
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-secondary text-secondary-foreground hover:bg-secondary/80'
              }`}
            >
              {m}
            </button>
          ))}
        </div>
        <p className="text-xs text-muted-foreground">
          {importMode === 'move'
            ? 'Files are moved into storage. The original is removed.'
            : 'Files are copied into storage. The original stays in place.'}
        </p>
      </div>

      <Separator />

      <div className="space-y-3">
        <div>
          <h3 className="text-sm font-semibold">Remote Upload</h3>
          <p className="text-xs text-muted-foreground">
            When disabled, files marked as remote are imported locally instead
          </p>
        </div>
        <div className="flex gap-2">
          {([true, false] as const).map((v) => (
            <button
              key={String(v)}
              type="button"
              onClick={() => handleRemoteUploadChange(v)}
              className={`flex-1 px-3 py-2 rounded-xl text-xs font-medium transition-colors duration-200 ${
                remoteUpload === v
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-secondary text-secondary-foreground hover:bg-secondary/80'
              }`}
            >
              {v ? 'Enabled' : 'Disabled'}
            </button>
          ))}
        </div>
        <p className="text-xs text-muted-foreground">
          {remoteUpload
            ? 'Files with remote storage will be uploaded to Baidu Netdisk.'
            : 'Remote upload is skipped; files are saved locally regardless of storage kind.'}
        </p>
      </div>

      <Separator />

      <div className="space-y-3">
        <div>
          <h3 className="text-sm font-semibold">Migrate Uncategorized → Novel</h3>
          <p className="text-xs text-muted-foreground">
            One-time cleanup: re-assigns every file with no category to the
            "novel" category and moves them out of the retired
            {' '}<code>_uncategorized</code> folder.
          </p>
        </div>
        <Button
          variant="secondary"
          size="sm"
          onClick={handleRecategorize}
          disabled={migrating}
        >
          {migrating ? 'Migrating…' : 'Run migration'}
        </Button>
        {migrateResult && (
          <p className="text-xs text-muted-foreground" aria-live="polite">
            {migrateResult}
          </p>
        )}
      </div>
    </div>
  );
}
