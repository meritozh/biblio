import { Sun, Moon, Monitor } from 'lucide-react';
import { useTheme, type Theme } from '@/lib/useTheme';

const themes: { value: Theme; label: string; icon: typeof Sun }[] = [
  { value: 'light', label: 'Light', icon: Sun },
  { value: 'dark', label: 'Dark', icon: Moon },
  { value: 'system', label: 'System', icon: Monitor },
];

export function ThemeSetting() {
  const { theme, setTheme } = useTheme();

  return (
    <div className="space-y-3">
      <div>
        <h3 className="text-sm font-medium">Appearance</h3>
        <p className="text-xs text-muted-foreground">Choose your preferred theme</p>
      </div>
      <div className="flex gap-2">
        {themes.map(({ value, label, icon: Icon }) => (
          <button
            key={value}
            type="button"
            onClick={() => setTheme(value)}
            className={`
              flex items-center gap-2 px-3 py-2 rounded-xl text-sm font-medium
              transition-colors duration-200
              ${
                theme === value
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-secondary text-secondary-foreground hover:bg-secondary/80'
              }
            `}
            aria-pressed={theme === value}
            aria-label={`Set theme to ${label}`}
          >
            <Icon className="h-4 w-4" aria-hidden="true" />
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}
