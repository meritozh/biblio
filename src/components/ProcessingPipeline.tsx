import { useState, useEffect, useCallback, useRef } from 'react';
import { UnlistenFn } from '@tauri-apps/api/event';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import {
  DynamicMetadataForm,
  type DynamicMetadataFormValues,
} from '@/components/DynamicMetadataForm';
import { SuggestedTagChip } from '@/components/SuggestedTagChip';
import { DuplicateWarning } from '@/components/DuplicateWarning';
import { filePrepareImport, fileCreate, fileReplace, cancelProcessing, listenProcessingProgress } from '@/lib/tauri';
import {
  ChevronDown,
  ChevronRight,
  Loader2,
  CheckCircle2,
  AlertCircle,
  AlertTriangle,
  FileText,
} from 'lucide-react';
import type {
  Category,
  Tag,
  Author,
  FilePreparedImport,
  MetadataType,
  DuplicateAction,
  FileAnalysisStatus,
} from '@/types';

type FileStatus = FileAnalysisStatus;

interface FileItemState {
  path: string;
  fileName: string;
  status: FileStatus;
  preparedImport?: FilePreparedImport;
  formValues: DynamicMetadataFormValues;
  error?: string;
  userEdited: boolean;
  suggestedTags: string[];
  duplicateAction: DuplicateAction | null;
}

interface ProcessingPipelineProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  paths: string[];
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onCategoryCreated: (category: Category) => void;
  onTagCreate: (name: string) => Promise<Tag>;
  onAuthorCreate: (name: string) => Promise<Author>;
  onImportComplete: () => void;
}

const EMPTY_FORM_VALUES: DynamicMetadataFormValues = {
  display_name: '',
  category_id: null,
  tag_ids: [],
  author_ids: [],
  metadata: [],
  progress: '',
};


export function ProcessingPipeline({
  open,
  onOpenChange,
  paths,
  categories,
  tags,
  authors,
  onCategoryCreated,
  onTagCreate,
  onAuthorCreate,
  onImportComplete,
}: ProcessingPipelineProps) {
  const [fileItems, setFileItems] = useState<FileItemState[]>([]);
  const [analyzing, setAnalyzing] = useState(false);
  const [importing, setImporting] = useState(false);
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());
  const analysisStarted = useRef(false);

  useEffect(() => {
    if (!open || paths.length === 0) {
      analysisStarted.current = false;
      return;
    }

    // Prevent re-entry
    if (analysisStarted.current) return;
    analysisStarted.current = true;

    const initialItems: FileItemState[] = paths.map((path) => {
      const fileName = path.substring(Math.max(0, path.lastIndexOf('/') + 1)) || path;
      return {
        path,
        fileName,
        status: 'pending' as FileStatus,
        formValues: { ...EMPTY_FORM_VALUES, display_name: fileName },
        userEdited: false,
        suggestedTags: [],
        duplicateAction: null,
      };
    });
    setFileItems(initialItems);
    setImporting(false);

    const runAnalysis = async () => {
      setAnalyzing(true);

      let unlisten: UnlistenFn | null = null;
      try {
        unlisten = await listenProcessingProgress((p) => {
          setFileItems((prev) =>
            prev.map((item) =>
              item.path === p.current_file
                ? { ...item, status: p.status as FileStatus }
                : item
            )
          );
        });
      } catch (error) {
        console.error('Failed to listen for progress:', error);
      }

      try {
        const results = await filePrepareImport(paths);

        // Auto-create unresolved authors from LLM results
        const createdAuthorIds: Record<string, number> = {};
        for (const result of results) {
          for (const name of result.unresolved_author_names) {
            if (!createdAuthorIds[name]) {
              try {
                const newAuthor = await onAuthorCreate(name);
                createdAuthorIds[name] = newAuthor.id;
              } catch {
                // Author creation failed (e.g. duplicate) — skip
              }
            }
          }
        }

        const updatedItems = results.map((result: FilePreparedImport) => {
          const prev = fileItems.find((item) => item.path === result.path);

          // Merge resolved + newly created author IDs
          const allAuthorIds = [...result.author_ids];
          for (const name of result.unresolved_author_names) {
            const id = createdAuthorIds[name];
            if (id && !allAuthorIds.includes(id)) {
              allAuthorIds.push(id);
            }
          }

          const formValues: DynamicMetadataFormValues = prev?.userEdited
            ? prev.formValues
            : {
                display_name: result.display_name || result.file_name,
                category_id: result.category_id,
                tag_ids: result.tag_ids,
                author_ids: allAuthorIds,
                metadata: result.metadata.map((m) => ({
                  key: m.key,
                  value: m.value,
                  data_type: m.data_type as MetadataType,
                })),
                progress: result.progress ?? '',
                cover_data: result.cover_data,
                cover_mime_type: result.cover_mime_type,
              };
          return {
            path: result.path,
            fileName: result.file_name,
            status: (prev?.status === 'partial' || prev?.status === 'error')
              ? prev.status
              : 'ready' as FileStatus,
            preparedImport: result,
            formValues,
            userEdited: prev?.userEdited ?? false,
            suggestedTags: result.suggested_tags ?? [],
            duplicateAction: result.duplicate_of?.recommendation ?? null,
          };
        });

        setFileItems((prev) => {
          const resultPaths = new Set(updatedItems.map((r) => r.path));
          const failed = prev
            .filter((item) => !resultPaths.has(item.path))
            .map((item) => ({ ...item, status: 'error' as FileStatus, error: 'Analysis failed' }));
          return [...updatedItems, ...failed];
        });

        // Auto-expand all items after analysis
        setExpandedIds(new Set(paths));
      } catch (error) {
        console.error('Analysis failed:', error);
        setFileItems((prev) =>
          prev.map((item) => ({
            ...item,
            status: 'error' as FileStatus,
            error: String(error),
          }))
        );
      } finally {
        if (unlisten) {
          unlisten();
        }
        setAnalyzing(false);
  
      }
    };

    runAnalysis();
  }, [open, paths]);

  const handleCancelAnalysis = useCallback(async () => {
    await cancelProcessing();
    analysisStarted.current = false;
    setAnalyzing(false);
    setFileItems((prev) =>
      prev.map((item) =>
        item.status === 'extracting_name' || item.status === 'analyzing_content'
          ? { ...item, status: 'pending' as FileStatus }
          : item
      )
    );
    onOpenChange(false);
  }, [onOpenChange]);

  const handleToggleExpand = useCallback((path: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  const handleFormChange = useCallback((path: string, values: DynamicMetadataFormValues) => {
    setFileItems((prev) =>
      prev.map((item) =>
        item.path === path ? { ...item, formValues: values, userEdited: true } : item
      )
    );
  }, []);

  const handleApproveSuggestedTag = useCallback(
    async (path: string, tagName: string) => {
      try {
        const newTag = await onTagCreate(tagName);
        setFileItems((prev) =>
          prev.map((item) => {
            if (item.path !== path) return item;
            return {
              ...item,
              suggestedTags: item.suggestedTags.filter((t) => t !== tagName),
              formValues: {
                ...item.formValues,
                tag_ids: [...item.formValues.tag_ids, newTag.id],
              },
            };
          })
        );
      } catch (error) {
        console.error('Failed to create tag:', error);
      }
    },
    [onTagCreate]
  );

  const handleDismissSuggestedTag = useCallback((path: string, tagName: string) => {
    setFileItems((prev) =>
      prev.map((item) =>
        item.path === path
          ? { ...item, suggestedTags: item.suggestedTags.filter((t) => t !== tagName) }
          : item
      )
    );
  }, []);

  const handleDuplicateAction = useCallback((path: string, action: DuplicateAction) => {
    setFileItems((prev) =>
      prev.map((item) =>
        item.path === path ? { ...item, duplicateAction: action } : item
      )
    );
  }, []);

  const handleImportAll = useCallback(async () => {
    if (importing) return;

    const readyFiles = fileItems.filter(
      (item) => item.status === 'ready' || item.status === 'partial'
    );
    if (readyFiles.length === 0) return;

    setImporting(true);

    const errors: string[] = [];

    try {
      for (const item of readyFiles) {
        if (item.duplicateAction === 'Skip') continue;

        try {
          const createParams = {
            path: item.path,
            display_name: item.formValues.display_name,
            category_id: item.formValues.category_id,
            tag_ids: item.formValues.tag_ids,
            author_ids: item.formValues.author_ids,
            metadata: item.formValues.metadata,
            progress: item.formValues.progress,
            cover_data: item.formValues.cover_data,
            cover_mime_type: item.formValues.cover_mime_type,
          };

          if (
            item.duplicateAction === 'Replace' &&
            item.preparedImport?.duplicate_of
          ) {
            await fileReplace(
              item.preparedImport.duplicate_of.existing_file_id,
              createParams
            );
          } else {
            await fileCreate(createParams);
          }
        } catch (error) {
          console.error(`Failed to import ${item.fileName}:`, error);
          errors.push(`${item.fileName}: ${String(error)}`);
          setFileItems((prev) =>
            prev.map((f) =>
              f.path === item.path
                ? { ...f, status: 'error' as FileStatus, error: String(error) }
                : f
            )
          );
        }
      }

      if (errors.length === 0) {
        onOpenChange(false);
        onImportComplete();
      } else {
        alert(`Some files failed to import:\n${errors.join('\n')}`);
      }
    } finally {
      setImporting(false);
    }
  }, [fileItems, importing, onOpenChange, onImportComplete]);

  const doneCount = fileItems.filter(
    (item) => item.status === 'ready' || item.status === 'partial'
  ).length;
  const errorCount = fileItems.filter((item) => item.status === 'error').length;
  const totalFiles = fileItems.length;
  const skipCount = fileItems.filter(
    (item) => item.duplicateAction === 'Skip'
  ).length;


  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="max-w-2xl max-h-[90vh] flex flex-col overflow-hidden"
        onInteractOutside={(e) => e.preventDefault()}
        onEscapeKeyDown={(e) => e.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>Import Files</DialogTitle>
        </DialogHeader>

        {/* File list — native overflow so scrollbar is reliable and sits outside content padding */}
        <div className="flex-1 min-h-0 -mx-6 overflow-y-auto">
          <div className="space-y-3 py-2 px-6">
            {fileItems.map((item) => (
              <Card
                key={item.path}
                className="transition-all duration-200"
              >
                <CardContent className="p-3">
                  {/* File header row — only this is clickable, so popovers in the expanded form below aren't caught by the toggle handler */}
                  <div
                    className="flex items-center gap-3 cursor-pointer hover:text-primary/90"
                    onClick={() => handleToggleExpand(item.path)}
                  >
                    {/* Status icon */}
                    <div className="shrink-0">
                      {item.status === 'extracting_name' || item.status === 'analyzing_content' ? (
                        <Loader2 className="h-4 w-4 animate-spin text-primary" />
                      ) : item.status === 'ready' ? (
                        <CheckCircle2 className="h-4 w-4 text-green-500" />
                      ) : item.status === 'partial' ? (
                        <AlertTriangle className="h-4 w-4 text-amber-500" />
                      ) : item.status === 'error' ? (
                        <AlertCircle className="h-4 w-4 text-destructive" />
                      ) : (
                        <FileText className="h-4 w-4 text-muted-foreground" />
                      )}
                    </div>

                    {/* File name */}
                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-medium truncate">{item.fileName}</p>
                      {item.status === 'extracting_name' && (
                        <p className="text-xs text-muted-foreground">Extracting name...</p>
                      )}
                      {item.status === 'analyzing_content' && (
                        <p className="text-xs text-muted-foreground">Analyzing content...</p>
                      )}
                      {item.status === 'partial' && (
                        <p className="text-xs text-amber-600">Partial extraction — please fill missing fields</p>
                      )}
                    </div>

                    {/* Expand/collapse icon */}
                    <div className="shrink-0 text-muted-foreground">
                      {expandedIds.has(item.path) ? (
                        <ChevronDown className="h-4 w-4" />
                      ) : (
                        <ChevronRight className="h-4 w-4" />
                      )}
                    </div>
                  </div>

                  {/* Expandable form */}
                  {expandedIds.has(item.path) && (item.status === 'ready' || item.status === 'partial') && (
                    <div className="mt-4 pt-4 border-t border-border space-y-4">
                      {/* Duplicate warning */}
                      {item.preparedImport?.duplicate_of && (
                        <DuplicateWarning
                          duplicateInfo={item.preparedImport.duplicate_of}
                          newProgress={item.formValues.progress ?? null}
                          selectedAction={item.duplicateAction ?? item.preparedImport.duplicate_of.recommendation}
                          onActionChange={(action) => handleDuplicateAction(item.path, action)}
                        />
                      )}

                      {/* Suggested new tags */}
                      {item.suggestedTags.length > 0 && (
                        <div className="space-y-2">
                          <p className="text-xs text-muted-foreground">Suggested new tags:</p>
                          <div className="flex flex-wrap gap-1.5">
                            {item.suggestedTags.map((tag) => (
                              <SuggestedTagChip
                                key={tag}
                                name={tag}
                                onApprove={(name) => handleApproveSuggestedTag(item.path, name)}
                                onDismiss={(name) => handleDismissSuggestedTag(item.path, name)}
                              />
                            ))}
                          </div>
                        </div>
                      )}

                      <DynamicMetadataForm
                        values={item.formValues}
                        onChange={(values) => handleFormChange(item.path, values)}
                        categories={categories}
                        tags={tags}
                        authors={authors}
                        onCategoryCreated={onCategoryCreated}
                        onTagCreate={onTagCreate}
                        onAuthorCreate={onAuthorCreate}
                      />
                    </div>
                  )}

                  {/* Error message */}
                  {item.status === 'error' && item.error && (
                    <div className="mt-2 text-xs text-destructive">{item.error}</div>
                  )}
                </CardContent>
              </Card>
            ))}
          </div>
        </div>

        {/* Footer with summary and actions */}
        <DialogFooter className="flex-col gap-3 sm:flex-row sm:justify-between">
          {/* Summary */}
          <div className="text-xs text-muted-foreground">
            {doneCount} of {totalFiles} ready to import
            {errorCount > 0 && <span className="text-destructive ml-2">({errorCount} errors)</span>}
          </div>

          {/* Action buttons */}
          <div className="flex gap-2">
            <Button
              variant="outline"
              onClick={analyzing ? handleCancelAnalysis : () => onOpenChange(false)}
            >
              {analyzing ? 'Cancel' : 'Close'}
            </Button>
            <Button onClick={handleImportAll} disabled={analyzing || importing || doneCount === 0}>
              {importing ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Importing...
                </>
              ) : (
                `Import ${doneCount - skipCount} File${doneCount - skipCount !== 1 ? 's' : ''}${skipCount > 0 ? ` (${skipCount} skipped)` : ''}`
              )}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
