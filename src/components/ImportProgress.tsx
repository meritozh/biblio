import { Progress } from '@/components/ui/progress';
import { Button } from '@/components/ui/button';
import { X } from 'lucide-react';

interface ImportProgressProps {
  current: number;
  total: number;
  currentFile: string;
  onCancel?: () => void;
}

export function ImportProgress({ current, total, currentFile, onCancel }: ImportProgressProps) {
  const percentage = total > 0 ? Math.round((current / total) * 100) : 0;

  return (
    <div className="fixed inset-0 bg-background/80 backdrop-blur-sm z-50 flex items-center justify-center">
      <div className="bg-card border rounded-lg shadow-lg p-6 w-full max-w-md mx-4">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold">Importing Files</h3>
          {onCancel && (
            <Button variant="ghost" size="icon" onClick={onCancel}>
              <X className="h-4 w-4" />
            </Button>
          )}
        </div>

        <div className="space-y-4">
          <Progress value={percentage} className="h-2" />

          <div className="flex justify-between text-sm text-muted-foreground">
            <span>
              {current} of {total} files
            </span>
            <span>{percentage}%</span>
          </div>

          <div className="text-sm text-muted-foreground truncate">
            <span className="font-medium">Current:</span> {currentFile || 'Preparing...'}
          </div>
        </div>
      </div>
    </div>
  );
}
