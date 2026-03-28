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
import { Badge } from '@/components/ui/badge';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import {
  ChevronLeft,
  ChevronRight,
  ArrowUpDown,
  MoreVertical,
  Pencil,
  Trash2,
  FolderOpen,
  Copy,
} from 'lucide-react';
import type { FileEntry } from '@/types';
import { revealItemInDir } from '@/lib/tauri';

interface FileListProps {
  files: FileEntry[];
  onFileClick?: (file: FileEntry) => void;
  onFileEdit?: (file: FileEntry) => void;
  onFileDelete?: (file: FileEntry) => void;
}

export function FileList({ files, onFileClick, onFileEdit, onFileDelete }: FileListProps) {
  const [sorting, setSorting] = useState<{ id: string; desc: boolean }[]>([]);

  const handleCopyPath = async (path: string) => {
    try {
      await navigator.clipboard.writeText(path);
    } catch (error) {
      console.error('Failed to copy path:', error);
    }
  };

  const handleRevealInFinder = async (path: string) => {
    try {
      await revealItemInDir(path);
    } catch (error) {
      console.error('Failed to reveal in finder:', error);
    }
  };

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
        >
          Name
          <ArrowUpDown className="ml-2 h-4 w-4" aria-hidden="true" />
        </Button>
      ),
    },
    {
      accessorKey: 'created_at',
      header: 'Added',
      cell: ({ row }: { row: { original: FileEntry } }) =>
        new Date(row.original.created_at).toLocaleDateString(),
    },
    {
      id: 'tags',
      header: 'Tags',
      cell: ({ row }: { row: { original: FileEntry } }) => {
        const tags = row.original.tags;
        if (!tags || tags.length === 0) {
          return <span className="text-muted-foreground text-sm">—</span>;
        }
        const MAX_VISIBLE = 2;
        const visibleTags = tags.slice(0, MAX_VISIBLE);
        const overflowCount = tags.length - MAX_VISIBLE;
        return (
          <div className="flex items-center gap-1 max-w-[180px] flex-wrap">
            {visibleTags.map((tag) => (
              <Badge key={tag.id} variant="gray" className="text-xs font-normal shrink-0">
                {tag.name}
              </Badge>
            ))}
            {overflowCount > 0 && (
              <TooltipProvider>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Badge variant="gray" className="text-xs font-normal cursor-default">
                      +{overflowCount}
                    </Badge>
                  </TooltipTrigger>
                  <TooltipContent side="bottom" className="max-w-[200px]">
                    <div className="flex flex-wrap gap-1">
                      {tags.slice(MAX_VISIBLE).map((tag) => (
                        <Badge key={tag.id} variant="gray" className="text-xs font-normal">
                          {tag.name}
                        </Badge>
                      ))}
                    </div>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
            )}
          </div>
        );
      },
    },
    {
      id: 'authors',
      header: 'Authors',
      cell: ({ row }: { row: { original: FileEntry } }) => {
        const authors = row.original.authors;
        if (!authors || authors.length === 0) {
          return <span className="text-muted-foreground text-sm">—</span>;
        }
        const MAX_VISIBLE = 2;
        const visibleAuthors = authors.slice(0, MAX_VISIBLE);
        const overflowCount = authors.length - MAX_VISIBLE;
        return (
          <div className="max-w-[160px]">
            <span className="text-sm truncate block">
              {visibleAuthors.map((a) => a.name).join(', ')}
              {overflowCount > 0 && (
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <span className="text-muted-foreground cursor-default">
                        {' '}
                        +{overflowCount} more
                      </span>
                    </TooltipTrigger>
                    <TooltipContent side="bottom">
                      <p>
                        {authors
                          .slice(MAX_VISIBLE)
                          .map((a) => a.name)
                          .join(', ')}
                      </p>
                    </TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              )}
            </span>
          </div>
        );
      },
    },
    {
      id: 'actions',
      cell: ({ row }: { row: { original: FileEntry } }) => (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
              onClick={(e) => e.stopPropagation()}
            >
              <MoreVertical className="h-4 w-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-40">
            {onFileEdit && (
              <DropdownMenuItem onClick={() => onFileEdit(row.original)}>
                <Pencil className="h-4 w-4 mr-2" />
                Edit
              </DropdownMenuItem>
            )}
            {onFileDelete && (
              <DropdownMenuItem
                onClick={() => onFileDelete(row.original)}
                className="text-destructive focus:text-destructive"
              >
                <Trash2 className="h-4 w-4 mr-2" />
                Delete
              </DropdownMenuItem>
            )}
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => handleRevealInFinder(row.original.path)}>
              <FolderOpen className="h-4 w-4 mr-2" />
              Open in Finder
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => handleCopyPath(row.original.path)}>
              <Copy className="h-4 w-4 mr-2" />
              Copy Path
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
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
                  aria-label={onFileClick ? `View ${row.original.display_name}` : undefined}
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
