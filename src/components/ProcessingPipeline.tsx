import { useState, useEffect, useCallback } from 'react';
import { UnlistenFn } from '@tauri-apps/api/event';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Progress } from '@/components/ui/progress';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Card, CardContent } from '@/components/ui/card';
import {
  DynamicMetadataForm,
  type DynamicMetadataFormValues,
} from '@/components/DynamicMetadataForm';
import { filePrepareImport, fileCreate, coverSet, listenProcessingProgress } from '@/lib/tauri';
import {
  ChevronDown,
  ChevronRight,
  Loader2,
  CheckCircle2,
  AlertCircle,
  FileText,
} from 'lucide-react';
import type {
  Category,
  Tag,
  Author,
  FilePreparedImport,
  ProcessingProgress,
  MetadataType,
} from '@/types';

type FileStatus = 'pending' | 'analyzing' | 'done' | 'error';

interface FileItemState {
  path: string;
  fileName: string;
  status: FileStatus;
  preparedImport?: FilePreparedImport;
  formValues: DynamicMetadataFormValues;
  error?: string;
  userEdited: boolean;
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
};

function StatusBadge({ status }: { status: FileStatus }) {
  const variants: Record<FileStatus, 'gray' | 'orange' | 'green' | 'destructive'> = {
    pending: 'gray',
    analyzing: 'orange',
    done: 'green',
    error: 'destructive',
  };

  const icons: Record<FileStatus, React.ReactNode> = {
    pending: <FileText className="h-3 w-3" />,
    analyzing: <Loader2 className="h-3 w-3 animate-spin" />,
    done: <CheckCircle2 className="h-3 w-3" />,
    error: <AlertCircle className="h-3 w-3" />,
  };

  const labels: Record<FileStatus, string> = {
    pending: 'Pending',
    analyzing: 'Analyzing',
    done: 'Ready',
    error: 'Error',
  };

  return (
    <Badge variant={variants[status]} className="gap-1.5">
      {icons[status]}
      {labels[status]}
    </Badge>
  );
}

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
  const [progress, setProgress] = useState<ProcessingProgress | null>(null);
  const [abortController, setAbortController] = useState<AbortController | null>(null);

  useEffect(() => {
    if (open && paths.length > 0) {
      const initialItems: FileItemState[] = paths.map((path) => {
        const fileName = path.substring(Math.max(0, path.lastIndexOf('/') + 1)) || path;
        return {
          path,
          fileName,
          status: 'pending',
          formValues: { ...EMPTY_FORM_VALUES, display_name: fileName },
          userEdited: false,
        };
      });
      setFileItems(initialItems);
      const firstPath = paths[0];
      if (firstPath) {
        setExpandedIds(new Set([firstPath]));
      }
      setProgress(null);
      setAnalyzing(false);
      setImporting(false);
    }
  }, [open, paths]);

  useEffect(() => {
    if (!open || paths.length === 0 || analyzing) return;

    const runAnalysis = async () => {
      setAnalyzing(true);
      const controller = new AbortController();
      setAbortController(controller);

      setFileItems((prev) => prev.map((item) => ({ ...item, status: 'analyzing' as FileStatus })));

      let unlisten: UnlistenFn | null = null;
      try {
        unlisten = await listenProcessingProgress((p) => {
          setProgress(p);
          setFileItems((prev) =>
            prev.map((item) =>
              item.path === p.current_file ? { ...item, status: 'analyzing' as FileStatus } : item
            )
          );
        });
      } catch (error) {
        console.error('Failed to listen for progress:', error);
      }

      try {
        const results = await filePrepareImport(paths);

        setFileItems((prev) =>
          prev.map((item) => {
            const result = results.find((r: FilePreparedImport) => r.path === item.path);
            if (result) {
              const formValues: DynamicMetadataFormValues = item.userEdited
                ? item.formValues
                : {
                    display_name: result.display_name || result.file_name,
                    category_id: result.category_id,
                    tag_ids: result.tag_ids,
                    author_ids: result.author_ids,
                    metadata: result.metadata.map((m) => ({
                      key: m.key,
                      value: m.value,
                      data_type: m.data_type as MetadataType,
                    })),
                  };
              return {
                ...item,
                status: 'done' as FileStatus,
                preparedImport: result,
                formValues,
              };
            }
            return { ...item, status: 'error' as FileStatus, error: 'Analysis failed' };
          })
        );
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
        setProgress(null);
        setAbortController(null);
      }
    };

    runAnalysis();
  }, [open, paths, analyzing]);

  const handleCancelAnalysis = useCallback(() => {
    if (abortController) {
      abortController.abort();
      setAnalyzing(false);
      setFileItems((prev) =>
        prev.map((item) =>
          item.status === 'analyzing' ? { ...item, status: 'pending' as FileStatus } : item
        )
      );
    }
    onOpenChange(false);
  }, [abortController, onOpenChange]);

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

  const handleImportAll = useCallback(async () => {
    if (importing) return;

    const readyFiles = fileItems.filter((item) => item.status === 'done');
    if (readyFiles.length === 0) return;

    setImporting(true);

    const errors: string[] = [];

    try {
      for (const item of readyFiles) {
        try {
          const result = await fileCreate({
            path: item.path,
            display_name: item.formValues.display_name,
            category_id: item.formValues.category_id,
            tag_ids: item.formValues.tag_ids,
            author_ids: item.formValues.author_ids,
            metadata: item.formValues.metadata,
          });

          if (item.formValues.cover_data && result.id) {
            const binaryString = atob(item.formValues.cover_data);
            const bytes = new Uint8Array(binaryString.length);
            for (let i = 0; i < binaryString.length; i++) {
              bytes[i] = binaryString.charCodeAt(i);
            }
            await coverSet(result.id, Array.from(bytes));
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

  const doneCount = fileItems.filter((item) => item.status === 'done').length;
  const errorCount = fileItems.filter((item) => item.status === 'error').length;
  const totalFiles = fileItems.length;

  const progressPercent = progress
    ? Math.round((progress.current / progress.total) * 100)
    : analyzing && fileItems.length > 0
      ? Math.round(
          (fileItems.filter((f) => f.status !== 'analyzing').length / fileItems.length) * 100
        )
      : 0;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            Import Files
            {analyzing && (
              <Badge variant="orange" className="gap-1.5">
                <Loader2 className="h-3 w-3 animate-spin" />
                Analyzing
              </Badge>
            )}
          </DialogTitle>
        </DialogHeader>

        {/* Progress bar during analysis */}
        {analyzing && (
          <div className="space-y-2 pb-4">
            <Progress value={progressPercent} className="h-2" />
            <div className="flex justify-between text-xs text-muted-foreground">
              <span>
                {progress?.current_file
                  ? `Analyzing ${progress.current_file.split('/').pop()}...`
                  : 'Starting analysis...'}
              </span>
              <span>{progressPercent}%</span>
            </div>
          </div>
        )}

        {/* File list with expandable forms */}
        <ScrollArea className="flex-1 min-h-0 pr-4">
          <div className="space-y-3 py-2">
            {fileItems.map((item) => (
              <Card
                key={item.path}
                className="transition-all duration-200 cursor-pointer hover:border-primary/30"
                onClick={() => handleToggleExpand(item.path)}
              >
                <CardContent className="p-3">
                  {/* File header row */}
                  <div className="flex items-center gap-3">
                    {/* Expand/collapse icon */}
                    <div className="shrink-0 text-muted-foreground">
                      {expandedIds.has(item.path) ? (
                        <ChevronDown className="h-4 w-4" />
                      ) : (
                        <ChevronRight className="h-4 w-4" />
                      )}
                    </div>

                    {/* File name */}
                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-medium truncate">{item.fileName}</p>
                      {item.preparedImport?.unresolved_author_names &&
                        item.preparedImport.unresolved_author_names.length > 0 && (
                          <p className="text-xs text-muted-foreground truncate">
                            Unresolved authors: {item.preparedImport.unresolved_author_names.join(', ')}
                          </p>
                        )}
                    </div>

                    {/* Status badge */}
                    <StatusBadge status={item.status} />
                  </div>

                  {/* Expandable form */}
                  {expandedIds.has(item.path) && item.status === 'done' && (
                    <div className="mt-4 pt-4 border-t border-border">
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
        </ScrollArea>

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
                `Import All (${doneCount})`
              )}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
