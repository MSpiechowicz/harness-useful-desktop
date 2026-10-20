import { cleanup, fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';

import ScopeSelect from './ScopeSelect.svelte';
import type { ScopeTarget } from './scope-options';

const PROJECTS: ProjectScope[] = [
  { path: '/work/widget', name: 'widget', root: '/work/widget', key: 'git:github.com/acme/widget' },
  { path: '/work/alpha', name: 'Alpha', root: '/work/alpha', key: 'git:github.com/acme/alpha' },
];

afterEach(cleanup);

function show(value: ScopeTarget, extra: Record<string, unknown> = {}) {
  const onchange = vi.fn();
  render(ScopeSelect, { label: 'Applies to', labelId: 'scope-label', projects: PROJECTS, value, onchange, ...extra });

  return onchange;
}

async function openMenu(): Promise<HTMLElement> {
  await fireEvent.click(screen.getByRole('button', { name: 'Applies to' }));

  return screen.findByRole('menu', { name: 'Applies to' });
}

describe('the Applies to field', () => {
  it('shows the scope it holds under its label, with the repository of a project next to the name', () => {
    show({ kind: 'repo', path: '/work/widget' }, { help: 'Where it applies.' });

    expect(screen.getByRole('button', { name: 'Applies to' })).toHaveTextContent('widget acme/widget');
    expect(screen.getByText('Where it applies.')).toBeInTheDocument();
  });

  it('lists Global and the projects on this computer, and finds one by what is typed', async () => {
    show({ kind: 'global' });
    const menu = await openMenu();
    const items = () => within(menu).getAllByRole('menuitemradio').map((item) => item.textContent?.trim());

    expect(items()).toEqual(['Global', 'Alpha acme/alpha', 'widget acme/widget']);

    await fireEvent.input(within(menu).getByRole('searchbox', { name: 'Search repositories' }), { target: { value: 'wid' } });
    expect(items()).toEqual(['widget acme/widget']);

    await fireEvent.input(within(menu).getByRole('searchbox', { name: 'Search repositories' }), { target: { value: 'zzz' } });
    expect(within(menu).getByText('No repository matches')).toBeInTheDocument();
  });

  it('hands over the folder of the project chosen, or Global', async () => {
    const onchange = show({ kind: 'global' });
    const menu = await openMenu();

    await fireEvent.click(within(menu).getByRole('menuitemradio', { name: /^Alpha/ }));
    expect(onchange).toHaveBeenCalledWith({ kind: 'repo', path: '/work/alpha' });

    cleanup();
    const back = show({ kind: 'repo', path: '/work/alpha' });
    await fireEvent.click(within(await openMenu()).getByRole('menuitemradio', { name: 'Global' }));

    expect(back).toHaveBeenCalledWith({ kind: 'global' });
  });

  it('does not open when the scope is fixed', async () => {
    show({ kind: 'repo', path: '/work/widget' }, { disabled: true });

    expect(screen.getByRole('button', { name: 'Applies to' })).toBeDisabled();
    await fireEvent.click(screen.getByRole('button', { name: 'Applies to' }));
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });

  it('shows an error in place of the help', () => {
    show({ kind: 'global' }, { help: 'Where it applies.', error: 'Pick a repository.' });

    expect(screen.getByRole('alert')).toHaveTextContent('Pick a repository.');
    expect(screen.queryByText('Where it applies.')).not.toBeInTheDocument();
  });
});
