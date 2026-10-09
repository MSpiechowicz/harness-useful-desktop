import { fireEvent, render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import Tooltip from './Tooltip.svelte';

const trigger = createRawSnippet(() => ({ render: () => '<button type="button">Trigger</button>' }));

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('Tooltip', () => {
  it('renders outside a clipping ancestor so an overflow-hidden dialog cannot cut it off', async () => {
    const clipper = document.createElement('div');
    clipper.style.overflow = 'hidden';
    document.body.append(clipper);

    render(Tooltip, { props: { text: 'Close shortcuts', children: trigger }, target: clipper });
    await fireEvent.pointerEnter(screen.getByRole('button', { name: 'Trigger' }).parentElement as HTMLElement);

    const tooltip = await screen.findByRole('tooltip');

    expect(tooltip).toHaveTextContent('Close shortcuts');
    expect(clipper).not.toContainElement(tooltip);
    expect(tooltip.parentElement).toBe(document.body);
  });

  describe('keyboard focus', () => {
    // happy-dom has no :focus-visible; this focuses the trigger as if the user had been using the keyboard.
    async function focusByKeyboard(button: HTMLElement): Promise<void> {
      const matches = vi.spyOn(HTMLElement.prototype, 'matches').mockImplementation((selector) => selector === ':focus-visible');

      await fireEvent.focusIn(button);
      matches.mockRestore();
    }

    it('shows for a Tab that lands on the trigger', async () => {
      vi.useFakeTimers();
      render(Tooltip, { props: { text: 'Close shortcuts', children: trigger } });
      const button = screen.getByRole('button', { name: 'Trigger' });
      vi.advanceTimersByTime(1000);

      await focusByKeyboard(button);

      expect(await screen.findByRole('tooltip')).toHaveTextContent('Close shortcuts');
    });

    it('stays quiet when the app moves focus onto it as soon as it appears', async () => {
      vi.useFakeTimers();
      render(Tooltip, { props: { text: 'Close shortcuts', children: trigger } });
      const button = screen.getByRole('button', { name: 'Trigger' });

      await focusByKeyboard(button);

      expect(screen.queryByRole('tooltip')).toBeNull();
    });
  });
});
