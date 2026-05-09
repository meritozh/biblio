import { useRef, useMemo } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import {
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Clock,
  Loader2,
  X,
} from 'lucide-react';
import type { RemoteUploadProgress } from '@/types';

interface RemoteUploadProgressPanelProps {
  uploads: RemoteUploadProgress[];
  minimized: boolean;
  onMinimize: () => void;
  onExpand: () => void;
  onDismiss: () => void;
  onClearCompleted: () => void;
}

const ROW_HEIGHT = 36;
const MAX_VISIBLE_ROWS = 8;

export function RemoteUploadProgressPanel({
  uploads,
  minimized,
  onMinimize,
  onExpand,
  onDismiss,
  onClearCompleted,
}: RemoteUploadProgressPanelProps) {
  const counts = useMemo(() => {
    let pending = 0;
    let uploading = 0;
    let done = 0;
    let failed = 0;
    for (const u of uploads) {
      if (u.status === 'pending') pending++;
      else if (u.status === 'uploading') uploading++;
      else if (u.status === 'success') done++;
      else if (u.status === 'error') failed++;
    }
    return { pending, uploading, done, failed, total: uploads.length };
  }, [uploads]);

  const inFlight = counts.pending + counts.uploading;
  const completedAny = counts.done + counts.failed > 0;
  // Full dismiss only when there's nothing queued or running. Otherwise the
  // user can only minimize — protects against losing visibility into work
  // they just started.
  const canDismiss = inFlight === 0;

  if (minimized) {
    return (
      <div className="fixed bottom-4 right-4 bg-background border border-border rounded-full shadow-lg z-50 flex items-center pl-3 pr-1.5 py-1 gap-2 text-xs">
        {inFlight > 0 ? (
          <Loader2 className="h-3.5 w-3.5 animate-spin text-primary shrink-0" />
        ) : counts.failed > 0 ? (
          <AlertCircle className="h-3.5 w-3.5 text-destructive shrink-0" />
        ) : (
          <CheckCircle2 className="h-3.5 w-3.5 text-success shrink-0" />
        )}
        <span className="text-foreground/80">
          {counts.done}/{counts.total} done
          {counts.failed > 0 && (
            <span className="text-destructive ml-1.5">· {counts.failed}</span>
          )}
        </span>
        <button
          type="button"
          onClick={onExpand}
          className="text-muted-foreground hover:text-foreground p-1 rounded-full"
          aria-label="Expand upload panel"
        >
          <ChevronUp className="h-3.5 w-3.5" />
        </button>
      </div>
    );
  }

  return (
    <div className="fixed bottom-4 right-4 w-80 bg-background border border-border rounded-xl shadow-lg z-50 flex flex-col max-h-[60vh]">
      <div className="flex items-center justify-between px-3 py-2 border-b border-border">
        <span className="text-sm font-medium">Uploading to Baidu Pan</span>
        <div className="flex items-center gap-0.5">
          <button
            type="button"
            onClick={onMinimize}
            className="text-muted-foreground hover:text-foreground p-1 rounded"
            aria-label="Minimize panel"
            title="Minimize"
          >
            <ChevronDown className="h-4 w-4" />
          </button>
          <button
            type="button"
            onClick={onDismiss}
            disabled={!canDismiss}
            className="text-muted-foreground hover:text-foreground disabled:opacity-30 disabled:cursor-not-allowed p-1 rounded"
            aria-label={canDismiss ? 'Dismiss panel' : 'Cannot dismiss while uploads are in flight'}
            title={canDismiss ? 'Dismiss' : 'Wait for uploads to finish'}
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      </div>

      <UploadList uploads={uploads} />

      <div className="flex items-center justify-between px-3 py-2 border-t border-border text-xs text-muted-foreground">
        <span>
          {counts.done}/{counts.total} done
          {counts.failed > 0 && (
            <span className="text-destructive ml-1.5">· {counts.failed} failed</span>
          )}
          {inFlight > 0 && <span className="ml-1.5">· {inFlight} in flight</span>}
        </span>
        {completedAny && (
          <button
            type="button"
            onClick={onClearCompleted}
            className="text-muted-foreground hover:text-foreground underline-offset-2 hover:underline"
          >
            Clear completed
          </button>
        )}
      </div>
    </div>
  );
}

function UploadList({ uploads }: { uploads: RemoteUploadProgress[] }) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: uploads.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 6,
  });
  const virtualItems = virtualizer.getVirtualItems();
  const totalSize = virtualizer.getTotalSize();

  // Cap the visible scroll area at MAX_VISIBLE_ROWS — above that, scroll.
  const listMaxHeight = Math.min(uploads.length, MAX_VISIBLE_ROWS) * ROW_HEIGHT;

  return (
    <div ref={scrollRef} className="overflow-y-auto" style={{ height: listMaxHeight }}>
      <div style={{ height: totalSize, position: 'relative' }}>
        {virtualItems.map((virtualRow) => {
          const u = uploads[virtualRow.index];
          if (!u) return null;
          return (
            <div
              key={u.file_id}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                right: 0,
                height: ROW_HEIGHT,
                transform: `translateY(${virtualRow.start}px)`,
              }}
              className="flex items-center gap-2 px-3 hover:bg-secondary/50 transition-colors duration-200"
            >
              <StatusIcon status={u.status} />
              <div className="flex-1 min-w-0">
                <p className="text-xs truncate" title={u.file_name}>
                  {u.file_name}
                </p>
                {u.status === 'error' && u.error && (
                  <p className="text-[10px] text-destructive truncate" title={u.error}>
                    {u.error}
                  </p>
                )}
              </div>
              <span className="text-[10px] text-muted-foreground shrink-0">
                {labelFor(u.status)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function StatusIcon({ status }: { status: RemoteUploadProgress['status'] }) {
  switch (status) {
    case 'pending':
      return <Clock className="h-3.5 w-3.5 text-muted-foreground shrink-0" />;
    case 'uploading':
      return <Loader2 className="h-3.5 w-3.5 animate-spin text-primary shrink-0" />;
    case 'success':
      return <CheckCircle2 className="h-3.5 w-3.5 text-success shrink-0" />;
    case 'error':
      return <AlertCircle className="h-3.5 w-3.5 text-destructive shrink-0" />;
    case 'skipped':
      return <AlertCircle className="h-3.5 w-3.5 text-muted-foreground shrink-0" />;
  }
}

function labelFor(status: RemoteUploadProgress['status']): string {
  switch (status) {
    case 'pending':
      return 'Queued';
    case 'uploading':
      return 'Uploading…';
    case 'success':
      return 'Done';
    case 'error':
      return 'Failed';
    case 'skipped':
      return 'Skipped';
  }
}
