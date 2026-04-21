import { AlertTriangle } from 'lucide-react';
import type { DuplicateInfo, DuplicateAction } from '@/types';

interface DuplicateWarningProps {
  duplicateInfo: DuplicateInfo;
  newProgress: string | null;
  selectedAction: DuplicateAction;
  onActionChange: (action: DuplicateAction) => void;
}

const ACTION_LABELS: Record<DuplicateAction, { label: string; description: string }> = {
  Replace: {
    label: 'Replace existing',
    description: 'Delete the existing file and import this one',
  },
  Delete: {
    label: 'Delete',
    description: 'Do not import — permanently delete this file from disk',
  },
  ImportAnyway: {
    label: 'Import anyway',
    description: 'Keep both files',
  },
};

export function DuplicateWarning({
  duplicateInfo,
  newProgress,
  selectedAction,
  onActionChange,
}: DuplicateWarningProps) {
  return (
    <div
      className="rounded-lg border border-amber-200 bg-amber-50/50 p-3 space-y-3"
      onClick={(e) => e.stopPropagation()}
    >
      <div className="flex items-center gap-2 text-amber-700">
        <AlertTriangle className="h-4 w-4 shrink-0" />
        <span className="text-sm font-medium">
          Duplicate of &ldquo;{duplicateInfo.existing_display_name}&rdquo;
        </span>
      </div>

      {/* Progress comparison */}
      <div className="grid grid-cols-2 gap-3 text-xs">
        <div className="space-y-1">
          <span className="text-muted-foreground">Existing progress</span>
          <p className="font-medium">{duplicateInfo.existing_progress || 'None'}</p>
        </div>
        <div className="space-y-1">
          <span className="text-muted-foreground">New file progress</span>
          <p className="font-medium">{newProgress || 'None'}</p>
        </div>
      </div>

      {/* Action selection */}
      <div className="space-y-2">
        {(Object.keys(ACTION_LABELS) as DuplicateAction[]).map((action) => (
          <label
            key={action}
            className="flex items-start gap-2 cursor-pointer"
          >
            <input
              type="radio"
              name={`duplicate-action-${duplicateInfo.existing_file_id}`}
              checked={selectedAction === action}
              onChange={() => onActionChange(action)}
              className="mt-0.5 accent-primary"
            />
            <div>
              <span className="text-sm font-medium block">
                {ACTION_LABELS[action].label}
              </span>
              <span className="text-xs text-muted-foreground block">
                {ACTION_LABELS[action].description}
              </span>
            </div>
          </label>
        ))}
      </div>
    </div>
  );
}
