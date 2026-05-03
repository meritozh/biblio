import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import {
  useReactTable,
  getCoreRowModel,
  getSortedRowModel,
  flexRender,
  type VisibilityState,
  type SortingState,
} from '@tanstack/react-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Button } from '@/components/ui/button';
import { ArrowUpDown, Loader2, BookOpen } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { FileContextMenu } from '@/components/FileContextMenu';
import { coverGet } from '@/lib/tauri';
import { schemaForPath, isImportable, KIND_REGISTRY } from '@/lib/fileKind';
import type { FileEntry } from '@/types';

// ── CoverCell ─────────────────────────────────────────────────────────────────

function CoverCell({ fileId }: { fileId: number }) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    coverGet(fileId)
      .then(({ data, mime_type }) => setSrc(`data:${mime_type};base64,${data}`))
      .catch(() => {});
  }, [fileId]);

  return src ? (
    <img src={src} alt="Cover" className="h-8 w-6 object-cover rounded-sm shrink-0" />
  ) : (
    <div className="flex items-center justify-center h-8 w-6">
      <BookOpen className="h-3.5 w-3.5 text-muted-foreground/40" />
    </div>
  );
}

/** Larger cover variant for the grid card. Fills the parent (which carries
 *  the aspect-ratio constraint). Same lazy-fetch contract as `CoverCell`. */
function CardCover({ fileId }: { fileId: number }) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    coverGet(fileId)
      .then(({ data, mime_type }) => setSrc(`data:${mime_type};base64,${data}`))
      .catch(() => {});
  }, [fileId]);

  return src ? (
    <img src={src} alt="Cover" className="h-full w-full object-cover" />
  ) : (
    <BookOpen className="h-8 w-8 text-muted-foreground/40" />
  );
}

// ── Constants ─────────────────────────────────────────────────────────────────

const ROW_HEIGHT = 48;
const ACTIONS_WIDTH = 52;
const COVER_WIDTH = 52;
const SCROLLBAR_BUFFER = 10;
const OVERSCAN = 10;
const LOAD_MORE_THRESHOLD = 5;
const DEBOUNCE_MS = 150;
const MAX_VISIBLE_TAGS = 3;

const DEFAULT_COL_SIZES: Record<string, number> = {
  display_name: 300,
  description: 280,
  tags: 200,
  authors: 180,
  progress: 120,
};

// Grid layout sizing — comic emphasis, ~4–6 columns at typical widths.
const CARD_WIDTH = 180;
const CARD_HEIGHT = 280;
const GRID_GAP = 16;
const GRID_PAD = 4; // matches scroll container's interior padding

// ── Props ─────────────────────────────────────────────────────────────────────

interface FileListProps {
  files: FileEntry[];
  total?: number;
  loadingMore?: boolean;
  onLoadMore?: () => void;
  filterKey?: string | number | null;
  onFileClick?: (file: FileEntry) => void;
  onFileEdit?: (file: FileEntry) => void;
  onFileDelete?: (file: FileEntry) => void;
}

// ── FileList ──────────────────────────────────────────────────────────────────

export function FileList({
  files: rawFiles,
  total,
  loadingMore = false,
  onLoadMore,
  filterKey = null,
  onFileClick,
  onFileEdit,
  onFileDelete,
}: FileListProps) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [containerWidth, setContainerWidth] = useState(0);

  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const prevFilterKeyRef = useRef(filterKey);

  // Drop legacy rows whose extension is no longer in the supported set
  // (.epub / .pdf / standalone images from the pre-removal era). Their DB
  // rows stay intact; we just don't surface them in the list.
  const files = useMemo(
    () => rawFiles.filter((f) => isImportable(f.path)),
    [rawFiles]
  );

  // Scroll to top when the filter (category / search) changes
  useEffect(() => {
    if (prevFilterKeyRef.current === filterKey) return;
    prevFilterKeyRef.current = filterKey;
    scrollContainerRef.current?.scrollTo(0, 0);
  }, [filterKey]);

  // Schema picks the layout (table vs grid) and column visibility from the
  // first file's kind. Categories are typically homogeneous, so the first
  // file determines the rendering for the whole list. Falls back to the
  // novel schema when the list is empty.
  const schema = useMemo(
    () => schemaForPath(files[0]?.path) ?? KIND_REGISTRY.novel,
    [files[0]?.path] // eslint-disable-line react-hooks/exhaustive-deps
  );
  const layout = schema.layout;
  const columnVisibility = useMemo<VisibilityState>(
    () => ({ ...schema.columns }),
    [schema]
  );

  const columns = useMemo(
    () => [
      {
        id: 'cover',
        header: 'Cover',
        enableHiding: true,
        enableResizing: false,
        enableSorting: false,
        cell: ({ row }: { row: { original: FileEntry } }) => (
          <CoverCell fileId={row.original.id} />
        ),
        size: COVER_WIDTH,
        minSize: COVER_WIDTH,
        maxSize: COVER_WIDTH,
      },
      {
        accessorKey: 'display_name',
        enableHiding: false,
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
        id: 'description',
        header: 'Description',
        enableHiding: true,
        cell: ({ row }: { row: { original: FileEntry } }) => {
          const d = row.original.description;
          return d ? (
            <span className="text-sm block truncate" title={d}>
              {d}
            </span>
          ) : (
            <span className="text-muted-foreground text-sm">—</span>
          );
        },
        size: DEFAULT_COL_SIZES.description,
        minSize: 120,
        maxSize: 600,
      },
      {
        id: 'tags',
        header: 'Tags',
        enableHiding: true,
        cell: ({ row }: { row: { original: FileEntry } }) => {
          const tags = row.original.tags;
          if (!tags?.length) return <span className="text-muted-foreground text-sm">—</span>;
          return (
            <div className="flex items-center gap-1">
              {tags.slice(0, MAX_VISIBLE_TAGS).map((tag) => (
                <Badge key={tag.id} variant="gray" className="text-xs font-normal shrink-0">
                  {tag.name}
                </Badge>
              ))}
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
        enableHiding: true,
        cell: ({ row }: { row: { original: FileEntry } }) => {
          const authors = row.original.authors;
          if (!authors?.length) return <span className="text-muted-foreground text-sm">—</span>;
          return <span className="text-sm">{authors.map((a) => a.name).join(', ')}</span>;
        },
        size: DEFAULT_COL_SIZES.authors,
        minSize: 80,
        maxSize: 400,
      },
      {
        id: 'progress',
        header: 'Progress',
        enableHiding: true,
        cell: ({ row }: { row: { original: FileEntry } }) => {
          const p = row.original.progress;
          return p ? (
            <span className="text-sm">{p}</span>
          ) : (
            <span className="text-muted-foreground text-sm">—</span>
          );
        },
        size: DEFAULT_COL_SIZES.progress,
        minSize: 80,
        maxSize: 300,
      },
      {
        id: 'actions',
        enableHiding: false,
        enableResizing: false,
        enableSorting: false,
        cell: ({ row }: { row: { original: FileEntry } }) => (
          <div className="flex justify-end" role="group" aria-label="File actions">
            {onFileEdit && onFileDelete && (
              <FileContextMenu
                file={row.original}
                onEdit={onFileEdit}
                onDelete={onFileDelete}
              />
            )}
          </div>
        ),
        size: ACTIONS_WIDTH,
        minSize: ACTIONS_WIDTH,
        maxSize: ACTIONS_WIDTH,
      },
    ],
    [onFileEdit, onFileDelete]
  );

  const table = useReactTable({
    data: files,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    onSortingChange: setSorting,
    state: { sorting, columnVisibility },
    columnResizeMode: 'onChange',
    enableColumnResizing: true,
  });

  // Keep a ref so ResizeObserver callback always reads latest table state
  const tableRef = useRef(table);
  tableRef.current = table;

  const rows = table.getRowModel().rows;

  // ── Virtualizers (both always created so hook order stays stable) ────────
  // Table mode: one virtual item per row. Grid mode: one per grid row, each
  // holding `colCount` cards.
  const colCount = Math.max(
    1,
    Math.floor((containerWidth - GRID_PAD * 2 + GRID_GAP) / (CARD_WIDTH + GRID_GAP))
  );
  const gridRowCount = Math.ceil(rows.length / colCount);

  const tableVirtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: OVERSCAN,
  });
  const gridVirtualizer = useVirtualizer({
    count: gridRowCount,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: () => CARD_HEIGHT + GRID_GAP,
    overscan: 4,
  });
  const activeVirtualizer = layout === 'grid' ? gridVirtualizer : tableVirtualizer;
  const virtualItems = activeVirtualizer.getVirtualItems();
  const totalVirtualSize = activeVirtualizer.getTotalSize();

  // Trigger server-side load-more when the last virtual element is near the
  // end. In table mode "end" = last row; in grid mode "end" = last grid row.
  useEffect(() => {
    const lastItem = virtualItems[virtualItems.length - 1];
    if (!lastItem || loadingMore || files.length >= (total ?? 0)) return;
    const totalCount = layout === 'grid' ? gridRowCount : rows.length;
    if (lastItem.index >= totalCount - 1 - LOAD_MORE_THRESHOLD) {
      onLoadMore?.();
    }
  }, [virtualItems, gridRowCount, rows.length, files.length, total, loadingMore, onLoadMore, layout]);

  // Column auto-sizing for table mode. Grid mode just needs containerWidth
  // for column-count math; the ResizeObserver below feeds both.
  const applyFitSizing = useCallback((width: number) => {
    const t = tableRef.current;
    const coverVisible = t.getColumn('cover')?.getIsVisible() ?? false;
    const coverW = coverVisible ? COVER_WIDTH : 0;
    const visibleResizable = Object.entries(DEFAULT_COL_SIZES).filter(
      ([id]) => t.getColumn(id)?.getIsVisible() !== false
    );
    const resizableTotal = visibleResizable.reduce((a, [, b]) => a + b, 0);
    const available = Math.max(0, width - ACTIONS_WIDTH - coverW - SCROLLBAR_BUFFER);
    const ratio = resizableTotal > 0 ? available / resizableTotal : 1;

    const newSizing: Record<string, number> = { actions: ACTIONS_WIDTH, cover: COVER_WIDTH };
    for (const [id, base] of Object.entries(DEFAULT_COL_SIZES)) {
      const col = t.getColumn(id);
      if (!col) continue;
      newSizing[id] = Math.min(
        col.columnDef.maxSize ?? 600,
        Math.max(col.columnDef.minSize ?? 60, Math.round(base * ratio))
      );
    }
    t.setColumnSizing(newSizing);
  }, []);

  useEffect(() => {
    const el = scrollContainerRef.current;
    if (!el) return;
    let timeoutId: number | null = null;
    const recompute = () => {
      const w = el.clientWidth;
      setContainerWidth(w);
      applyFitSizing(w);
    };
    recompute();
    const observer = new ResizeObserver(() => {
      if (timeoutId !== null) window.clearTimeout(timeoutId);
      timeoutId = window.setTimeout(recompute, DEBOUNCE_MS);
    });
    observer.observe(el);
    return () => {
      if (timeoutId !== null) window.clearTimeout(timeoutId);
      observer.disconnect();
    };
  }, [applyFitSizing]);

  useEffect(() => {
    const el = scrollContainerRef.current;
    if (el) applyFitSizing(el.clientWidth);
  }, [columnVisibility, applyFitSizing]); // re-measure when file type changes

  // Sort control above the grid — the table mode sorts via the column
  // header; cards have no header so it lives in a small toolbar.
  const nameSort = sorting.find((s) => s.id === 'display_name');
  const toggleNameSort = () => {
    table.getColumn('display_name')?.toggleSorting(nameSort?.desc !== true);
  };

  return (
    <div className="flex-1 flex flex-col min-h-0">
      {layout === 'grid' && (
        <div className="flex items-center justify-end gap-2 pb-2 shrink-0">
          <button
            type="button"
            onClick={toggleNameSort}
            className="inline-flex items-center gap-1.5 text-xs px-2.5 py-1 rounded-full border bg-background hover:bg-secondary/60 transition-colors"
            aria-label="Toggle name sort"
          >
            Name
            <ArrowUpDown className="h-3 w-3" aria-hidden="true" />
            {nameSort && (
              <span className="text-muted-foreground">
                {nameSort.desc ? 'Z–A' : 'A–Z'}
              </span>
            )}
          </button>
        </div>
      )}

      <div
        ref={scrollContainerRef}
        className={`flex-1 min-h-0 overflow-auto ${
          layout === 'grid' ? '' : 'rounded-md border'
        }`}
      >
        {layout === 'table' ? (
          // ── Table mode ───────────────────────────────────────────────
          // display:grid on table/thead/tbody lets tbody use position:
          // relative + height for the scroll area, and thead use position:
          // sticky without breaking table layout.
          <table style={{ display: 'grid', width: table.getTotalSize() }}>
            <thead
              style={{ display: 'grid', position: 'sticky', top: 0, zIndex: 1 }}
              className="bg-background border-b"
            >
              {table.getHeaderGroups().map((headerGroup) => (
                <tr key={headerGroup.id} style={{ display: 'flex', width: '100%' }}>
                  {headerGroup.headers.map((header) => (
                    <th
                      key={header.id}
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        width: header.getSize(),
                        position: 'relative',
                        overflow: 'hidden',
                      }}
                      className="px-3 h-10 text-left text-xs font-medium text-muted-foreground"
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
                    </th>
                  ))}
                </tr>
              ))}
            </thead>
            <tbody
              style={{
                display: 'grid',
                height: totalVirtualSize,
                position: 'relative',
              }}
            >
              {virtualItems.map((virtualRow) => {
                const row = rows[virtualRow.index];
                if (!row) return null;
                return (
                  <tr
                    key={row.id}
                    style={{
                      display: 'flex',
                      position: 'absolute',
                      transform: `translateY(${virtualRow.start}px)`,
                      width: '100%',
                      height: ROW_HEIGHT,
                      alignItems: 'center',
                    }}
                    className={`border-b border-border/50 ${
                      onFileClick ? 'cursor-pointer hover:bg-muted/50' : ''
                    }`}
                    onClick={() => onFileClick?.(row.original)}
                    tabIndex={onFileClick ? 0 : undefined}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && onFileClick) onFileClick(row.original);
                    }}
                    role={onFileClick ? 'button' : undefined}
                    aria-label={
                      onFileClick ? `View ${row.original.display_name}` : undefined
                    }
                  >
                    {row.getVisibleCells().map((cell) => (
                      <td
                        key={cell.id}
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          width: cell.column.getSize(),
                          overflow: 'hidden',
                        }}
                        className="px-3 whitespace-nowrap"
                      >
                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                      </td>
                    ))}
                  </tr>
                );
              })}
            </tbody>
          </table>
        ) : (
          // ── Grid mode ────────────────────────────────────────────────
          // One virtual item per "grid row" (a horizontal strip of N cards).
          // Each grid row absolutely positioned within a relative tbody-
          // equivalent so we can reuse the same scroll container + virtualizer
          // pattern as table mode.
          <div
            style={{
              height: totalVirtualSize,
              position: 'relative',
              padding: GRID_PAD,
            }}
          >
            {virtualItems.map((virtualRow) => {
              const startIdx = virtualRow.index * colCount;
              const endIdx = Math.min(startIdx + colCount, rows.length);
              const slice = rows.slice(startIdx, endIdx);
              return (
                <div
                  key={virtualRow.index}
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    transform: `translateY(${virtualRow.start}px)`,
                    width: '100%',
                    paddingLeft: GRID_PAD,
                    paddingRight: GRID_PAD,
                    display: 'grid',
                    gridTemplateColumns: `repeat(${colCount}, ${CARD_WIDTH}px)`,
                    gap: GRID_GAP,
                    justifyContent: 'start',
                  }}
                >
                  {slice.map((row) => {
                    const file = row.original;
                    return (
                      <div
                        key={row.id}
                        className="relative group"
                        style={{ width: CARD_WIDTH, height: CARD_HEIGHT }}
                      >
                        <button
                          type="button"
                          onClick={() => onFileClick?.(file)}
                          className="w-full h-full flex flex-col gap-2 text-left rounded-lg p-2 hover:bg-muted/50 transition-colors focus:outline-none focus:ring-2 focus:ring-ring"
                          aria-label={`View ${file.display_name}`}
                        >
                          <div className="aspect-[2/3] w-full rounded-md overflow-hidden bg-secondary/40 border flex items-center justify-center">
                            <CardCover fileId={file.id} />
                          </div>
                          <div className="space-y-0.5 min-w-0 px-0.5">
                            <p
                              className="text-sm font-medium leading-tight truncate"
                              title={file.display_name}
                            >
                              {file.display_name}
                            </p>
                            {file.authors && file.authors.length > 0 && (
                              <p className="text-xs text-muted-foreground line-clamp-1">
                                {file.authors.map((a) => a.name).join(', ')}
                              </p>
                            )}
                          </div>
                        </button>
                        {onFileEdit && onFileDelete && (
                          <div className="absolute top-3 right-3 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity">
                            <FileContextMenu
                              file={file}
                              onEdit={onFileEdit}
                              onDelete={onFileDelete}
                            />
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              );
            })}
          </div>
        )}

        {files.length === 0 && (
          <div className="flex items-center justify-center h-24 text-sm text-muted-foreground">
            No files in library.
          </div>
        )}

        {loadingMore && (
          <div
            className="flex items-center justify-center py-3 gap-2 text-xs text-muted-foreground font-serif-italic"
            aria-live="polite"
          >
            <Loader2 className="h-3 w-3 animate-spin" aria-hidden="true" />
            {total != null ? `loading ${total - files.length} more…` : 'loading more…'}
          </div>
        )}
      </div>
    </div>
  );
}
