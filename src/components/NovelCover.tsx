import { useMemo } from 'react';
import { gradientFromSeed, tagSeed } from '@/lib/colorHash';
import type { Tag } from '@/types';

interface NovelCoverProps {
  tags?: Tag[];
  fileId: number;
  displayName: string;
  /** Optional reading-progress string (e.g. "1-12章"). Rendered as a
   *  small italic line centered under the display name, so the card's
   *  cover surface carries both pieces of identity instead of pushing
   *  progress into a separate row under the cover. */
  progress?: string | null;
}

export function NovelCover({ tags, fileId, displayName, progress }: NovelCoverProps) {
  const gradient = useMemo(() => gradientFromSeed(tagSeed(tags, fileId)), [tags, fileId]);

  return (
    <div
      className="relative h-full w-full"
      style={{
        backgroundImage: `linear-gradient(${gradient.angle}deg, ${gradient.from}, ${gradient.to})`,
      }}
    >
      <span
        aria-hidden="true"
        className="pointer-events-none absolute inset-y-0 left-0 w-[5px] bg-gradient-to-r from-black/20 to-transparent"
      />
      <span
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 ring-1 ring-inset ring-black/5"
      />
      <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 px-4 py-6">
        <h3 className="line-clamp-5 break-words text-center text-[15px] font-semibold leading-[1.15] tracking-normal text-[#2a2420]/85">
          {displayName}
        </h3>
        {progress && (
          <p className="line-clamp-1 max-w-full text-center text-[11px] font-normal leading-tight text-[#2a2420]/65">
            {progress}
          </p>
        )}
      </div>
    </div>
  );
}
