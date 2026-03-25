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
    available: 'bg-green-500',
    missing: 'bg-destructive',
    moved: 'bg-yellow-500',
  };

  const statusLabel = {
    available: 'File available',
    missing: 'File not found',
    moved: 'File has been moved',
  };

  return (
    <Card className="group">
      <CardContent className="p-0">
        <div className="flex items-start justify-between gap-4 p-4">
          <div className="flex items-start gap-3 min-w-0 flex-1">
            <div className="mt-0.5">
              <FileText className="h-5 w-5 text-muted-foreground shrink-0" aria-hidden="true" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <h3 className="font-medium text-sm truncate" id={`file-name-${file.id}`}>
                  {file.display_name}
                </h3>
                <TooltipProvider>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <div
                        className={`w-1.5 h-1.5 rounded-full ${statusColor[file.file_status as keyof typeof statusColor]}`}
                        role="status"
                        aria-label={statusLabel[file.file_status as keyof typeof statusLabel]}
                      />
                    </TooltipTrigger>
                    <TooltipContent side="top">
                      <p>{statusLabel[file.file_status as keyof typeof statusLabel]}</p>
                    </TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              </div>
              <TooltipProvider>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <p className="text-xs text-muted-foreground truncate mt-0.5">{file.path}</p>
                  </TooltipTrigger>
                  <TooltipContent side="top">
                    <p className="max-w-xs break-all">{file.path}</p>
                  </TooltipContent>
                </Tooltip>
              </TooltipProvider>
              {file.tags && file.tags.length > 0 && (
                <div className="flex flex-wrap gap-1 mt-2">
                  {file.tags.map((tag: Tag) => (
                    <Badge key={tag.id} variant="gray" className="text-xs font-normal">
                      {tag.name}
                    </Badge>
                  ))}
                </div>
              )}
            </div>
          </div>
          <div className="flex gap-0.5 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity" role="group" aria-label="File actions">
            <TooltipProvider>
              {onEdit && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => onEdit(file)}
                      aria-label={`Edit ${file.display_name}`}
                    >
                      <Edit className="h-4 w-4" aria-hidden="true" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent side="top">
                    <p>Edit</p>
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
                    >
                      <Trash2 className="h-4 w-4" aria-hidden="true" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent side="top">
                    <p>Delete</p>
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