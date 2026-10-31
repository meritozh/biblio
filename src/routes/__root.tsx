import { createRootRoute, Outlet, useNavigate, useRouterState } from '@tanstack/react-router';
import { useCallback, useEffect, useRef, useState } from 'react';
import { PanelLeft } from 'lucide-react';
import { CategorySidebar } from '@/components/CategorySidebar';
import { SettingsDialog } from '@/components/SettingsDialog';
import { Button } from '@/components/ui/button';
import {
  loadCategories,
  setSelectedCategoryId,
  setSettingsOpen,
  useAppState,
} from '@/stores/appStore';
import { loadSchemas } from '@/stores/schemaStore';

export const Route = createRootRoute({
  component: AppShell,
});

function AppShell() {
  const categories = useAppState((s) => s.categories);
  const selectedCategoryId = useAppState((s) => s.selectedCategoryId);
  const settingsOpen = useAppState((s) => s.settingsOpen);
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const navigate = useNavigate();
  const [mobileOpen, setMobileOpen] = useState(false);
  const menuButtonRef = useRef<HTMLButtonElement>(null);
  const mobileNavRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void loadSchemas();
    void loadCategories();
  }, []);

  useEffect(() => {
    setMobileOpen(false);
  }, [pathname]);

  useEffect(() => {
    const media = window.matchMedia('(min-width: 768px)');
    const handleChange = (event: MediaQueryListEvent) => {
      if (event.matches) setMobileOpen(false);
    };
    if (media.addEventListener) media.addEventListener('change', handleChange);
    else media.addListener(handleChange);
    return () => {
      if (media.removeEventListener) media.removeEventListener('change', handleChange);
      else media.removeListener(handleChange);
    };
  }, []);

  useEffect(() => {
    if (!mobileOpen) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setMobileOpen(false);
        requestAnimationFrame(() => menuButtonRef.current?.focus());
        return;
      }
      if (event.key !== 'Tab') return;
      const focusable = mobileNavRef.current?.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), [tabindex="0"]'
      );
      if (!focusable?.length) return;
      const first = focusable[0]!;
      const last = focusable[focusable.length - 1]!;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [mobileOpen]);

  const handleCategorySelect = useCallback(
    (id: number | null) => {
      setSelectedCategoryId(id);
      if (pathname !== '/') void navigate({ to: '/' });
    },
    [pathname, navigate]
  );

  const closeMobileNav = useCallback(() => {
    setMobileOpen(false);
    requestAnimationFrame(() => menuButtonRef.current?.focus());
  }, []);

  const onLibrary = pathname === '/';
  const sidebarProps = {
    categories,
    selectedCategoryId: onLibrary ? selectedCategoryId : null,
    onCategorySelect: handleCategorySelect,
    currentPath: pathname,
  };

  return (
    <div className="flex h-screen bg-background supports-[height:100dvh]:h-dvh">
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:left-3 focus:top-3 focus:z-[70] focus:rounded-md focus:bg-card focus:px-3 focus:py-2 focus:text-sm focus:shadow-lg focus:outline-none focus:ring-2 focus:ring-ring"
      >
        Skip to main content
      </a>
      <CategorySidebar
        {...sidebarProps}
        className="hidden shrink-0 md:flex"
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <main
        id="main-content"
        tabIndex={-1}
        inert={mobileOpen ? true : undefined}
        className="relative flex min-w-0 flex-1 flex-col overflow-hidden outline-none"
      >
        <div className="flex h-12 shrink-0 items-center gap-2 border-b border-border/80 px-3 md:hidden">
          <Button
            ref={menuButtonRef}
            variant="ghost"
            size="icon"
            aria-label="Open navigation"
            aria-expanded={mobileOpen}
            aria-controls="mobile-navigation"
            onClick={() => setMobileOpen(true)}
          >
            <PanelLeft aria-hidden="true" />
          </Button>
          <span className="text-sm font-semibold">Biblio</span>
        </div>
        <Outlet />
      </main>
      {mobileOpen && (
        <div className="fixed inset-0 z-40 md:hidden">
          <button
            type="button"
            className="absolute inset-0 bg-black/25"
            aria-label="Close navigation"
            onClick={closeMobileNav}
          />
          <div
            ref={mobileNavRef}
            id="mobile-navigation"
            role="dialog"
            aria-modal="true"
            aria-label="Navigation"
            className="relative h-full w-[272px] max-w-[85vw] bg-sidebar shadow-[12px_0_36px_rgba(0,0,0,0.18)] animate-in slide-in-from-left-4 fade-in duration-200"
          >
            <CategorySidebar
              {...sidebarProps}
              autoFocus
              resizable={false}
              className="flex h-full w-full"
              onNavigate={closeMobileNav}
              onOpenSettings={() => {
                setMobileOpen(false);
                setSettingsOpen(true);
              }}
            />
          </div>
        </div>
      )}
      <SettingsDialog open={settingsOpen} onOpenChange={(open) => setSettingsOpen(open)} />
    </div>
  );
}
