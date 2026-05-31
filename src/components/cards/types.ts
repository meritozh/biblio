import type { FileEntry } from '@/types';

export interface FileCardProps {
  id: number;
  isSelected: boolean;
  isUploading: boolean;
  blocked: boolean;
  selectionMode: boolean;
  onCardClick: (file: FileEntry) => void;
  onToggleSelect: (id: number) => void;
  onEdit?: (file: FileEntry) => void;
  onDelete?: (file: FileEntry) => void;
  /** Whether cloud storage is configured. Forwarded to the row's context
   *  menu to gate its Upload / Download items. */
  remoteEnabled?: boolean;
}
