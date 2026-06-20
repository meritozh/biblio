import { memo, useLayoutEffect, useState } from 'react';
import { createFileListController, normalizeProps } from '@/components/file-list/controller';
import { FileListControllerContext } from '@/components/file-list/context';
import { FileListContentConnected } from '@/components/file-list/FileListContentConnected';
import { FileListHeaderConnected } from '@/components/file-list/FileListHeaderConnected';
import type { FileListController, FileListProps } from '@/components/file-list/types';

/** Compatibility shell for the library file list. It owns a stable scoped
 *  controller and renders only the independently subscribed header/content
 *  boundaries; row data, worker queues, selection, and filter/sort derivation
 *  no longer subscribe at this level. */
export function FileList(props: FileListProps) {
  const normalized = normalizeProps(props);
  const [controller] = useState(() => createFileListController(normalized));

  useLayoutEffect(() => {
    controller.updateInput(normalized);
  });

  return <FileListRoot controller={controller} />;
}

const FileListRoot = memo(function FileListRoot({
  controller,
}: {
  controller: FileListController;
}) {
  return (
    <FileListControllerContext.Provider value={controller}>
      <div className="flex-1 flex flex-col min-h-0">
        <FileListHeaderConnected />
        <FileListContentConnected />
      </div>
    </FileListControllerContext.Provider>
  );
});
