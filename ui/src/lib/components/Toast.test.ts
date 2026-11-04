import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { ToastItem } from '$lib/stores/toasts.svelte';

import Toast from './Toast.svelte';

const toast = (patch: Partial<ToastItem> = {}): ToastItem => ({ id: 1, title: 'Hook saved', description: 'Nightly check was saved.', tone: 'success', duration: 4000, ...patch });

describe('Toast', () => {
  it('shows the title with the description under it', () => {
    render(Toast, { props: { toast: toast(), ondismiss: () => {} } });

    const title = screen.getByText('Hook saved');
    const description = screen.getByText('Nightly check was saved.');

    expect(title.compareDocumentPosition(description) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('says there are no more details instead of leaving the line empty', () => {
    render(Toast, { props: { toast: toast({ title: 'Could not delete the note', description: '' }), ondismiss: () => {} } });

    expect(screen.getByText('No more details were given.')).toBeInTheDocument();
  });

  it('treats a description of only spaces as empty', () => {
    render(Toast, { props: { toast: toast({ description: '  \n ' }), ondismiss: () => {} } });

    expect(screen.getByText('No more details were given.')).toBeInTheDocument();
  });
});
