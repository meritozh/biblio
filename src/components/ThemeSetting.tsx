import { Sun, Moon, Monitor } from 'lucide-react';
import {
  useTheme,
  type ColorScheme,
  type DesignTheme,
} from '@/lib/useTheme';
import { cn } from '@/lib/utils';

const colorSchemes: { value: ColorScheme; label: string; icon: typeof Sun }[] = [
  { value: 'light', label: 'Light', icon: Sun },
  { value: 'dark', label: 'Dark', icon: Moon },
  { value: 'system', label: 'System', icon: Monitor },
];

const designThemes: {
  value: DesignTheme;
  label: string;
  description: string;
  swatches: [string, string, string];
}[] = [
  {
    value: 'porcelain',
    label: 'Porcelain Soft',
    description: 'Cool porcelain surfaces with mist-blue ink',
    swatches: ['#F4F6F8', '#3E5F73', '#D9E6EE'],
  },
  {
    value: 'graphite',
    label: 'Warm Graphite',
    description: 'Warm stone neutrals with soft sage accents',
    swatches: ['#F3F2EF', '#5C7260', '#E2E8E1'],
  },
];

export function ThemeSetting() {
  const { theme, setTheme, designTheme, setDesignTheme } = useTheme();

  return (
    <div className="space-y-8">
      <div className="space-y-3">
        <div>
          <h3 className="text-sm font-semibold">Theme</h3>
          <p className="text-xs text-muted-foreground">
            Choose a design system for surfaces, accent, and typography
          </p>
        </div>
        <div className="grid gap-2 sm:grid-cols-2">
          {designThemes.map(({ value, label, description, swatches }) => {
            const selected = designTheme === value;
            return (
              <button
                key={value}
                type="button"
                onClick={() => setDesignTheme(value)}
                className={cn(
                  'flex flex-col gap-3 rounded-xl border p-3 text-left transition-colors duration-200',
                  selected
                    ? 'border-primary/40 bg-primary/10 ring-1 ring-primary/25'
                    : 'border-border/70 bg-card hover:bg-secondary/60'
                )}
                aria-pressed={selected}
                aria-label={`Set theme to ${label}`}
              >
                <div className="flex items-center gap-1.5">
                  {swatches.map((hex) => (
                    <span
                      key={hex}
                      className="h-5 w-5 rounded-full border border-black/5"
                      style={{ backgroundColor: hex }}
                      aria-hidden="true"
                    />
                  ))}
                </div>
                <div>
                  <div className="text-sm font-semibold text-foreground">{label}</div>
                  <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
                </div>
              </button>
            );
          })}
        </div>
      </div>

      <div className="space-y-3">
        <div>
          <h3 className="text-sm font-semibold">Appearance</h3>
          <p className="text-xs text-muted-foreground">Light, dark, or follow the system</p>
        </div>
        <div className="flex gap-2">
          {colorSchemes.map(({ value, label, icon: Icon }) => (
            <button
              key={value}
              type="button"
              onClick={() => setTheme(value)}
              className={cn(
                'flex items-center gap-2 rounded-xl px-3 py-2 text-sm font-medium transition-colors duration-200',
                theme === value
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-secondary text-secondary-foreground hover:bg-secondary/80'
              )}
              aria-pressed={theme === value}
              aria-label={`Set appearance to ${label}`}
            >
              <Icon className="h-4 w-4" aria-hidden="true" />
              {label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
