import { useEffect, useRef } from 'react';
import { AlertCircle, CheckCircle2, Loader2, X } from 'lucide-react';
import type { RemoteUploadProgress } from '@/types';

interface RemoteUploadProgressPanelProps {
  uploads: RemoteUploadProgress[];
  onClose: () => void;
}

export function RemoteUploadProgressPanel({ uploads, onClose }: RemoteUploadProgressPanelProps) {
  const allDone = uploads.length > 0 && uploads.every(u => u.status !== 'uploading');
  const successCount = uploads.filter(u => u.status === 'success').length;
  const failCount = uploads.filter(u => u.status === 'error').length;
  const timerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => {
    if (allDone) {
      timerRef.current = setTimeout(onClose, 5000);
      return () => clearTimeout(timerRef.current);
    }
    return undefined;
  }, [allDone, onClose]);

  return (
    <div className="fixed bottom-4 right-4 w-80 bg-background border border-border rounded-xl shadow-lg z-50 flex flex-col max-h-[60vh]">
      <div className="flex items-center justify-between px-3 py-2 border-b border-border">
        <span className="text-sm font-medium">Uploading to Baidu Pan</span>
        <button type="button" onClick={onClose} className="text-muted-foreground hover:text-foreground">
          <X className="h-4 w-4" />
        </button>
      </div>
      <div className="overflow-y-auto flex-1 p-2 space-y-1">
        {uploads.map((u) => (
          <div key={u.file_id} className="flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-secondary/50 transition-colors duration-200">
            {u.status === 'uploading' && <Loader2 className="h-3.5 w-3.5 animate-spin text-primary shrink-0" />}
            {u.status === 'success' && <CheckCircle2 className="h-3.5 w-3.5 text-success shrink-0" />}
            {u.status === 'error' && <AlertCircle className="h-3.5 w-3.5 text-destructive shrink-0" />}
            {u.status === 'skipped' && <AlertCircle className="h-3.5 w-3.5 text-muted-foreground shrink-0" />}
            <div className="flex-1 min-w-0">
              <p className="text-xs truncate">{u.file_name}</p>
              {u.status === 'error' && u.error && (
                <p className="text-xs text-destructive truncate">{u.error}</p>
              )}
            </div>
            <span className="text-xs text-muted-foreground shrink-0">
              {u.status === 'uploading' ? 'Uploading...' : u.status === 'success' ? 'Done' : u.status === 'error' ? 'Failed' : 'Skipped'}
            </span>
          </div>
        ))}
      </div>
      {allDone && (
        <div className="px-3 py-2 border-t border-border text-xs text-muted-foreground">
          {failCount === 0
            ? `${successCount} file${successCount !== 1 ? 's' : ''} uploaded`
            : `${successCount} succeeded, ${failCount} failed`}
        </div>
      )}
      {!allDone && (
        <div className="px-3 py-2 border-t border-border text-xs text-muted-foreground">
          {successCount + failCount}/{uploads.length} files processed
        </div>
      )}
    </div>
  );
}
