/**
 * IPC bridge for bulk author operations used by the cleanup page's
 * author tools. Lives outside `tauri.ts` because that file is at the
 * repo's 800-line guard (tests/unit/sourceFileSize.test.ts) — new
 * bridge domains get their own file (see also `schemaBridge.ts`).
 */

import { invoke } from '@tauri-apps/api/core';

/** Merge author `fromId` into `toId`: all files are relinked to the
 *  target, the source row is deleted. `moved` counts files newly linked
 *  to the target (files already linked to both keep their link). */
export async function authorMerge(
  fromId: number,
  toId: number
): Promise<{ moved: number }> {
  return invoke('author_merge', { fromId, toId });
}
