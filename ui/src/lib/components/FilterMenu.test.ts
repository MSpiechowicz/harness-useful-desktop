import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Plus from '@lucide/svelte/icons/plus';
import { describe, expect, it, vi } from 'vitest';

import FilterMenu from './FilterMenu.svelte';

const REASON = 'Teammates could not find this folder';

const options = [
  { value: 'a', label: 'Alpha' },
  { value: 'b', label: 'Beta', disabled: true, reason: REASON },
  { value: 'c', label: 'Gamma' },
];

async function open(onchange: (value: string) => void) {
  render(FilterMenu, { label: 'Pick', options, value: '', onchange, action: { label: 'Pick one', icon: Plus } });
  await fireEvent.click(screen.getByRole('button', { name: 'Pick one' }));
}

describe('a choice that is off', () => {
  it('is reached by the arrow keys, says why, and does nothing when it is chosen', async () => {
    const onchange = vi.fn();
    await open(onchange);

    const alpha = await screen.findByRole('menuitem', { name: 'Alpha' });
    await waitFor(() => expect(alpha).toHaveFocus());

    await fireEvent.keyDown(alpha, { key: 'ArrowDown' });
    const beta = screen.getByRole('menuitem', { name: new RegExp('Beta') });

    expect(beta).toHaveFocus();
    expect(beta).toHaveAttribute('aria-disabled', 'true');
    expect(beta).not.toBeDisabled();
    expect(beta).toHaveAccessibleDescription(REASON);

    await fireEvent.keyDown(beta, { key: 'Enter' });
    await fireEvent.click(beta);

    expect(onchange).not.toHaveBeenCalled();
    expect(screen.getByRole('menu')).toBeInTheDocument();

    await fireEvent.keyDown(beta, { key: 'ArrowDown' });
    expect(screen.getByRole('menuitem', { name: 'Gamma' })).toHaveFocus();

    await fireEvent.click(screen.getByRole('menuitem', { name: 'Gamma' }));
    expect(onchange).toHaveBeenCalledExactlyOnceWith('c');
  });

  it('does not take the focus when the menu opens while an enabled choice follows', async () => {
    render(FilterMenu, { label: 'Pick', options: [{ ...options[1] }, options[0]], value: '', onchange: () => {}, action: { label: 'Pick one', icon: Plus } });
    await fireEvent.click(screen.getByRole('button', { name: 'Pick one' }));

    await waitFor(() => expect(screen.getByRole('menuitem', { name: 'Alpha' })).toHaveFocus());
  });
});

describe('an action menu that is off', () => {
  it('stays focusable with its reason as the description and does not open', async () => {
    render(FilterMenu, {
      label: 'Pick',
      options,
      value: '',
      onchange: () => {},
      action: { label: 'Pick one', icon: Plus },
      disabled: true,
      describedby: 'why',
    });

    const why = document.createElement('p');
    why.id = 'why';
    why.textContent = 'Ask an admin';
    document.body.append(why);

    const button = screen.getByRole('button', { name: 'Pick one' });
    button.focus();

    expect(button).toHaveFocus();
    expect(button).toHaveAttribute('aria-disabled', 'true');
    expect(button).not.toBeDisabled();
    expect(button).toHaveAccessibleDescription('Ask an admin');

    await fireEvent.click(button);
    expect(screen.queryByRole('menu')).toBeNull();
  });
});
