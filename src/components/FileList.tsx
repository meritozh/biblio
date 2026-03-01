import { useState } from 'react';
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
import type { FileEntry } from '@/types';

interface FileListProps {
  files: FileEntry[];
  onFileClick?: (file: FileEntry) => void;
  onFileEdit?: (file: FileEntry) => void;
  onFileDelete?: (file: FileEntry) => void;
}

export function FileList({ files, onFileClick, onFileEdit, onFileDelete }: FileListProps) {
  const [sorting, setSorting] = useState<{ id: string; desc: boolean }[]>([]);

  const columns = [
    {
      accessorKey: 'displayName',
      header: ({
        column,
      }: {
        column: { getIsSorted: () => string | false; toggleSorting: (desc?: boolean) => void };
      }) => (
        <Button
          variant="ghost"
          onClick={() => column.toggleSorting(column.getIsSorted() === 'asc')}
          aria-label={`Sort by name ${column.getIsSorted() === 'asc' ? 'descending' : 'ascending'}`}
        >
          Name
          <ArrowUpDown className="ml-2 h-4 w-4" aria-hidden="true" />
        </Button>
      ),
    },
    {
      accessorKey: 'path',
      header: 'Path',
      cell: ({ row }: { row: { original: FileEntry } }) => (
        <span className="text-muted-foreground truncate max-w-md block">{row.original.path}</span>
      ),
    },
    {
      accessorKey: 'fileStatus',
      header: 'Status',
      cell: ({ row }: { row: { original: FileEntry } }) => {
        const status = row.original.fileStatus;
        const colors = {
          available: 'text-green-600',
          missing: 'text-red-600',
          moved: 'text-yellow-600',
        };
        return <span className={colors[status as keyof typeof colors] || ''}>{status}</span>;
      },
    },
    {
      accessorKey: 'createdAt',
      header: 'Added',
      cell: ({ row }: { row: { original: FileEntry } }) =>
        new Date(row.original.createdAt).toLocaleDateString(),
    },
    {
      id: 'actions',
      cell: ({ row }: { row: { original: FileEntry } }) => (
        <div className="flex gap-2" role="group" aria-label="File actions">
          {onFileEdit && (
            <Button
              variant="outline"
              size="sm"
              onClick={() => onFileEdit(row.original)}
              aria-label={`Edit ${row.original.displayName}`}
            >
              Edit
            </Button>
          )}
          {onFileDelete && (
            <Button
              variant="destructive"
              size="sm"
              onClick={() => onFileDelete(row.original)}
              aria-label={`Delete ${row.original.displayName}`}
            >
              Delete
            </Button>
          )}
        </div>
      ),
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
  });

  return (
    <div className="space-y-4">
      <div className="rounded-md border">
        <Table>
          <TableHeader>
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow key={headerGroup.id}>
                {headerGroup.headers.map((header) => (
                  <TableHead key={header.id}>
                    {header.isPlaceholder
                      ? null
                      : flexRender(header.column.columnDef.header, header.getContext())}
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
                  aria-label={onFileClick ? `View ${row.original.displayName}` : undefined}
                >
                  {row.getVisibleCells().map((cell) => (
                    <TableCell key={cell.id}>
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
        className="flex items-center justify-end space-x-2"
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
