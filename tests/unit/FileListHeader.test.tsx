import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { FileListHeader, type SortKey } from '@/components/FileListHeader';
import type { Condition } from '@/lib/filters';

function renderHeader(overrides: Partial<React.ComponentProps<typeof FileListHeader>> = {}) {
  const props: React.ComponentProps<typeof FileListHeader> = {
    showCollections: false,
    view: {
      viewMode: 'flat',
      available: false,
    },
    sort: {
      sortBy: 'name' as SortKey,
      sortDesc: false,
      setSortBy: vi.fn(),
      setSortDesc: vi.fn(),
    },
    filter: {
      conditions: [] as Condition[],
      setConditions: vi.fn(),
      filterOpen: false,
      setFilterOpen: vi.fn(),
      removeCondition: vi.fn(),
      availableTags: [],
      availableAuthors: [],
      tagsById: new Map(),
      authorsById: new Map(),
    },
    selection: {
      selectionMode: false,
      selectedCount: 0,
      visibleCount: 1,
      enterSelectionMode: vi.fn(),
      exitSelectionMode: vi.fn(),
      clearSelection: vi.fn(),
      selectFirstN: vi.fn(),
    },
    bulk: {
      remoteEnabled: false,
      canDownload: false,
      canDelete: false,
      canClearCache: false,
      hasCacheableSelection: false,
      onUpload: vi.fn(),
      onDownload: vi.fn(),
      onDelete: vi.fn(),
      onClearCache: vi.fn(),
    },
    ...overrides,
  };

  return render(<FileListHeader {...props} />);
}

describe('FileListHeader breadcrumb', () => {
  it('renders the collection breadcrumb and calls back from the header', () => {
    const onBack = vi.fn();
    renderHeader({
      breadcrumb: {
        label: '秋子さんといっしょ31',
        onBack,
      },
    });

    const backButton = screen.getByRole('button', {
      name: /back to collections/i,
    });
    expect(backButton).toHaveTextContent('Collection');
    expect(backButton).toHaveTextContent('秋子さんといっしょ31');

    fireEvent.click(backButton);

    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
