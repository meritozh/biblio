import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';

interface PageHeaderProps {
  icon: LucideIcon;
  title: string;
  subtitle?: string;
  actions?: ReactNode;
}

export function PageHeader({ icon: Icon, title, subtitle, actions }: PageHeaderProps) {
  return (
    <header
      className="flex flex-col gap-3 border-b border-border/80 px-5 pb-4 pt-5 md:px-6 md:pb-3 md:pt-9 lg:min-h-[76px] lg:flex-row lg:items-center lg:justify-between"
      data-tauri-drag-region
    >
      <div className="flex min-w-0 items-center gap-2.5" data-tauri-drag-region>
        <Icon
          className="h-[18px] w-[18px] shrink-0 text-muted-foreground"
          strokeWidth={1.8}
          aria-hidden="true"
        />
        <h1 className="truncate text-[22px] font-semibold leading-7 text-foreground">{title}</h1>
        {subtitle && (
          <span className="shrink-0 text-xs text-muted-foreground [font-variant-numeric:tabular-nums]">
            {subtitle}
          </span>
        )}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2 lg:justify-end">{actions}</div>}
    </header>
  );
}
