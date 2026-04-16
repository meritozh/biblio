import { useState, useEffect, useRef } from 'react';
import {
  useReactTable,
  getCoreRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  flexRender,
} from '@tanstack/react-table';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { Button } from '@/components/ui/button';
import { ChevronLeft, ChevronRight, ArrowUpDown } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { FileContextMenu } from '@/components/FileContextMenu';
import type { FileEntry } from '@/types';

interface FileListProps {
  files: FileEntry[];
  onFileClick?: (file: FileEntry) => void;
  onFileEdit?: (file: FileEntry) => void;
  onFileDelete?: (file: FileEntry) => void;
}

const MAX_VISIBLE_TAGS = 3;

// Layout & sizing constants
// ROW_HEIGHT = FileContextMenu button (h-8 = 32) + TableCell p-2 padding (16) = 48
const ROW_HEIGHT = 48;
const HEADER_HEIGHT = 40;
const ACTIONS_WIDTH = 60;
const SCROLLBAR_BUFFER = 10;
const MIN_PAGE_SIZE = 10;
const DEBOUNCE_MS = 150;
const DEFAULT_COL_SIZES: Record<string, number> = {
  display_name: 320,
  created_at: 120,
  tags: 220,
  authors: 200,
  progress: 140,
};

export function FileList({ files, onFileClick, onFileEdit, onFileDelete }: FileListProps) {
  const [sorting, setSorting] = useState<{ id: string; desc: boolean }[]>([]);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const columns = [
    {
      accessorKey: 'display_name',
      header: ({
        column,
      }: {
        column: { getIsSorted: () => string | false; toggleSorting: (desc?: boolean) => void };
      }) => (
        <Button
          variant="ghost"
          onClick={() => column.toggleSorting(column.getIsSorted() === 'asc')}
          aria-label={`Sort by name ${column.getIsSorted() === 'asc' ? 'descending' : 'ascending'}`}
          className="-ml-3"
        >
          Name
          <ArrowUpDown className="ml-2 h-4 w-4" aria-hidden="true" />
        </Button>
      ),
      size: DEFAULT_COL_SIZES.display_name,
      minSize: 150,
      maxSize: 600,
    },
    {
      accessorKey: 'created_at',
      header: 'Added',
      cell: ({ row }: { row: { original: FileEntry } }) =>
        new Date(row.original.created_at).toLocaleDateString(),
      size: DEFAULT_COL_SIZES.created_at,
      minSize: 80,
      maxSize: 200,
    },
    {
      id: 'tags',
      header: 'Tags',
      cell: ({ row }: { row: { original: FileEntry } }) => {
        const tags = row.original.tags;
        if (!tags || tags.length === 0) {
          return <span className="text-muted-foreground text-sm">—</span>;
        }
        const visible = tags.slice(0, MAX_VISIBLE_TAGS);
        const overflow = tags.length - visible.length;
        return (
          <div className="flex items-center gap-1">
            {visible.map((tag) => (
              <Badge key={tag.id} variant="gray" className="text-xs font-normal shrink-0">
                {tag.name}
              </Badge>
            ))}
            {overflow > 0 && (
              <Badge
                variant="gray"
                className="text-xs font-normal shrink-0 opacity-60"
                title={`${overflow} more`}
              >
                …
              </Badge>
            )}
          </div>
        );
      },
      size: DEFAULT_COL_SIZES.tags,
      minSize: 100,
      maxSize: 400,
    },
    {
      id: 'authors',
      header: 'Authors',
      cell: ({ row }: { row: { original: FileEntry } }) => {
        const authors = row.original.authors;
        if (!authors || authors.length === 0) {
          return <span className="text-muted-foreground text-sm">—</span>;
        }
        return <span className="text-sm">{authors.map((a) => a.name).join(', ')}</span>;
      },
      size: DEFAULT_COL_SIZES.authors,
      minSize: 80,
      maxSize: 400,
    },
    {
      id: 'progress',
      header: 'Progress',
      cell: ({ row }: { row: { original: FileEntry } }) => {
        const progress = row.original.progress;
        if (!progress) return <span className="text-muted-foreground text-sm">—</span>;
        return <span className="text-sm">{progress}</span>;
      },
      size: DEFAULT_COL_SIZES.progress,
      minSize: 80,
      maxSize: 300,
    },
    {
      id: 'actions',
      cell: ({ row }: { row: { original: FileEntry } }) => {
        const file = row.original;
        return (
          <div className="flex justify-end" role="group" aria-label="File actions">
            {onFileEdit && onFileDelete && (
              <FileContextMenu file={file} onEdit={onFileEdit} onDelete={onFileDelete} />
            )}
          </div>
        );
      },
      size: ACTIONS_WIDTH,
      minSize: ACTIONS_WIDTH,
      maxSize: ACTIONS_WIDTH,
      enableResizing: false,
    },
  ];

  const table = useReactTable({
    data: files,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    getSortedRowModel: getSortedRowModel(),
    onSortingChange: setSorting,
    state: { sorting },
    columnResizeMode: 'onChange',
    enableColumnResizing: true,
  });

  useEffect(() => {
    const el = scrollContainerRef.current;
    if (!el) return;

    let timeoutId: number | null = null;

    const applyFitSizing = (width: number, height: number) => {
      // Column widths — scale proportional to defaults, clamp to min/max
      const resizableTotal = Object.values(DEFAULT_COL_SIZES).reduce((a, b) => a + b, 0);
      const availableWidth = Math.max(0, width - ACTIONS_WIDTH - SCROLLBAR_BUFFER);
      const ratio = availableWidth / resizableTotal;

      const newSizing: Record<string, number> = { actions: ACTIONS_WIDTH };
      for (const [id, base] of Object.entries(DEFAULT_COL_SIZES)) {
        const col = table.getColumn(id);
        const min = col?.columnDef.minSize ?? 60;
        const max = col?.columnDef.maxSize ?? 600;
        newSizing[id] = Math.min(max, Math.max(min, Math.round(base * ratio)));
      }
      table.setColumnSizing(newSizing);

      // Page size — max(MIN_PAGE_SIZE, floor(rowsArea / ROW_HEIGHT))
      const rowsArea = Math.max(0, height - HEADER_HEIGHT);
      const pageSize = Math.max(MIN_PAGE_SIZE, Math.floor(rowsArea / ROW_HEIGHT));
      table.setPageSize(pageSize);
    };

    const recompute = () => {
      applyFitSizing(el.clientWidth, el.clientHeight);
    };

    // Initial: run immediately (no debounce) so first render is correct
    recompute();

    // Subsequent resize events: debounced
    const observer = new ResizeObserver(() => {
      if (timeoutId !== null) window.clearTimeout(timeoutId);
      timeoutId = window.setTimeout(recompute, DEBOUNCE_MS);
    });
    observer.observe(el);

    return () => {
      if (timeoutId !== null) window.clearTimeout(timeoutId);
      observer.disconnect();
    };
  }, [table]);

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div ref={scrollContainerRef} className="flex-1 min-h-0 overflow-auto rounded-md border">
        <Table
          className="table-fixed w-auto"
          style={{ width: table.getTotalSize() }}
        >
          <TableHeader>
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow key={headerGroup.id}>
                {headerGroup.headers.map((header) => (
                  <TableHead
                    key={header.id}
                    style={{ width: header.getSize() }}
                    className="relative overflow-hidden whitespace-nowrap"
                  >
                    {header.isPlaceholder
                      ? null
                      : flexRender(header.column.columnDef.header, header.getContext())}
                    {header.column.getCanResize() && (
                      <div
                        onMouseDown={(e) => {
                          e.stopPropagation();
                          header.getResizeHandler()(e);
                        }}
                        onTouchStart={(e) => {
                          e.stopPropagation();
                          header.getResizeHandler()(e);
                        }}
                        onClick={(e) => e.stopPropagation()}
                        className={`absolute top-0 right-0 h-full w-1 cursor-col-resize select-none touch-none hover:bg-primary/40 ${
                          header.column.getIsResizing() ? 'bg-primary' : ''
                        }`}
                        aria-label="Resize column"
                      />
                    )}
                  </TableHead>
                ))}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {table.getRowModel().rows?.length ? (
              table.getRowModel().rows.map((row) => (
                <TableRow
                  key={row.id}
                  data-state={row.getIsSelected() && 'selected'}
                  className={onFileClick ? 'cursor-pointer hover:bg-muted/50' : ''}
                  onClick={() => onFileClick?.(row.original)}
                  tabIndex={onFileClick ? 0 : undefined}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && onFileClick) {
                      onFileClick(row.original);
                    }
                  }}
                  role={onFileClick ? 'button' : undefined}
                  aria-label={onFileClick ? `View ${row.original.display_name}` : undefined}
                >
                  {row.getVisibleCells().map((cell) => (
                    <TableCell
                      key={cell.id}
                      style={{ width: cell.column.getSize() }}
                      className="overflow-hidden whitespace-nowrap truncate"
                    >
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </TableCell>
                  ))}
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell colSpan={columns.length} className="h-24 text-center">
                  No files in library.
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>
      <div
        className="flex items-center justify-end space-x-2 mt-4 shrink-0"
        role="navigation"
        aria-label="Table pagination"
      >
        <Button
          variant="outline"
          size="sm"
          onClick={() => table.previousPage()}
          disabled={!table.getCanPreviousPage()}
          aria-label="Previous page"
        >
          <ChevronLeft className="h-4 w-4" aria-hidden="true" />
          Previous
        </Button>
        <span aria-live="polite" className="text-sm text-muted-foreground">
          Page {table.getState().pagination.pageIndex + 1} of {table.getPageCount()}
        </span>
        <Button
          variant="outline"
          size="sm"
          onClick={() => table.nextPage()}
          disabled={!table.getCanNextPage()}
          aria-label="Next page"
        >
          Next
          <ChevronRight className="h-4 w-4" aria-hidden="true" />
        </Button>
      </div>
    </div>
  );
}
