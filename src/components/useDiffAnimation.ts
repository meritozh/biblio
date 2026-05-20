import { useEffect, useRef } from 'react';

/**
 * Compute whether the current render should skip enter/move animations
 * because the list changed by more than `threshold` keys vs the previous
 * render.
 *
 * Picks up bulk mutations (delete-all-unused-tags, load-more pulling in
 * a full page, etc.) and suppresses animations for those renders so the
 * UI doesn't fire 50 simultaneous transitions. Single-row mutations
 * (delete one tag, edit one author) stay below the threshold and
 * animate normally.
 *
 * `prevKeysRef` updates in a layout effect *after* the diff is computed
 * for the current render, so the diff always compares against the
 * last-rendered state — not the in-flight state.
 */
export function useDiffAnimation<T>(
  items: ReadonlyArray<T>,
  getKey: (item: T) => string | number,
  threshold: number = 20
): boolean {
  const prevKeysRef = useRef<Set<string | number>>(new Set());

  const currentKeys = new Set<string | number>();
  let addedCount = 0;
  for (const item of items) {
    const k = getKey(item);
    currentKeys.add(k);
    if (!prevKeysRef.current.has(k)) addedCount += 1;
  }
  let removedCount = 0;
  for (const k of prevKeysRef.current) {
    if (!currentKeys.has(k)) removedCount += 1;
  }
  const skipAnim = addedCount + removedCount > threshold;

  // Update after commit so the next render compares against the
  // just-rendered state. Refs in effects don't trigger re-renders.
  useEffect(() => {
    prevKeysRef.current = currentKeys;
  });

  return skipAnim;
}
