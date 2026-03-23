import { Card, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { FileText, Edit, Trash2 } from 'lucide-react';
import type { FileEntry, Tag } from '@/types';

interface FileCardProps {
  file: FileEntry;
  onEdit?: (file: FileEntry) => void;
  onDelete?: (file: FileEntry) => void;
}

export function FileCard({ file, onEdit, onDelete }: FileCardProps) {
  const statusColor = {
    available: 'bg-emerald-500',
    missing: 'bg-destructive',
    moved: 'bg-amber-500',
  };

  const statusLabel = {
    available: 'File available',
    missing: 'File not found',
    moved: 'File has been moved',
  };

  return (
    <Card className="hover:shadow-md hover:border-accent/30 transition-all duration-200">
      <CardContent className="p-5">
        <div className="flex items-start justify-between gap-4">
          <div className="flex items-start gap-4 min-w-0 flex-1">
            <div className="p-2.5 rounded-xl bg-secondary">
              <FileText className="h-5 w-5 text-muted-foreground" aria-hidden="true" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2.5">
                <h3 className="font-medium text-foreground truncate" id={`file-name-${file.id}`}>
                  {file.display_name}
                </h3>
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <div
                        className={`w-2 h-2 rounded-full ${statusColor[file.file_status as keyof typeof statusColor]}`}
                        role="status"
                        aria-label={statusLabel[file.file_status as keyof typeof statusLabel]}
                      />
                    </TooltipTrigger>
                    <TooltipContent>
                      <p>{statusLabel[file.file_status as keyof typeof statusLabel]}</p>
                    </TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              </div>
              <TooltipProvider>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <p className="text-sm text-muted-foreground truncate mt-1">{file.path}</p>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p className="max-w-xs break-all">{file.path}</p>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
              {file.tags && file.tags.length > 0 && (
                <div className="flex flex-wrap gap-1.5 mt-3">
                  {file.tags.map((tag: Tag) => (
                    <Badge key={tag.id} variant="secondary" className="text-xs font-normal">
                      {tag.name}
                    </Badge>
                  ))}
                </div>
              )}
            </div>
          </div>
          <div className="flex gap-1 shrink-0" role="group" aria-label="File actions">
            <TooltipProvider>
              {onEdit && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => onEdit(file)}
                      aria-label={`Edit ${file.display_name}`}
                      className="hover:bg-secondary"
                    >
                      <Edit className="h-4 w-4" aria-hidden="true" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p>Edit file details</p>
                  </TooltipContent>
                </Tooltip>
              )}
              {onDelete && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => onDelete(file)}
                      aria-label={`Delete ${file.display_name}`}
                      className="hover:bg-secondary hover:text-destructive"
                    >
                      <Trash2 className="h-4 w-4" aria-hidden="true" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>
                    <p>Remove from library</p>
                  </TooltipContent>
                </Tooltip>
              )}
            </TooltipProvider>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}