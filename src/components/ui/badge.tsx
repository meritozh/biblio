import * as React from 'react';
import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '@/lib/utils';

const badgeVariants = cva(
  'inline-flex items-center rounded-xl px-2.5 py-1 text-xs font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2',
  {
    variants: {
      variant: {
        default: 'bg-primary/10 text-primary',
        secondary: 'bg-secondary text-secondary-foreground',
        destructive: 'bg-destructive/10 text-destructive',
        outline: 'border border-input text-foreground',
        blue: 'bg-notion-blue-muted text-notion-blue',
        purple: 'bg-notion-purple-muted text-notion-purple',
        pink: 'bg-notion-pink-muted text-notion-pink',
        green: 'bg-notion-green-muted text-notion-green',
        orange: 'bg-notion-orange-muted text-notion-orange',
        yellow: 'bg-notion-yellow-muted text-notion-yellow',
        gray: 'bg-notion-gray-muted text-muted-foreground',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  }
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>, VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return <div className={cn(badgeVariants({ variant }), className)} {...props} />;
}

export { Badge, badgeVariants };
