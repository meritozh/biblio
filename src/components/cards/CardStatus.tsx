import { Cloud, HardDrive, Loader2 } from 'lucide-react';

interface CardStatusProps {
  storageKind?: string;
  isUploading: boolean;
  hasLocalCache: boolean;
}

/** Storage status pill — identical geometry across states; only icon and color
 *  change so the badge reads as one consistent visual element. */
export function CardStatus({ storageKind, isUploading, hasLocalCache }: CardStatusProps) {
  // `rounded-md` matches the card cover's corner radius so the badge reads
  // as part of the same visual system instead of a circular sticker.
  const wrapper =
    'flex items-center justify-center h-6 w-6 rounded-md bg-background/90 backdrop-blur-sm border border-border/40 shadow-sm';
  if (isUploading) {
    return (
      <div className={wrapper} title="Uploading…" aria-label="Uploading">
        <Loader2 className="h-3.5 w-3.5 animate-spin text-amber-600 dark:text-amber-400" />
      </div>
    );
  }
  if (storageKind === 'remote') {
    // The dot in the corner indicates a local cache is also present —
    // user can read the file without re-downloading.
    const title = hasLocalCache ? 'Synced to cloud · cached locally' : 'Synced to cloud';
    return (
      <div className={`${wrapper} relative`} title={title} aria-label={title}>
        <Cloud className="h-3.5 w-3.5 text-primary" />
        {hasLocalCache && (
          <span
            className="absolute -bottom-0.5 -right-0.5 h-2 w-2 rounded-full bg-success border border-background"
            aria-hidden="true"
          />
        )}
      </div>
    );
  }
  return (
    <div className={wrapper} title="Local only" aria-label="Local only">
      <HardDrive className="h-3.5 w-3.5 text-muted-foreground" />
    </div>
  );
}
