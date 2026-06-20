import { act, render } from '@testing-library/react';
import { useLayoutEffect, useState, type Dispatch, type SetStateAction } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FileListProps } from '@/components/file-list/types';

const renderCounters = vi.hoisted(() => ({
  headerBoundary: 0,
  contentBoundary: 0,
}));

vi.mock('@/components/file-list/FileListHeaderConnected', () => ({
  FileListHeaderConnected: () => {
    renderCounters.headerBoundary += 1;
    return <div data-testid="file-list-header-boundary" />;
  },
}));

vi.mock('@/components/file-list/FileListContentConnected', () => ({
  FileListContentConnected: () => {
    renderCounters.contentBoundary += 1;
    return <div data-testid="file-list-content-boundary" />;
  },
}));

import { FileList } from '@/components/FileList';

let setFileListProps: Dispatch<SetStateAction<FileListProps>> | null = null;

function Harness() {
  const [props, setProps] = useState<FileListProps>({
    ids: [1],
    total: 1,
    filterKey: 'initial',
  });
  useLayoutEffect(() => {
    setFileListProps = setProps;
    return () => {
      setFileListProps = null;
    };
  }, [setProps]);
  return <FileList {...props} />;
}

describe('FileList root render boundary', () => {
  afterEach(() => {
    setFileListProps = null;
    renderCounters.headerBoundary = 0;
    renderCounters.contentBoundary = 0;
  });

  it('does not rerender the root frame when parent props change', () => {
    render(<Harness />);

    const headerRendersAfterMount = renderCounters.headerBoundary;
    const contentRendersAfterMount = renderCounters.contentBoundary;

    act(() => {
      setFileListProps?.((prev) => ({
        ...prev,
        ids: [1, 2],
        total: 2,
      }));
    });

    expect(renderCounters.headerBoundary).toBe(headerRendersAfterMount);
    expect(renderCounters.contentBoundary).toBe(contentRendersAfterMount);
  });
});
