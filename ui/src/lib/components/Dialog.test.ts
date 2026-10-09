import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';

import Dialog from './Dialog.svelte';

const body = createRawSnippet(() => ({
  render: () => '<div><button type="button">First action</button><button type="button">Last action</button></div>',
}));

function mountWithTrigger(open: boolean) {
  const trigger = document.createElement('button');
  trigger.textContent = 'Open';
  document.body.append(trigger);
  trigger.focus();

  const view = render(Dialog, { props: { open, title: 'Confirm change', children: body } });

  return { trigger, ...view };
}

describe('Dialog', () => {
  it('is an accessible modal named by its title and moves focus inside', async () => {
    mountWithTrigger(true);

    const dialog = await screen.findByRole('dialog', { name: 'Confirm change' });

    expect(dialog).toHaveAttribute('aria-modal', 'true');
    await waitFor(() => expect(dialog).toContainElement(document.activeElement as HTMLElement));
  });

  it('starts on the first action rather than the close button', async () => {
    mountWithTrigger(true);

    const first = await screen.findByRole('button', { name: 'First action' });

    await waitFor(() => expect(document.activeElement).toBe(first));
  });

  it('keeps Tab inside the dialog, wrapping at both ends', async () => {
    mountWithTrigger(true);
    const dialog = await screen.findByRole('dialog');
    const last = screen.getByRole('button', { name: 'Last action' });
    const close = screen.getByRole('button', { name: 'Close dialog' });

    last.focus();
    await fireEvent.keyDown(last, { key: 'Tab' });
    expect(document.activeElement).toBe(close);

    await fireEvent.keyDown(close, { key: 'Tab', shiftKey: true });
    expect(document.activeElement).toBe(last);
    expect(dialog).toContainElement(document.activeElement as HTMLElement);
  });

  it('closes on Escape and returns focus to the element that opened it', async () => {
    const { trigger } = mountWithTrigger(true);
    const dialog = await screen.findByRole('dialog');

    await fireEvent.keyDown(dialog, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(document.activeElement).toBe(trigger);
  });

  it('does not close on Escape when it is not dismissible', async () => {
    render(Dialog, { props: { open: true, title: 'Blocking', dismissible: false, children: body } });
    const blocking = await screen.findByRole('dialog', { name: 'Blocking' });

    await fireEvent.keyDown(blocking, { key: 'Escape' });

    expect(screen.getByRole('dialog', { name: 'Blocking' })).toBeInTheDocument();
  });
});
