import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import Pagination from './Pagination.svelte';

describe('Pagination', () => {
  it('says which entries are on the page and turns it with Previous, Next and the page numbers', async () => {
    const onpage = vi.fn();
    render(Pagination, { page: 1, pageSize: 50, total: 1234, onpage });

    expect(screen.getByText('51–100 of 1,234')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Page 2' })).toHaveAttribute('aria-current', 'page');
    expect(screen.getByRole('button', { name: 'Page 25' })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Previous' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Page 25' }));

    expect(onpage.mock.calls).toEqual([[2], [0], [24]]);
  });

  it('disables Previous on the first page and Next on the last, and cuts the last page short', () => {
    const { unmount } = render(Pagination, { page: 0, pageSize: 50, total: 120, onpage: () => {} });
    expect(screen.getByRole('button', { name: 'Previous' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Next' })).toBeEnabled();
    unmount();

    render(Pagination, { page: 2, pageSize: 50, total: 120, onpage: () => {} });
    expect(screen.getByText('101–120 of 120')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Next' })).toBeDisabled();
  });

  it('shows only the range for a list that fits one page', () => {
    render(Pagination, { page: 0, pageSize: 50, total: 12, onpage: () => {} });

    expect(screen.getByText('1–12 of 12')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Next' })).toBeNull();
  });
});
