import { fireEvent, render, screen, within } from '@testing-library/svelte';
import type { ComponentProps } from 'svelte';
import { describe, expect, it, vi } from 'vitest';

import SplitButton from './SplitButton.svelte';

function setup(props: Partial<ComponentProps<typeof SplitButton>> = {}) {
  const main = vi.fn();
  const merge = vi.fn();

  render(SplitButton, {
    label: 'Rebase on main',
    menuLabel: 'More ways to update',
    onclick: main,
    items: [
      { id: 'rebase', label: 'Rebase on main', onselect: main },
      { id: 'merge', label: 'Merge main', description: 'Adds one merge commit', onselect: merge },
    ],
    ...props,
  });

  return { main, merge };
}

describe('SplitButton', () => {
  it('runs the main action from the button and the others from the menu', async () => {
    const { main, merge } = setup();

    await fireEvent.click(screen.getByRole('button', { name: 'Rebase on main' }));
    expect(main).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('menu')).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'More ways to update' }));
    await fireEvent.click(within(screen.getByRole('menu')).getByRole('menuitem', { name: /Merge main/ }));

    expect(merge).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('closes the menu with Escape and returns to the chevron, without letting the panel around it see the key', async () => {
    const outer = vi.fn();
    document.addEventListener('keydown', outer);
    setup();
    const chevron = screen.getByRole('button', { name: 'More ways to update' });

    await fireEvent.click(chevron);
    await fireEvent.keyDown(screen.getByRole('menuitem', { name: 'Rebase on main' }), { key: 'Escape' });

    expect(screen.queryByRole('menu')).toBeNull();
    expect(chevron).toHaveFocus();
    expect(outer).not.toHaveBeenCalled();
    document.removeEventListener('keydown', outer);
  });

  it('closes the menu on a press outside it', async () => {
    setup();

    await fireEvent.click(screen.getByRole('button', { name: 'More ways to update' }));
    await fireEvent.pointerDown(document.body);

    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('turns both buttons off together while disabled or working', () => {
    setup({ disabled: true });
    expect(screen.getByRole('button', { name: 'Rebase on main' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'More ways to update' })).toBeDisabled();
  });

  it('keeps the other button off while the main action works', () => {
    setup({ loading: true });

    expect(screen.getByRole('button', { name: 'More ways to update' })).toBeDisabled();
  });
});
