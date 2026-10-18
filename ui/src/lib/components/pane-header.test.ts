import { fireEvent, render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it, vi } from 'vitest';

import PaneHeader from './PaneHeader.svelte';

function text(content: string) {
  return createRawSnippet(() => ({ render: () => `<span>${content}</span>` }));
}

describe('the pane header', () => {
  it('names a region by its title and shows the subtitle under it', () => {
    render(PaneHeader, { title: 'Settings', titleId: 'pane-title', subtitle: text('Global settings') });

    expect(screen.getByRole('heading', { level: 2, name: 'Settings' })).toHaveAttribute('id', 'pane-title');
    expect(screen.getByText('Global settings')).toBeInTheDocument();
  });

  it('gives a shortened title its whole name', () => {
    render(PaneHeader, { title: 'lib.rs', label: 'src/lib.rs', mono: true });

    expect(screen.getByRole('heading', { name: 'src/lib.rs' })).toHaveTextContent('lib.rs');
  });

  it('shows a close button only when it is given something to do, and the actions before it', async () => {
    const onclose = vi.fn();
    const { unmount } = render(PaneHeader, { title: 'Details' });
    expect(screen.queryByRole('button')).toBeNull();
    unmount();

    render(PaneHeader, { title: 'Details', onclose, actions: text('actions') });
    await fireEvent.click(screen.getByRole('button', { name: 'Close' }));

    expect(onclose).toHaveBeenCalledOnce();
    expect(screen.getByText('actions').compareDocumentPosition(screen.getByRole('button', { name: 'Close' })) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});
