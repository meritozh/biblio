import { useState } from 'react';
import {
  Palette,
  FolderCog,
  Sparkles,
  Bug,
  type LucideIcon,
} from 'lucide-react';
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog';
import { Separator } from '@/components/ui/separator';
import { cn } from '@/lib/utils';
import { StoragePathSetting } from './StoragePathSetting';
import { ThemeSetting } from './ThemeSetting';
import { AiSetting } from './AiSetting';
import { RemoteStorageSetting } from './RemoteStorageSetting';
import { DebugSetting } from './DebugSetting';

interface SettingsDialogProps {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

type SectionId = 'general' | 'storage' | 'ai' | 'debug';

const SECTIONS: ReadonlyArray<{ id: SectionId; label: string; icon: LucideIcon }> = [
  { id: 'general', label: 'General', icon: Palette },
  { id: 'storage', label: 'Storage', icon: FolderCog },
  { id: 'ai', label: 'AI', icon: Sparkles },
  { id: 'debug', label: 'Debug', icon: Bug },
];

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const [section, setSection] = useState<SectionId>('general');
  const activeLabel = SECTIONS.find((s) => s.id === section)?.label ?? 'Settings';

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-3xl h-[85vh] p-0 gap-0 overflow-hidden grid-rows-[1fr]">
        {/* DialogTitle is required for a11y; hidden visually because each pane has its own heading. */}
        <DialogTitle className="sr-only">Settings</DialogTitle>

        <div className="grid grid-cols-[180px_1fr] h-full min-h-0">
          <aside className="border-r bg-muted/40 flex flex-col py-4 min-h-0">
            <div className="px-4 pb-3">
              <h2 className="text-lg font-semibold leading-none tracking-tight">Settings</h2>
            </div>
            <nav className="flex flex-col gap-1 px-2 overflow-y-auto scrollbar-hidden">
              {SECTIONS.map(({ id, label, icon: Icon }) => {
                const active = section === id;
                return (
                  <button
                    key={id}
                    type="button"
                    onClick={() => setSection(id)}
                    className={cn(
                      'flex items-center gap-2 px-3 py-2 rounded-xl text-sm font-medium transition-colors duration-200 text-left',
                      active
                        ? 'bg-primary/10 text-primary'
                        : 'text-muted-foreground hover:bg-secondary hover:text-foreground'
                    )}
                  >
                    <Icon className="h-4 w-4" />
                    {label}
                  </button>
                );
              })}
            </nav>
          </aside>

          <div className="flex flex-col min-h-0">
            {/* pr-14 clears the dialog's absolute close button at top-right. */}
            <header className="px-6 pt-5 pb-3 pr-14">
              <h2 className="text-lg font-semibold leading-none tracking-tight">{activeLabel}</h2>
            </header>
            <div className="flex-1 overflow-y-auto scrollbar-hidden px-6 pb-6 space-y-6 min-h-0">
              {section === 'general' && <ThemeSetting />}

              {section === 'storage' && (
                <>
                  <StoragePathSetting />
                  <Separator />
                  <RemoteStorageSetting />
                </>
              )}

              {section === 'ai' && <AiSetting />}

              {section === 'debug' && <DebugSetting />}
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
