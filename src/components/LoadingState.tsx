import { Loader2 } from 'lucide-react';

interface LoadingStateProps {
  message?: string;
  className?: string;
}

export function LoadingState({ message = 'Loading...', className = '' }: LoadingStateProps) {
  return (
    <div className={`flex items-center justify-center gap-2 p-8 ${className}`}>
      <Loader2 className="h-6 w-6 animate-spin" aria-hidden="true" />
      <span className="text-muted-foreground">{message}</span>
    </div>
  );
}
