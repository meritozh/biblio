import { useState, useEffect } from 'react';
import {
  settingsGet,
  settingsSet,
  recompressCovers,
  onRecompressCoversProgress,
  type RecompressCoversResult,
} from '@/lib/tauri';
import { Separator } from '@/components/ui/separator';

export function DebugSetting() {
  const [importMode, setImportMode] = useState<'move' | 'copy'>('move');
  const [loading, setLoading] = useState(true);

  // Recompress state. `running` flips on at click and off when the
  // command resolves; `progress` carries the latest event payload from
  // the backend; `result` is the final summary shown when done.
  const [recompressRunning, setRecompressRunning] = useState(false);
  const [recompressProgress, setRecompressProgress] = useState<{
    done: number;
    total: number;
    skipped: number;
  } | null>(null);
  const [recompressResult, setRecompressResult] =
    useState<RecompressCoversResult | null>(null);
  const [recompressError, setRecompressError] = useState<string | null>(null);

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

  const handleRecompress = async () => {
    if (recompressRunning) return;
    const ok = window.confirm(
      'Recompress every cover in the library to JPEG q=72 / max 800 px?\n\n' +
        'This is lossy and irreversible — existing high-resolution covers ' +
        'will lose detail. Recommended only if the DB has grown too large.'
    );
    if (!ok) return;

    setRecompressRunning(true);
    setRecompressProgress(null);
    setRecompressResult(null);
    setRecompressError(null);

    // Subscribe before invoking so we don't miss the immediate "0 / N"
    // baseline event the backend emits before the first row decodes.
    const unlisten = await onRecompressCoversProgress((p) => {
      setRecompressProgress(p);
    });

    try {
      const result = await recompressCovers();
      setRecompressResult(result);
    } catch (e) {
      setRecompressError(String(e));
    } finally {
      unlisten();
      setRecompressRunning(false);
    }
  };

  if (loading) {
    return <div className="text-sm text-muted-foreground min-h-[60px]">Loading...</div>;
  }

  const pct =
    recompressProgress && recompressProgress.total > 0
      ? Math.round((recompressProgress.done / recompressProgress.total) * 100)
      : 0;

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
          <h3 className="text-sm font-semibold">Recompress covers</h3>
          <p className="text-xs text-muted-foreground">
            Re-encode every cover already in the DB to JPEG q=72, max 800 px wide.
            Lossy and irreversible — only run if cover blobs have grown too large.
          </p>
        </div>
        <button
          type="button"
          onClick={handleRecompress}
          disabled={recompressRunning}
          className="px-3 py-2 rounded-xl text-xs font-medium bg-secondary text-secondary-foreground hover:bg-secondary/80 disabled:opacity-50 disabled:cursor-not-allowed transition-colors duration-200"
        >
          {recompressRunning ? 'Recompressing…' : 'Recompress all covers'}
        </button>
        {recompressProgress && (
          <div className="space-y-1.5">
            <div className="h-1.5 rounded-full bg-muted overflow-hidden">
              <div
                className="h-full bg-primary transition-[width] duration-200"
                style={{ width: `${pct}%` }}
              />
            </div>
            <p className="text-xs text-muted-foreground">
              {recompressProgress.done} / {recompressProgress.total} done
              {recompressProgress.skipped > 0 && (
                <span className="text-destructive ml-1.5">
                  · {recompressProgress.skipped} skipped
                </span>
              )}
            </p>
          </div>
        )}
        {recompressResult && !recompressRunning && (
          <p className="text-xs text-muted-foreground">
            Done. Recompressed {recompressResult.recompressed} of{' '}
            {recompressResult.total} cover{recompressResult.total === 1 ? '' : 's'}
            {recompressResult.skipped > 0 && (
              <span className="text-destructive ml-1.5">
                · {recompressResult.skipped} skipped (decode failure)
              </span>
            )}
            .
          </p>
        )}
        {recompressError && (
          <p className="text-xs text-destructive">Failed: {recompressError}</p>
        )}
      </div>
    </div>
  );
}
