import { memo, useEffect, useState } from 'react';
import { BookOpen } from 'lucide-react';
import { coverGet } from '@/lib/tauri';
import type { ComicCollection } from '@/types';
import { CARD_HEIGHT, CARD_WIDTH } from './constants';

interface CollectionCardProps {
  collection: ComicCollection;
  onOpen: (c: ComicCollection) => void;
}

/** Stacked-card visual for a comic collection. The preview cover comes from
 *  `cover_file_id`; the two offset rectangles behind the cover hint at the
 *  multi-file grouping without committing to a real cover stack (which would
 *  triple the IPC roundtrips per card). */
export const CollectionCard = memo(function CollectionCard({
  collection,
  onOpen,
}: CollectionCardProps) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (collection.cover_file_id == null) return;
    let cancelled = false;
    coverGet(collection.cover_file_id)
      .then(({ data, mime_type }) => {
        if (!cancelled) setSrc(`data:${mime_type};base64,${data}`);
      })
      .catch(() => { });
    return () => {
      cancelled = true;
    };
  }, [collection.cover_file_id]);

  const count = collection.file_ids.length;
  return (
    <div
      className="relative group"
      style={{ width: CARD_WIDTH, height: CARD_HEIGHT }}
    >
      <button
        type="button"
        onClick={() => onOpen(collection)}
        className="w-full h-full flex flex-col gap-2 text-left rounded-lg p-2 transition-colors hover:bg-muted/50 focus:outline-none focus:ring-2 focus:ring-ring"
        aria-label={`Open ${collection.title} (${count} items)`}
      >
        <div className="relative aspect-2/3 w-full">
          {/* Two offset card-shaped layers behind the cover, peeking out
           *  beyond the right/bottom edges to read as a stack instead of a
           *  single card. */}
          <div className="absolute inset-0 translate-x-1.5 translate-y-1.5 rounded-md bg-secondary/40 border border-border/60" />
          <div className="absolute inset-0 translate-x-0.5 translate-y-0.5 rounded-md bg-secondary/60 border border-border/60" />
          <div className="absolute inset-0 rounded-md overflow-hidden bg-secondary/40 border flex items-center justify-center">
            {src ? (
              <img src={src} alt="" className="h-full w-full object-cover" />
            ) : (
              <BookOpen className="h-8 w-8 text-muted-foreground/40" />
            )}
          </div>
        </div>
        <div className="space-y-0.5 min-w-0 px-0.5">
          <p
            className="text-sm font-medium leading-tight truncate"
            title={collection.title}
          >
            {collection.title}
          </p>
          <p className="text-xs text-muted-foreground">
            {count} {count === 1 ? 'volume' : 'volumes'}
          </p>
        </div>
      </button>
    </div>
  );
});
