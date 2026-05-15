import { createRootRoute, Outlet, useNavigate, useRouterState } from '@tanstack/react-router';
import { useCallback, useEffect } from 'react';
import { CategorySidebar } from '@/components/CategorySidebar';
import { SettingsDialog } from '@/components/SettingsDialog';
import {
  loadCategories,
  setSelectedCategoryId,
  setSettingsOpen,
  useAppState,
} from '@/stores/appStore';

export const Route = createRootRoute({
  component: AppShell,
});

function AppShell() {
  const categories = useAppState((s) => s.categories);
  const selectedCategoryId = useAppState((s) => s.selectedCategoryId);
  const settingsOpen = useAppState((s) => s.settingsOpen);
  const pathname = useRouterState({ select: (s) => s.location.pathname });
  const navigate = useNavigate();

  useEffect(() => {
    void loadCategories();
  }, []);

  const handleCategorySelect = useCallback(
    (id: number | null) => {
      setSelectedCategoryId(id);
      if (pathname !== '/') void navigate({ to: '/' });
    },
    [pathname, navigate]
  );

  // Highlight the selected category only on Library; on management routes
  // the category list stays visible (so the user can jump back to a filter)
  // but no row is shown as "active".
  const onLibrary = pathname === '/';

  return (
    <div className="flex h-screen bg-background">
      <CategorySidebar
        categories={categories}
        selectedCategoryId={onLibrary ? selectedCategoryId : null}
        onCategorySelect={handleCategorySelect}
        onOpenSettings={() => setSettingsOpen(true)}
        currentPath={pathname}
      />
      <main className="flex-1 flex flex-col overflow-hidden relative">
        <Outlet />
      </main>
      <SettingsDialog
        open={settingsOpen}
        onOpenChange={(open) => setSettingsOpen(open)}
      />
    </div>
  );
}
