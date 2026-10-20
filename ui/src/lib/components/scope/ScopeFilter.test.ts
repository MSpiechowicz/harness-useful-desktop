import { cleanup, fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';

import ScopeFilter from './ScopeFilter.svelte';
import { buildScopeOptions, type ScopeChoice } from './scope-options';

const LABELS = { all: 'All', global: 'Global', remoteOnly: 'Remote only' };

const PROJECTS: ProjectScope[] = [
  { path: '/work/widget', name: 'widget', root: '/work/widget', key: 'git:github.com/acme/widget' },
  { path: '/tmp/scratch', name: 'scratch', root: null, key: null },
];

const OPTIONS = buildScopeOptions({
  projects: PROJECTS,
  remote: [
    { key: 'git:github.com/acme/widget', label: 'github.com/acme/widget', count: 5 },
    { key: 'git:github.com/zeta/last', label: 'github.com/zeta/last', count: 2 },
  ],
  globalCount: 3,
  allCount: 10,
  labels: LABELS,
});

afterEach(cleanup);

async function openMenu(value: ScopeChoice = 'all') {
  const onchange = vi.fn();
  const { rerender } = render(ScopeFilter, { options: OPTIONS, value, onchange });

  await fireEvent.click(screen.getByRole('button', { name: /^Scope:/ }));

  return { menu: await screen.findByRole('menu', { name: 'Scope' }), onchange, rerender };
}

function labels(menu: HTMLElement): (string | undefined)[] {
  return within(menu).getAllByRole('menuitemradio').map((item) => item.textContent?.replace(/\s+/g, ' ').trim());
}

describe('the Scope filter', () => {
  it('reads "Scope: All" and lists All, the projects on this computer, the repositories only the source has, and Global', async () => {
    const { menu } = await openMenu();

    expect(screen.getByRole('button', { name: 'Scope: All' })).toBeInTheDocument();
    expect(labels(menu)).toEqual(['All 10', 'scratch 0', 'widget acme/widget 5', 'zeta/last Remote only 2', 'Global 3']);
  });

  it('keeps the entries whose name or hint has what is typed and says when none does', async () => {
    const { menu } = await openMenu();
    const search = within(menu).getByRole('searchbox', { name: 'Search repositories' });

    await fireEvent.input(search, { target: { value: 'REMOTE' } });
    expect(labels(menu)).toEqual(['zeta/last Remote only 2']);

    await fireEvent.input(search, { target: { value: 'acme' } });
    expect(labels(menu)).toEqual(['widget acme/widget 5']);

    await fireEvent.input(search, { target: { value: 'nothing' } });
    expect(within(menu).queryAllByRole('menuitemradio')).toHaveLength(0);
    expect(within(menu).getByText('No repository matches')).toBeInTheDocument();
  });

  it('hands over the choice and goes back to All with the clear button', async () => {
    const { menu, onchange } = await openMenu();

    await fireEvent.click(within(menu).getByRole('menuitemradio', { name: /^zeta\/last/ }));
    expect(onchange).toHaveBeenCalledWith('repo:git:github.com/zeta/last');

    cleanup();
    const again = vi.fn();
    render(ScopeFilter, { options: OPTIONS, value: 'global', onchange: again });
    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Scope filter' }));

    expect(screen.getByRole('button', { name: 'Scope: Global' })).toBeInTheDocument();
    expect(again).toHaveBeenCalledWith('all');
  });

  it('shows a project without a repository key but does not let it be chosen where filtering needs a key', async () => {
    const onchange = vi.fn();
    render(ScopeFilter, { options: OPTIONS, value: 'all', onchange, disableKeyless: true });
    await fireEvent.click(screen.getByRole('button', { name: /^Scope:/ }));
    const menu = await screen.findByRole('menu', { name: 'Scope' });

    expect(within(menu).getByRole('menuitemradio', { name: /^scratch/ })).toBeDisabled();
    expect(within(menu).getByRole('menuitemradio', { name: /^widget/ })).toBeEnabled();

    await fireEvent.click(within(menu).getByRole('menuitemradio', { name: /^scratch/ }));
    expect(onchange).not.toHaveBeenCalled();
  });
});
