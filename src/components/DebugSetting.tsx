import { useState, useEffect } from 'react';
import { settingsGet, settingsSet } from '@/lib/tauri';

export function DebugSetting() {
  const [importMode, setImportMode] = useState<'move' | 'copy'>('move');
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    settingsGet('import_mode')
      .then((mode) => {
        if (mode === 'copy') setImportMode('copy');
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, []);

  const handleImportModeChange = async (newMode: 'move' | 'copy') => {
    setImportMode(newMode);
    await settingsSet('import_mode', newMode);
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
    </div>
  );
}
