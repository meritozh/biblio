import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Button } from '@/components/ui/button';
import {
  Copy,
  Download,
  FolderOpen,
  MoreHorizontal,
  Pencil,
  Play,
  Star,
  StarOff,
  Trash2,
  Upload,
  XCircle,
} from 'lucide-react';
import { cacheClear, cacheOpen, fileSetFavorite, revealItemInDir } from '@/lib/tauri';
import { patchFile } from '@/stores/fileStore';
import {
  enqueueUpload,
  useRemoteUploadStore,
} from '@/stores/remoteUploadStore';
import {
  enqueueDownload,
  useRemoteDownloadStore,
} from '@/stores/remoteDownloadStore';
import type { FileEntry } from '@/types';

interface FileContextMenuProps {
  file: FileEntry;
  onEdit: (file: FileEntry) => void;
  onDelete: (file: FileEntry) => void;
  /** Whether cloud storage is configured. Gates Upload / Download so
   *  actions that would fail authentication aren't shown. Defaults to
   *  false — when omitted, only local-only items appear. */
  remoteEnabled?: boolean;
}

export function FileContextMenu({
  file,
  onEdit,
  onDelete,
  remoteEnabled = false,
}: FileContextMenuProps) {
  const isRemote = file.storage_kind === 'remote';
  const hasCache = !!file.local_cache_path;
  // A file has a local copy when it's a local-storage row OR a remote
  // row whose download worker has populated the cache. The Open and
  // Show-in-Finder items key off this.
  const hasLocalCopy = !isRemote || hasCache;

  // Per-file in-flight state so the menu disables a queued action instead
  // of re-enqueuing it on a second click. Mirrors the bulk bar's guard.
  const uploadState = useRemoteUploadStore();
  const downloadState = useRemoteDownloadStore();
  const isUploading = uploadState.uploads.some(
    (u) =>
      u.file_id === file.id &&
      (u.status === 'pending' || u.status === 'uploading')
  );
  const isDownloading = downloadState.downloads.some(
    (d) =>
      d.file_id === file.id &&
      (d.status === 'pending' || d.status === 'downloading')
  );

  // Storage actions keyed on the row's storage state. Cloud actions
  // require remote configured; clearing a local cache copy is always
  // safe (the remote original stays).
  const canUpload = !isRemote && remoteEnabled;
  const canDownload = isRemote && !hasCache && remoteEnabled;
  const canClearCache = isRemote && hasCache;
  const showStorageSection = canUpload || canDownload || canClearCache;

  const handleUpload = async () => {
    // The store enqueue takes a fileId list + a name map (for the
    // progress panel's row labels) and handles backend errors internally
    // by marking the row errored — so no toast here, the panel surfaces it.
    await enqueueUpload([file.id], new Map([[file.id, file.display_name]]));
  };

  const handleDownload = async () => {
    await enqueueDownload([file.id], new Map([[file.id, file.display_name]]));
  };

  const handleOpen = async () => {
    try {
      await cacheOpen(file.id);
    } catch (error) {
      console.error('Failed to open file:', error);
      alert(`Failed to open: ${error}`);
    }
  };

  const handleRevealInFinder = async () => {
    // Local rows use file.path directly; remote-with-cache routes to
    // the cache path. Remote-without-cache hides this item, so the
    // branch below is exhaustive.
    const target = isRemote ? file.local_cache_path : file.path;
    if (!target) return;
    try {
      await revealItemInDir(target);
    } catch (error) {
      console.error('Failed to reveal file:', error);
    }
  };

  const handleCopyPath = async () => {
    try {
      await navigator.clipboard.writeText(file.path);
    } catch (error) {
      console.error('Failed to copy path:', error);
    }
  };

  const handleClearCache = async () => {
    const ok = window.confirm(
      'Delete the local cache copy of this file?\n\n' +
        'The remote copy stays on Baidu Pan; you can re-download anytime.'
    );
    if (!ok) return;
    try {
      await cacheClear(file.id);
      // Patch the row locally so the cache badge updates without a refetch.
      patchFile(file.id, { local_cache_path: null });
    } catch (error) {
      console.error('Failed to clear cache:', error);
      alert(`Failed to clear cache: ${error}`);
    }
  };

  const handleToggleFavorite = async () => {
    const next = !file.is_favorite;
    try {
      await fileSetFavorite(file.id, next);
      patchFile(file.id, { is_favorite: next });
    } catch (error) {
      console.error('Failed to update favorite:', error);
      alert(`Failed to update favorite: ${error}`);
    }
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          aria-label={`Actions for ${file.display_name}`}
          onClick={(e) => {
            e.stopPropagation();
          }}
        >
          <MoreHorizontal className="h-4 w-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {hasLocalCopy && (
          <DropdownMenuItem onClick={handleOpen}>
            <Play className="h-4 w-4 mr-2" />
            Open
          </DropdownMenuItem>
        )}
        <DropdownMenuItem onClick={() => onEdit(file)}>
          <Pencil className="h-4 w-4 mr-2" />
          Edit
        </DropdownMenuItem>
        <DropdownMenuItem onClick={handleToggleFavorite}>
          {file.is_favorite ? (
            <StarOff className="h-4 w-4 mr-2" />
          ) : (
            <Star className="h-4 w-4 mr-2" />
          )}
          {file.is_favorite ? 'Remove favorite' : 'Add favorite'}
        </DropdownMenuItem>
        <DropdownMenuItem
          onClick={() => onDelete(file)}
          className="text-destructive focus:text-destructive"
        >
          <Trash2 className="h-4 w-4 mr-2" />
          Delete
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        {hasLocalCopy && (
          <DropdownMenuItem onClick={handleRevealInFinder}>
            <FolderOpen className="h-4 w-4 mr-2" />
            Show in Finder
          </DropdownMenuItem>
        )}
        <DropdownMenuItem onClick={handleCopyPath}>
          <Copy className="h-4 w-4 mr-2" />
          Copy Path
        </DropdownMenuItem>
        {showStorageSection && <DropdownMenuSeparator />}
        {canUpload && (
          <DropdownMenuItem onClick={handleUpload} disabled={isUploading}>
            <Upload className="h-4 w-4 mr-2" />
            {isUploading ? 'Uploading…' : 'Upload to cloud'}
          </DropdownMenuItem>
        )}
        {canDownload && (
          <DropdownMenuItem onClick={handleDownload} disabled={isDownloading}>
            <Download className="h-4 w-4 mr-2" />
            {isDownloading ? 'Downloading…' : 'Download'}
          </DropdownMenuItem>
        )}
        {canClearCache && (
          <DropdownMenuItem onClick={handleClearCache}>
            <XCircle className="h-4 w-4 mr-2" />
            Clear cache
          </DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
