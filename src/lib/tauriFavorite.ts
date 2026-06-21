import { invoke } from '@tauri-apps/api/core';

export async function fileSetFavorite(
  id: number,
  isFavorite: boolean
): Promise<{ success: boolean }> {
  return invoke('file_set_favorite', { id, isFavorite });
}
