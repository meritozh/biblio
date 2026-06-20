import { createContext, useContext } from 'react';
import type { FileListController } from './types';

export const FileListControllerContext = createContext<FileListController | null>(null);

export function useFileListControllerContext(): FileListController {
  const controller = useContext(FileListControllerContext);
  if (!controller) {
    throw new Error('FileList controller missing');
  }
  return controller;
}
