import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import type { ComponentProps, ReactNode } from 'react';
import { CategorySidebar } from '@/components/CategorySidebar';

vi.mock('@tanstack/react-router', () => ({
  Link: ({ to, children, ...props }: ComponentProps<'a'> & { to: string; children: ReactNode }) => (
    <a href={to} {...props}>
      {children}
    </a>
  ),
}));

describe('CategorySidebar', () => {
  it('keeps favorites out of standalone navigation', () => {
    render(
      <CategorySidebar
        categories={[]}
        selectedCategoryId={null}
        onCategorySelect={() => {}}
        currentPath="/"
      />
    );

    expect(screen.queryByRole('link', { name: /favorites/i })).not.toBeInTheDocument();
    expect(screen.getByRole('link', { name: /categories/i })).toBeInTheDocument();
  });
});
