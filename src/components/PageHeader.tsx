/**
 * The shared page masthead — lucide icon + serif display title +
 * optional italic count, with a right-aligned action row. Every route
 * composes this instead of re-typing the `px-8 pt-14 pb-5 border-b`
 * markup, so headers stay visually identical app-wide.
 */
import type { LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';

interface PageHeaderProps {
  icon: LucideIcon;
  title: string;
  /** Serif-italic count or hint beside the title, e.g. "— 3 schemas". */
  subtitle?: string;
  /** Right-aligned controls; rendered in a gap-2 row. */
  actions?: ReactNode;
}

export function PageHeader({ icon: Icon, title, subtitle, actions }: PageHeaderProps) {
  return (
    <div
      className="flex items-end justify-between px-8 pt-14 pb-5 border-b border-border"
      data-tauri-drag-region
    >
      <div className="flex items-baseline gap-3">
        <h1 className="text-3xl text-foreground flex items-center gap-3">
          <Icon className="h-6 w-6 text-muted-foreground" aria-hidden="true" />
          {title}
        </h1>
        {subtitle && (
          <span className="font-serif-italic text-sm text-muted-foreground">{subtitle}</span>
        )}
      </div>
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </div>
  );
}
