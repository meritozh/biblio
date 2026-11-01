import { useState, useEffect, useCallback } from 'react';

/** Light / dark appearance preference. */
export type ColorScheme = 'light' | 'dark' | 'system';

/** Design-system palette shown in Settings → Theme. */
export type DesignTheme = 'porcelain' | 'graphite';

/** @deprecated Use ColorScheme. Kept as an alias for older imports. */
export type Theme = ColorScheme;

const COLOR_SCHEME_KEY = 'theme';
const DESIGN_THEME_KEY = 'design-theme';

function getSystemColorScheme(): 'light' | 'dark' {
  if (typeof window === 'undefined') return 'light';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function getStoredColorScheme(): ColorScheme {
  if (typeof window === 'undefined') return 'system';
  const stored = localStorage.getItem(COLOR_SCHEME_KEY);
  if (stored === 'light' || stored === 'dark' || stored === 'system') {
    return stored;
  }
  return 'system';
}

function getStoredDesignTheme(): DesignTheme {
  if (typeof window === 'undefined') return 'porcelain';
  const stored = localStorage.getItem(DESIGN_THEME_KEY);
  if (stored === 'porcelain' || stored === 'graphite') {
    return stored;
  }
  return 'porcelain';
}

function applyAppearance(colorScheme: ColorScheme, designTheme: DesignTheme) {
  const root = document.documentElement;
  const effective = colorScheme === 'system' ? getSystemColorScheme() : colorScheme;

  root.dataset.theme = designTheme;

  if (effective === 'dark') {
    root.classList.add('dark');
  } else {
    root.classList.remove('dark');
  }
}

export function useTheme() {
  const [theme, setThemeState] = useState<ColorScheme>(getStoredColorScheme);
  const [designTheme, setDesignThemeState] = useState<DesignTheme>(getStoredDesignTheme);

  const setTheme = useCallback(
    (newTheme: ColorScheme) => {
      setThemeState(newTheme);
      localStorage.setItem(COLOR_SCHEME_KEY, newTheme);
      applyAppearance(newTheme, designTheme);
    },
    [designTheme]
  );

  const setDesignTheme = useCallback(
    (next: DesignTheme) => {
      setDesignThemeState(next);
      localStorage.setItem(DESIGN_THEME_KEY, next);
      applyAppearance(theme, next);
    },
    [theme]
  );

  useEffect(() => {
    applyAppearance(theme, designTheme);

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleChange = () => {
      if (theme === 'system') {
        applyAppearance('system', designTheme);
      }
    };

    mediaQuery.addEventListener('change', handleChange);
    return () => mediaQuery.removeEventListener('change', handleChange);
  }, [theme, designTheme]);

  return { theme, setTheme, designTheme, setDesignTheme };
}

export function initTheme() {
  applyAppearance(getStoredColorScheme(), getStoredDesignTheme());
}
