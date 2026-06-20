import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import type { DynamicMetadataFormValues } from '@/components/DynamicMetadataForm';
import { CountBadge, TabPanel } from './ProcessingPipelineParts';
import type { Author, Category, DuplicateAction, Tag } from '@/types';
import type { Bucket, FileItemState, TabKey } from './types';
import { Loader2, Minus } from 'lucide-react';

interface ProcessingPipelineDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onMinimize: () => void;
  totalFiles: number;
  processingCount: number;
  analyzedCount: number;
  activeTab: TabKey;
  onActiveTabChange: (tab: TabKey) => void;
  buckets: Record<Bucket, FileItemState[]>;
  expandedIds: Set<string>;
  onToggleExpand: (path: string) => void;
  onToggleSelected: (path: string) => void;
  onToggleAllInTab: (tab: TabKey, value: boolean) => void;
  onFormChange: (path: string, values: DynamicMetadataFormValues) => void;
  onApproveSuggestedTag: (path: string, tagName: string) => void;
  onDismissSuggestedTag: (path: string, tagName: string) => void;
  onApproveSuggestedAuthor: (path: string, authorName: string) => void;
  onDismissSuggestedAuthor: (path: string, authorName: string) => void;
  onDuplicateAction: (path: string, action: DuplicateAction) => void;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onTagCreate: (name: string) => Promise<Tag>;
  onAuthorCreate: (name: string) => Promise<Author>;
  selectedNeedingDecision: number;
  selectedToImport: number;
  selectedToDelete: number;
  analyzing: boolean;
  importing: boolean;
  onCancelAnalysis: () => void;
  onImport: () => void;
}

export function ProcessingPipelineDialog({
  open,
  onOpenChange,
  onMinimize,
  totalFiles,
  processingCount,
  analyzedCount,
  activeTab,
  onActiveTabChange,
  buckets,
  expandedIds,
  onToggleExpand,
  onToggleSelected,
  onToggleAllInTab,
  onFormChange,
  onApproveSuggestedTag,
  onDismissSuggestedTag,
  onApproveSuggestedAuthor,
  onDismissSuggestedAuthor,
  onDuplicateAction,
  categories,
  tags,
  authors,
  onTagCreate,
  onAuthorCreate,
  selectedNeedingDecision,
  selectedToImport,
  selectedToDelete,
  analyzing,
  importing,
  onCancelAnalysis,
  onImport,
}: ProcessingPipelineDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange} modal={false}>
      <DialogContent
        className="max-w-3xl max-h-[90vh] flex flex-col overflow-hidden"
        onInteractOutside={(e) => e.preventDefault()}
        onEscapeKeyDown={(e) => e.preventDefault()}
      >
        {/* Minimize sits absolutely to the left of the shadcn Dialog's built-in
            close button, sharing the same top baseline without squeezing the title row. */}
        <button
          type="button"
          onClick={onMinimize}
          className="absolute right-12 top-4 text-muted-foreground hover:text-foreground hover:bg-secondary p-1 rounded-xl opacity-60 hover:opacity-100 transition-opacity"
          aria-label="Minimize import dialog"
          title="Minimize - analysis keeps running"
        >
          <Minus className="h-4 w-4" />
        </button>
        <DialogHeader>
          <DialogTitle className="flex items-baseline gap-3">
            <span>Import</span>
            <span className="font-serif-italic text-sm text-muted-foreground">
              - {totalFiles} {totalFiles === 1 ? 'file' : 'files'}
            </span>
          </DialogTitle>
        </DialogHeader>

        {processingCount > 0 && (
          <div className="flex items-center gap-2 px-1 pb-2 text-xs text-muted-foreground">
            <Loader2 className="h-3.5 w-3.5 animate-spin text-primary" />
            <span>
              Analyzing {analyzedCount + 1} of {totalFiles}...
            </span>
            <div className="flex-1 h-[3px] rounded-full bg-muted overflow-hidden ml-2">
              <div
                className="h-full bg-primary transition-[width] duration-300"
                style={{
                  width: `${(analyzedCount / Math.max(1, totalFiles)) * 100}%`,
                }}
              />
            </div>
          </div>
        )}

        <Tabs
          value={activeTab}
          onValueChange={(value) => onActiveTabChange(value as TabKey)}
          className="flex-1 flex flex-col min-h-0"
        >
          <TabsList className="self-start">
            <TabsTrigger value="review" className="gap-2">
              Review
              <CountBadge count={buckets.review.length} tone="warning" />
            </TabsTrigger>
            <TabsTrigger value="ready" className="gap-2">
              Ready
              <CountBadge count={buckets.ready.length} tone="success" />
            </TabsTrigger>
            <TabsTrigger value="failed" className="gap-2">
              Failed
              <CountBadge count={buckets.failed.length} tone="destructive" />
            </TabsTrigger>
          </TabsList>

          <TabPanel
            tabKey="review"
            isActive={activeTab === 'review'}
            items={buckets.review}
            emptyLabel="No files need review."
            expandedIds={expandedIds}
            onToggleExpand={onToggleExpand}
            onToggleSelected={onToggleSelected}
            onToggleAll={onToggleAllInTab}
            onFormChange={onFormChange}
            onApproveSuggestedTag={onApproveSuggestedTag}
            onDismissSuggestedTag={onDismissSuggestedTag}
            onApproveSuggestedAuthor={onApproveSuggestedAuthor}
            onDismissSuggestedAuthor={onDismissSuggestedAuthor}
            onDuplicateAction={onDuplicateAction}
            categories={categories}
            tags={tags}
            authors={authors}
            onTagCreate={onTagCreate}
            onAuthorCreate={onAuthorCreate}
          />

          <TabPanel
            tabKey="ready"
            isActive={activeTab === 'ready'}
            items={buckets.ready}
            emptyLabel={analyzing ? 'Waiting for analysis to finish...' : 'No files are ready yet.'}
            expandedIds={expandedIds}
            onToggleExpand={onToggleExpand}
            onToggleSelected={onToggleSelected}
            onToggleAll={onToggleAllInTab}
            onFormChange={onFormChange}
            onApproveSuggestedTag={onApproveSuggestedTag}
            onDismissSuggestedTag={onDismissSuggestedTag}
            onApproveSuggestedAuthor={onApproveSuggestedAuthor}
            onDismissSuggestedAuthor={onDismissSuggestedAuthor}
            onDuplicateAction={onDuplicateAction}
            categories={categories}
            tags={tags}
            authors={authors}
            onTagCreate={onTagCreate}
            onAuthorCreate={onAuthorCreate}
          />

          <TabPanel
            tabKey="failed"
            isActive={activeTab === 'failed'}
            items={buckets.failed}
            emptyLabel="Nothing failed."
            expandedIds={expandedIds}
            onToggleExpand={onToggleExpand}
            onToggleSelected={onToggleSelected}
            onToggleAll={onToggleAllInTab}
            onFormChange={onFormChange}
            onApproveSuggestedTag={onApproveSuggestedTag}
            onDismissSuggestedTag={onDismissSuggestedTag}
            onApproveSuggestedAuthor={onApproveSuggestedAuthor}
            onDismissSuggestedAuthor={onDismissSuggestedAuthor}
            onDuplicateAction={onDuplicateAction}
            categories={categories}
            tags={tags}
            authors={authors}
            onTagCreate={onTagCreate}
            onAuthorCreate={onAuthorCreate}
          />
        </Tabs>

        <DialogFooter className="flex-col gap-3 sm:flex-row sm:justify-between">
          <div className="text-xs text-muted-foreground">
            {selectedNeedingDecision > 0 && (
              <span>
                {selectedNeedingDecision} duplicate decision
                {selectedNeedingDecision !== 1 ? 's' : ''} needed
              </span>
            )}
            {selectedNeedingDecision === 0 && selectedToImport > 0 && (
              <span>
                {selectedToImport} selected to import
                {selectedToDelete > 0 && (
                  <span className="ml-2">· {selectedToDelete} to delete</span>
                )}
              </span>
            )}
            {selectedNeedingDecision === 0 && selectedToImport === 0 && selectedToDelete > 0 && (
              <span>{selectedToDelete} to delete</span>
            )}
            {selectedNeedingDecision === 0 && selectedToImport === 0 && selectedToDelete === 0 && (
              <span>Nothing selected.</span>
            )}
          </div>

          <div className="flex gap-2">
            <Button
              variant="outline"
              onClick={analyzing ? onCancelAnalysis : () => onOpenChange(false)}
            >
              {analyzing ? 'Cancel' : 'Close'}
            </Button>
            <Button
              onClick={onImport}
              disabled={
                importing ||
                selectedNeedingDecision > 0 ||
                (selectedToImport === 0 && selectedToDelete === 0)
              }
            >
              {importing ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Importing...
                </>
              ) : selectedNeedingDecision > 0 ? (
                `Choose ${selectedNeedingDecision} duplicate action${
                  selectedNeedingDecision !== 1 ? 's' : ''
                }`
              ) : selectedToImport > 0 ? (
                `Import ${selectedToImport}${
                  selectedToDelete > 0 ? ` (${selectedToDelete} delete)` : ''
                }`
              ) : (
                `Delete ${selectedToDelete} file${selectedToDelete !== 1 ? 's' : ''}`
              )}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
