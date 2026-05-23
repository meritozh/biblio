import { Button } from '@/components/ui/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  ArrowDown,
  ArrowUp,
  ChevronDown,
  Eraser,
  Filter as FilterIcon,
  Layers,
  Trash2,
  X,
} from 'lucide-react';
import { FilterEditor } from '@/components/FilterEditor';
import { describeCondition, type Condition } from '@/lib/filters';
import type { SortKey } from '@/stores';
import type { Author, ComicViewMode, Tag } from '@/types';

export type { SortKey };

export const SORT_OPTIONS: ReadonlyArray<{ value: SortKey; label: string }> = [
  { value: 'name', label: 'Name' },
  { value: 'created', label: 'Date added' },
  { value: 'updated', label: 'Last updated' },
];

export const VIEW_MODE_OPTIONS: ReadonlyArray<{ value: ComicViewMode; label: string }> = [
  { value: 'flat', label: 'All comics' },
  { value: 'author', label: 'By author' },
  { value: 'name_prefix', label: 'By series' },
];

interface ViewModeControls {
  viewMode: ComicViewMode;
  onViewModeChange?: (mode: ComicViewMode) => void;
  available: boolean;
}

interface SortControls {
  sortBy: SortKey;
  sortDesc: boolean;
  setSortBy: (key: SortKey) => void;
  setSortDesc: (desc: boolean) => void;
}

interface FilterControls {
  conditions: Condition[];
  setConditions: React.Dispatch<React.SetStateAction<Condition[]>>;
  filterOpen: boolean;
  setFilterOpen: (open: boolean) => void;
  removeCondition: (id: string) => void;
  availableTags: ReadonlyArray<Tag>;
  availableAuthors: ReadonlyArray<Author>;
  tagsById: Map<number, Tag>;
  authorsById: Map<number, Author>;
}

interface SelectionControls {
  selectionMode: boolean;
  selectedCount: number;
  visibleCount: number;
  enterSelectionMode: () => void;
  exitSelectionMode: () => void;
  clearSelection: () => void;
  selectFirstN: (n: number) => void;
}

interface BulkActions {
  remoteEnabled: boolean;
  canDownload: boolean;
  canDelete: boolean;
  /** Capability flag — clearing local cache is available when the handler
   *  is wired. The button is shown disabled when no selected file
   *  currently has a cache (so it doesn't look interactive on a no-op). */
  canClearCache: boolean;
  /** True iff at least one selected file has a non-empty `local_cache_path`.
   *  Driven by the parent's row-by-row inspection so the button accurately
   *  reflects whether the click would do anything. */
  hasCacheableSelection: boolean;
  onUpload: () => void;
  onDownload: () => void;
  onDelete: () => void;
  onClearCache: () => void;
}

interface FileListHeaderProps {
  /** When true, sort / filter / select controls are hidden because the
   *  body is rendering ComicCollection cards. They reappear after
   *  drill-down. */
  showCollections: boolean;
  view: ViewModeControls;
  sort: SortControls;
  filter: FilterControls;
  selection: SelectionControls;
  bulk: BulkActions;
}

/** Header bar above the file grid. Two modes:
 *  - normal: view / sort / filter chips + Select button
 *  - selection: count, Select-first-N dropdown, bulk actions, Cancel
 *
 *  The mode is driven by `selection.selectionMode` so the orchestrator
 *  can reset it on filter-key changes. */
export function FileListHeader({
  showCollections,
  view,
  sort,
  filter,
  selection,
  bulk,
}: FileListHeaderProps) {
  if (selection.selectionMode) {
    return (
      <div className="flex items-center gap-3 pb-3 shrink-0">
        <span className="text-sm font-medium text-foreground">
          {selection.selectedCount === 0
            ? 'Select files'
            : `${selection.selectedCount} file${selection.selectedCount === 1 ? '' : 's'} selected`}
        </span>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm" className="h-8 text-xs gap-1.5">
              Select first
              <ChevronDown className="h-3 w-3" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start">
            {[10, 25, 50, 100].map((n) => (
              <DropdownMenuItem
                key={n}
                className="text-xs"
                onClick={() => selection.selectFirstN(n)}
              >
                First {n}
              </DropdownMenuItem>
            ))}
            <DropdownMenuSeparator />
            <DropdownMenuItem
              className="text-xs"
              onClick={() => selection.selectFirstN(selection.visibleCount)}
            >
              All eligible
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <div className="flex-1" />
        {selection.selectedCount > 0 && (
          <>
            <Button
              variant="outline"
              size="sm"
              className="h-8 text-xs"
              onClick={selection.clearSelection}
            >
              Clear
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="h-8 text-xs"
              disabled={!bulk.remoteEnabled}
              onClick={bulk.onUpload}
            >
              ☁ Upload
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="h-8 text-xs"
              disabled={!bulk.remoteEnabled || !bulk.canDownload}
              onClick={bulk.onDownload}
            >
              ⬇ Download
            </Button>
            <Button
              size="sm"
              variant="outline"
              className="h-8 text-xs"
              disabled={!bulk.canClearCache || !bulk.hasCacheableSelection}
              onClick={bulk.onClearCache}
            >
              <Eraser className="h-3.5 w-3.5 mr-1" />
              Clear cache
            </Button>
            <Button
              size="sm"
              variant="destructive"
              className="h-8 text-xs"
              disabled={!bulk.canDelete}
              onClick={bulk.onDelete}
            >
              <Trash2 className="h-3.5 w-3.5 mr-1" />
              Delete
            </Button>
          </>
        )}
        <Button
          variant="ghost"
          size="sm"
          className="h-8 text-xs"
          onClick={selection.exitSelectionMode}
          aria-label="Cancel selection"
        >
          <X className="h-3.5 w-3.5 mr-1" />
          Cancel
        </Button>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2 pb-3 shrink-0 flex-wrap">
      {view.available && view.onViewModeChange && (
        <>
          <Select
            value={view.viewMode}
            onValueChange={(v) => view.onViewModeChange!(v as ComicViewMode)}
          >
            <SelectTrigger className="h-8 w-auto text-xs gap-1.5">
              <Layers
                className="h-3.5 w-3.5 text-muted-foreground"
                aria-hidden="true"
              />
              <span className="text-muted-foreground">View</span>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {VIEW_MODE_OPTIONS.map((opt) => (
                <SelectItem key={opt.value} value={opt.value} className="text-xs">
                  {opt.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <span className="h-5 w-px bg-border mx-1" aria-hidden="true" />
        </>
      )}
      {/* Sort + Filter operate on file rows; the collections grid renders
       *  ComicCollection cards directly, so these controls have no effect
       *  there. Hide them when grouped — they reappear after drill-down. */}
      {!showCollections && (
        <>
          <Select
            value={sort.sortBy}
            onValueChange={(v) => sort.setSortBy(v as SortKey)}
          >
            <SelectTrigger className="h-8 w-auto text-xs gap-1.5">
              <span className="text-muted-foreground">Sort</span>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {SORT_OPTIONS.map((opt) => (
                <SelectItem key={opt.value} value={opt.value} className="text-xs">
                  {opt.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Button
            variant="outline"
            size="icon"
            className="h-8 w-8 shrink-0"
            aria-label={sort.sortDesc ? 'Sort descending' : 'Sort ascending'}
            onClick={() => sort.setSortDesc(!sort.sortDesc)}
          >
            {sort.sortDesc ? (
              <ArrowDown className="h-3.5 w-3.5" />
            ) : (
              <ArrowUp className="h-3.5 w-3.5" />
            )}
          </Button>
          <span className="h-5 w-px bg-border mx-1" aria-hidden="true" />
          <Popover open={filter.filterOpen} onOpenChange={filter.setFilterOpen}>
            <PopoverTrigger asChild>
              <Button
                variant={filter.conditions.length > 0 ? 'default' : 'outline'}
                size="sm"
                className="h-8 text-xs gap-1.5"
              >
                <FilterIcon className="h-3.5 w-3.5" />
                Filter
                {filter.conditions.length > 0 && (
                  <span className="ml-0.5 rounded-full bg-background/30 px-1.5 text-[10px] leading-tight">
                    {filter.conditions.length}
                  </span>
                )}
              </Button>
            </PopoverTrigger>
            <PopoverContent align="start" sideOffset={6} className="w-auto p-3">
              <FilterEditor
                conditions={filter.conditions}
                onConditionsChange={filter.setConditions}
                tags={filter.availableTags}
                authors={filter.availableAuthors}
                bufferUntilApply
                onClose={() => filter.setFilterOpen(false)}
              />
            </PopoverContent>
          </Popover>
          {filter.conditions.map((c) => (
            <div
              key={c.id}
              className="inline-flex items-center rounded-full border bg-secondary/50 hover:bg-secondary transition-colors h-8"
            >
              <button
                type="button"
                onClick={() => filter.setFilterOpen(true)}
                className="text-xs pl-3 pr-1.5 h-full text-foreground/80 focus:outline-none"
                aria-label={`Edit condition: ${describeCondition(c, filter.tagsById, filter.authorsById)}`}
              >
                {describeCondition(c, filter.tagsById, filter.authorsById)}
              </button>
              <button
                type="button"
                onClick={() => filter.removeCondition(c.id)}
                className="px-1.5 h-full text-muted-foreground hover:text-foreground rounded-r-full focus:outline-none"
                aria-label="Remove condition"
              >
                <X className="h-3 w-3" />
              </button>
            </div>
          ))}
        </>
      )}
      <div className="flex-1" />
      {/* Selection mode operates on file rows. Collections grid has no
       *  per-card checkbox, so the button would toggle into a dead state. */}
      {!showCollections && (
        <Button
          variant="outline"
          size="sm"
          className="h-8 text-xs"
          onClick={selection.enterSelectionMode}
          disabled={selection.visibleCount === 0}
        >
          Select
        </Button>
      )}
    </div>
  );
}
