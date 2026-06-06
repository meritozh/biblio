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

/** Format a byte count as a compact human string (e.g. "4.2 GB"). */
function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(1)} ${units[i]}`;
}

interface RemoteUploadProgressPanelProps {
  uploads: RemoteUploadProgress[];
  minimized: boolean;
  onMinimize: () => void;
  onExpand: () => void;
  onDismiss: () => void;
  onClearCompleted: () => void;
}

const ROW_HEIGHT = 48;
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
      <div className="bg-background border border-border rounded-full shadow-lg flex items-center pl-3 pr-1.5 py-1 gap-2 text-xs">
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
    <div className="w-80 bg-background border border-border rounded-xl shadow-lg flex flex-col max-h-[60vh]">
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
  // Per-file speed sampler: last seen (bytes, timestamp) → bytes/sec between
  // successive progress ticks. Lives in a ref so it survives re-renders without
  // re-triggering them. Keyed by file_id.
  const speedRef = useRef<Map<number, { bytes: number; t: number; bps: number }>>(
    new Map()
  );
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

          const hasProgress =
            u.status === 'uploading' &&
            u.total_bytes != null &&
            u.total_bytes > 0 &&
            u.uploaded_bytes != null;
          const pct = hasProgress
            ? Math.min(100, Math.round((u.uploaded_bytes! / u.total_bytes!) * 100))
            : null;

          // Speed only makes sense for the network upload phase — encrypt/hash
          // are local disk and their "speed" is misleading. Sample keyed by
          // file so the rate resets cleanly when the phase rolls over (bytes
          // jump back to ~0). bps in bytes/sec from successive ticks.
          let bps = 0;
          if (hasProgress && u.phase === 'uploading') {
            const now = performance.now();
            const prev = speedRef.current.get(u.file_id);
            if (prev && u.uploaded_bytes! > prev.bytes && now > prev.t) {
              bps = ((u.uploaded_bytes! - prev.bytes) / (now - prev.t)) * 1000;
            } else if (prev) {
              bps = prev.bps; // no new bytes this render — keep last estimate
            }
            speedRef.current.set(u.file_id, {
              bytes: u.uploaded_bytes!,
              t: now,
              bps,
            });
          }

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
                {u.status === 'error' && u.error ? (
                  <p className="text-[10px] text-destructive truncate" title={u.error}>
                    {u.error}
                  </p>
                ) : hasProgress ? (
                  <div className="flex items-center gap-1.5 mt-0.5">
                    <div className="flex-1 h-1 rounded-full bg-secondary overflow-hidden">
                      <div
                        className="h-full bg-primary transition-[width] duration-200"
                        style={{ width: `${pct}%` }}
                      />
                    </div>
                    <span className="text-[10px] text-muted-foreground shrink-0 tabular-nums">
                      {pct}%{bps > 0 ? ` · ${formatBytes(bps)}/s` : ''}
                    </span>
                  </div>
                ) : null}
              </div>
              <span className="text-[10px] text-muted-foreground shrink-0">
                {u.status === 'uploading' ? phaseLabel(u.phase) : labelFor(u.status)}
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

/** Phase-specific label for an in-flight upload. The `uploading` status spans
 *  three multi-minute phases (encrypt → hash → POST); showing the phase keeps
 *  a large file from looking stuck on a blank "Uploading…". */
function phaseLabel(phase: RemoteUploadProgress['phase']): string {
  switch (phase) {
    case 'encrypting':
      return 'Encrypting…';
    case 'hashing':
      return 'Hashing…';
    case 'uploading':
    case undefined:
      return 'Uploading…';
  }
}
