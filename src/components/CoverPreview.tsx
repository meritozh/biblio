import { useEffect, useState } from 'react';
import { coverGet, preparedCoverGet } from '@/lib/tauri';

/** Cover thumbnail sizing for the small preview slot used in form fields
 *  and the dupe-compare panel. 2:3 aspect is the book-cover convention;
 *  pick a single size here so the form + dupe + grid renderers stay
 *  visually consistent. */
const PREVIEW_CLASS = 'h-24 w-16 object-cover rounded-md border';
const PLACEHOLDER_CLASS =
  'h-24 w-16 rounded-md border border-dashed flex items-center justify-center text-xs text-muted-foreground';

/** Self-fetches the existing cover from the DB by file id, mirroring the
 *  grid card's `CardCover` so we don't relay bytes through formValues
 *  state. Renders the dashed placeholder on rejection (no cover row) or
 *  while loading. Used by the edit dialog's cover field AND the import
 *  dupe panel's cover comparison row. */
export function ExistingCoverPreview({ fileId }: { fileId: number }) {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    coverGet(fileId)
      .then(({ data, mime_type }) => {
        if (!cancelled) setSrc(`data:${mime_type};base64,${data}`);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [fileId]);
  return src ? (
    <img src={src} alt="Cover preview" className={PREVIEW_CLASS} />
  ) : (
    <div className={PLACEHOLDER_CLASS}>…</div>
  );
}

/** Self-fetches a staged cover (Phase-2 pipeline output not yet committed)
 *  from the Rust-side cache and renders it via a Blob URL. The base64
 *  string from the IPC is converted to bytes and then dropped — the only
 *  retained reference is the blob URL, whose underlying bytes live in
 *  the browser's blob store off the JS heap. Revokes on unmount so memory
 *  releases promptly when the dialog scrolls the row out of view. */
export function StagedCoverPreview({ stagedPath }: { stagedPath: string }) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    let createdUrl: string | null = null;
    preparedCoverGet(stagedPath)
      .then(({ data, mime_type }) => {
        if (cancelled) return;
        const binary = atob(data);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
        const blob = new Blob([bytes], { type: mime_type });
        createdUrl = URL.createObjectURL(blob);
        setUrl(createdUrl);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      if (createdUrl) URL.revokeObjectURL(createdUrl);
    };
  }, [stagedPath]);
  return url ? (
    <img src={url} alt="Cover preview" className={PREVIEW_CLASS} />
  ) : (
    <div className={PLACEHOLDER_CLASS}>…</div>
  );
}

/** Renders a base64-encoded user-uploaded cover blob inline — no fetch.
 *  Used when the user has just picked a replacement in the form. Mirrors
 *  the inline-render branch from DynamicMetadataForm so the dupe panel
 *  can show the same image without duplicating the data-URI builder. */
export function InlineCoverPreview({
  coverData,
  coverMimeType,
}: {
  coverData: string;
  coverMimeType?: string;
}) {
  const src = `data:${coverMimeType ?? 'image/jpeg'};base64,${coverData}`;
  return <img src={src} alt="Cover preview" className={PREVIEW_CLASS} />;
}

/** Empty-state placeholder, exported so callers can render the same
 *  "no cover" cell the previews fall back to. The dupe panel needs this
 *  for the new-side cell when there's no inline blob, no staged path,
 *  and no DB cover to fetch. */
export function CoverPlaceholder({ label = '…' }: { label?: string }) {
  return <div className={PLACEHOLDER_CLASS}>{label}</div>;
}
