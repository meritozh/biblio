import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { StoragePathSetting } from './StoragePathSetting';
import { ThemeSetting } from './ThemeSetting';
import { AiSetting } from './AiSetting';
import { Separator } from '@/components/ui/separator';

interface SettingsDialogProps {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md max-h-[85vh]">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
        </DialogHeader>
        <div className="py-4 space-y-6 overflow-y-auto max-h-[calc(85vh-4rem)] scrollbar-hidden">
          <ThemeSetting />
          <Separator />
          <StoragePathSetting />
          <Separator />
          <AiSetting />
        </div>
      </DialogContent>
    </Dialog>
  );
}
