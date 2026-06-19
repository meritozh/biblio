import { Loader2, RefreshCw } from 'lucide-react';
import type { FileEntry } from '@/types';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  LUCKY_DIALOG_WIDTH,
  LuckyFileCards,
} from '@/components/LuckyFileCards';

interface LuckyDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  files: FileEntry[];
  loading: boolean;
  refreshing: boolean;
  error: string | null;
  canShuffle: boolean;
  onShuffle: () => void;
  onFileClick: (file: FileEntry) => void;
  onFileEdit?: (file: FileEntry) => void;
  onFileDelete?: (file: FileEntry) => void;
  remoteEnabled?: boolean;
}

export function LuckyDialog({
  open,
  onOpenChange,
  files,
  loading,
  refreshing,
  error,
  canShuffle,
  onShuffle,
  onFileClick,
  onFileEdit,
  onFileDelete,
  remoteEnabled,
}: LuckyDialogProps) {
  const hasFiles = files.length > 0;
  const showBlockingLoading = loading && !hasFiles;
  const showBlockingError = error && !hasFiles;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="!max-w-none"
        style={{ width: `min(calc(100vw - 2rem), ${LUCKY_DIALOG_WIDTH}px)` }}
      >
        <DialogHeader>
          <DialogTitle>Lucky</DialogTitle>
          <DialogDescription>
            Three random picks from the current category and active filters.
          </DialogDescription>
        </DialogHeader>

        <div className="min-h-24">
          {showBlockingLoading ? (
            <div className="flex items-center justify-center gap-2 py-8 text-sm text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" aria-hidden="true" />
              Picking...
            </div>
          ) : showBlockingError ? (
            <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive">
              {error}
            </div>
          ) : hasFiles ? (
            <div className="space-y-2">
              <LuckyFileCards
                files={files}
                onFileClick={onFileClick}
                onFileEdit={onFileEdit}
                onFileDelete={onFileDelete}
                remoteEnabled={remoteEnabled}
              />
              {error && (
                <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-2 text-xs text-destructive">
                  {error}
                </div>
              )}
            </div>
          ) : (
            <div className="rounded-lg border bg-secondary/30 p-4 text-sm text-muted-foreground">
              No files match the current scope.
            </div>
          )}
        </div>

        <DialogFooter>
          <Button
            variant="outline"
            size="sm"
            onClick={onShuffle}
            disabled={!canShuffle || loading || refreshing}
            className="gap-1.5"
          >
            <RefreshCw
              className={`h-3.5 w-3.5 ${refreshing ? 'animate-spin' : ''}`}
              aria-hidden="true"
            />
            Shuffle again
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
