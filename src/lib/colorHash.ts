import type { Tag } from '@/types';

const FNV_OFFSET_BASIS = 0x811c9dc5;
const FNV_PRIME = 0x01000193;

function fnv1a(input: string): number {
  let hash = FNV_OFFSET_BASIS;
  for (let i = 0; i < input.length; i++) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, FNV_PRIME);
  }
  return hash >>> 0;
}

export function tagSeed(tags: Tag[] | undefined, fallbackId: number): number {
  const ids = (tags ?? []).map((t) => t.id).sort((a, b) => a - b);
  const key = ids.length > 0 ? ids.join(',') : `f${fallbackId}`;
  return fnv1a(key);
}

export interface CoverGradient {
  from: string;
  to: string;
  angle: number;
}

export function gradientFromSeed(seed: number): CoverGradient {
  const hueA = seed % 360;
  const hueB = (hueA + 30 + ((seed >>> 8) % 60)) % 360;
  const angle = ((seed >>> 16) % 12) * 15;
  return {
    from: `oklch(0.92 0.075 ${hueA})`,
    to: `oklch(0.78 0.10 ${hueB})`,
    angle,
  };
}
