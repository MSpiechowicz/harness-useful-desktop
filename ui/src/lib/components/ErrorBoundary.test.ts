import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import BoundaryProbe from '../../../tests/fixtures/BoundaryProbe.svelte';

afterEach(() => vi.restoreAllMocks());

describe('ErrorBoundary', () => {
  it('shows "Something went wrong" in place of a part that throws, and keeps the rest working', () => {
    const log = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(BoundaryProbe, { broken: () => true });

    expect(screen.getByRole('alert')).toHaveTextContent('Something went wrong');
    expect(screen.getByText('The Ecosystem page hit an unexpected error. The rest of the app still works.')).toBeInTheDocument();
    expect(screen.getByText('Outside the boundary')).toBeInTheDocument();
    expect(screen.queryByText('Page content')).not.toBeInTheDocument();
    expect(log).toHaveBeenCalledWith('[harness] The Ecosystem page failed while rendering', expect.any(Error));
  });

  it('shows the error on request and renders the part again on "Try again"', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    let broken = true;
    render(BoundaryProbe, { broken: () => broken });

    await fireEvent.click(screen.getByRole('button', { name: 'Error details' }));
    expect(screen.getByText('Error: each_key_duplicate: Keyed each block has duplicate key')).toBeInTheDocument();

    broken = false;
    await fireEvent.click(screen.getByRole('button', { name: 'Try again' }));

    expect(await screen.findByText('Page content')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('drops a decorative layer silently', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    render(BoundaryProbe, { broken: () => true, variant: 'silent' });

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByText('Outside the boundary')).toBeInTheDocument();
  });
});
