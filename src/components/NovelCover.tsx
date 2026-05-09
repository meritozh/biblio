import { useMemo } from 'react';
import { gradientFromSeed, tagSeed } from '@/lib/colorHash';
import type { Tag } from '@/types';

interface NovelCoverProps {
  tags?: Tag[];
  fileId: number;
  displayName: string;
}

export function NovelCover({ tags, fileId, displayName }: NovelCoverProps) {
  const gradient = useMemo(
    () => gradientFromSeed(tagSeed(tags, fileId)),
    [tags, fileId]
  );

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
      <div className="absolute inset-0 flex items-center justify-center px-4 py-6">
        <h3 className="font-serif-display text-center text-foreground/85 text-[15px] leading-[1.15] tracking-tight line-clamp-5 break-words">
          {displayName}
        </h3>
      </div>
    </div>
  );
}
