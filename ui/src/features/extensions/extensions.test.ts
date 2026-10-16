import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import { i18n } from '$lib/i18n/i18n.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { extensionsView } from '$lib/stores/views.svelte';

import { samplePrefs } from '../../../tests/helpers';
import ExtensionDetail from './ExtensionDetail.svelte';
import { buildItems, filterItems, visibleTabs } from './extension-items';
import { groupFolds } from './group-folds.svelte';
import ExtensionsViewer from './ExtensionsViewer.svelte';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  await harness.load();
});

/** The group headings on screen, without their counts. */
const headings = () => screen.getAllByRole('button', { expanded: true }).concat(screen.queryAllByRole('button', { expanded: false })).map((button) => button.textContent?.replace(/\s*\d+$/, '').trim());

async function useHarness(id: 'omp' | 'pi' | 'opencode') {
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: id });
  groupFolds.reset();
  render(ExtensionsViewer);
  await screen.findByRole('navigation', { name: 'Extensions' });
}

describe('ExtensionsViewer', () => {
  it('lists everything in one inventory, grouped by the kinds OMP has', async () => {
    await useHarness('omp');
    const view = extensionsView.data!;

    expect(screen.queryByRole('tab')).toBeNull();
    expect(headings()).toEqual(expect.arrayContaining(['Plugins', 'Skills']));
    expect(headings()).not.toContain('Prompts');
    expect(within(screen.getByRole('region', { name: 'Plugins' })).getAllByRole('button')).toHaveLength(view.plugins.length + 1);
  });

  it('lists the kinds PI has instead', async () => {
    await useHarness('pi');

    expect(headings()).toContain('Prompts');
    expect(headings()).not.toContain('Plugins');
  });

  it('offers no install or update actions for OpenCode, whose extensions are read-only', async () => {
    prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'opencode' });
    render(ExtensionsViewer);
    // The inventory is a list when OpenCode has extensions and the empty state when it has none.
    await waitFor(() => expect(screen.queryByRole('navigation', { name: 'Extensions' }) ?? screen.queryByText('No extensions yet')).not.toBeNull());

    expect(screen.queryByRole('button', { name: 'Install' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'More actions' })).toBeNull();
  });

  it('folds a long group at first and a heading toggles it', async () => {
    await useHarness('omp');
    const long = extensionsView.data!.themes.length > 20;

    const themes = screen.getByRole('button', { name: /^Themes/ });
    expect(themes).toHaveAttribute('aria-expanded', String(!long));

    await fireEvent.click(themes);
    expect(themes).toHaveAttribute('aria-expanded', String(long));
  });

  it('searches every kind at once, opening the groups that match, and shows the selected item in the detail pane', async () => {
    await useHarness('omp');
    const first = extensionsView.data!.plugins[0];

    const search = screen.getByRole('searchbox', { name: 'Search extensions and catalogs' });
    await fireEvent.input(search, { target: { value: 'zzz-no-such-extension' } });
    expect(await screen.findByRole('status')).toHaveTextContent('Nothing matches');

    await fireEvent.input(search, { target: { value: first.name } });
    await fireEvent.click(await screen.findByRole('button', { name: new RegExp(first.name) }));

    expect(screen.getByRole('heading', { level: 2, name: first.name })).toBeInTheDocument();
  });

  it('offers Reveal for an item that has a file inside the one more-actions menu, next to Install', async () => {
    await useHarness('omp');

    expect(screen.getByRole('button', { name: 'Install' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Reveal in file manager' })).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
    expect(await screen.findByRole('menuitem', { name: 'Reveal in file manager' })).toBeEnabled();
  });
});

describe('items', () => {
  const view = {
    harness: 'pi',
    plugins: [],
    marketplaces: [],
    skills: [],
    promptTemplates: [],
    agents: [
      { name: 'build', description: 'Full tool access.', mode: 'primary', model: null, builtIn: true, hidden: false, scope: 'builtin', path: null },
      { name: 'reviewer', description: 'Reviews a change.', mode: 'subagent', model: 'anthropic/claude-sonnet-4', builtIn: false, hidden: true, scope: 'project', path: '/work/app/.opencode/agents/reviewer.md' },
    ],
    themes: [],
    packages: [{ source: { kind: 'secret', value: { type: 'value' } }, kind: 'git', scope: 'user', resources: ['skills'] }],
    extensionModules: [],
    notices: [],
    mcpServers: [
      {
        name: 'remote',
        transport: 'http',
        command: null,
        args: [],
        url: { kind: 'secret', value: { type: 'envRef', name: 'REMOTE_URL' } },
        enabled: true,
        source: { path: '/home/demo/.pi/agent/mcp.json', layer: 'user', profile: null, format: 'json', exists: true },
        details: [{ key: 'headers', value: { kind: 'map', value: [{ key: 'Authorization', value: { kind: 'secret', value: { type: 'value' } } }] } }],
      },
    ],
  } as unknown as ExtensionsView;

  it('renders hidden MCP endpoints and headers as chips only', () => {
    const [entry] = buildItems('mcp', view);

    const { container } = render(ExtensionDetail, { props: { entry } });

    expect(screen.getByText('$REMOTE_URL')).toBeInTheDocument();
    expect(screen.getByText('•••• set')).toBeInTheDocument();
    expect(entry.subtitle).toBeNull();
    expect(container.textContent).not.toMatch(/Bearer|sk-/);
  });

  it('titles a package with a hidden source without leaking anything', () => {
    const [entry] = buildItems('packages', view);

    expect(entry.title).toBe('Hidden source');
  });

  it('lists agents with their mode, scope and visibility', () => {
    const [build, reviewer] = buildItems('agents', view);

    expect(build.badges.map((badge) => badge.label)).toEqual(['Primary', 'Built-in']);
    expect(reviewer.badges.map((badge) => badge.label)).toEqual(['Subagent', 'Project', 'Hidden']);
    expect(filterItems(buildItems('agents', view), 'sonnet')).toHaveLength(1);
  });

  it('shows an agent\'s model, visibility and file in its detail', () => {
    const entry = buildItems('agents', view)[1];

    render(ExtensionDetail, { props: { entry } });

    expect(screen.getByRole('heading', { level: 2, name: 'reviewer' })).toBeInTheDocument();
    expect(screen.getByTitle('anthropic/claude-sonnet-4')).toBeInTheDocument();
    expect(screen.getByText('Hidden', { selector: 'dd *' })).toBeInTheDocument();
    expect(screen.getByText('Reviews a change.')).toBeInTheDocument();
  });

  it('keeps the shared kinds in one place for OMP and PI: what the harness installs first, OMP catalogs last', () => {
    const omp = visibleTabs({ plugins: true, marketplaces: true, skills: true, mcp: true, themes: true } as never).map((tab) => tab.id);
    const pi = visibleTabs({ packages: true, skills: true, mcp: true, themes: true } as never).map((tab) => tab.id);

    expect(omp).toEqual(['plugins', 'skills', 'mcp', 'themes', 'marketplaces']);
    expect(pi).toEqual(['packages', 'skills', 'mcp', 'themes']);
  });

  it('offers the agents tab only to a harness with the capability', () => {
    const agents = visibleTabs({ agents: true } as never);

    expect(agents.map((tab) => tab.id)).toEqual(['agents']);
    expect(visibleTabs({ agents: false } as never)).toEqual([]);
  });

  it('offers only the tabs a harness has capabilities for', () => {
    const none = visibleTabs({ plugins: false, marketplaces: false, packages: true, mcp: false, skills: false, promptTemplates: false, extensionModules: false, themes: false } as never);

    expect(none.map((tab) => tab.id)).toEqual(['packages']);
  });

  it('filters list items by any visible text', () => {
    const entries = buildItems('mcp', view);

    expect(filterItems(entries, 'REMOTE')).toHaveLength(1);
    expect(filterItems(entries, 'nothing')).toHaveLength(0);
  });
});

describe('in Polish', () => {
  it('translates the tabs and counts a marketplace\'s plugins with the Polish plural forms', async () => {
    await i18n.load('pl');
    const marketplace = (entries: number) => ({ name: 'demo', source: 'a/b', sourceType: 'github', error: null, entries: Array.from({ length: entries }, (_, index) => ({ name: `p${index}` })) });
    const view = { plugins: [], marketplaces: [marketplace(1)] } as unknown as ExtensionsView;

    expect(buildItems('marketplaces', view)[0].badges.map((badge) => badge.label)).toContain('1 wtyczka');
    for (const [count, label] of [[2, '2 wtyczki'], [5, '5 wtyczek'], [22, '22 wtyczki']] as const) {
      view.marketplaces = [marketplace(count)] as never;

      expect(buildItems('marketplaces', view)[0].badges.map((badge) => badge.label)).toContain(label);
    }

    prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp' });
    render(ExtensionsViewer);
    await screen.findByRole('navigation', { name: 'Rozszerzenia' });

    expect(headings()).toEqual(expect.arrayContaining(['Wtyczki', 'Skille']));
    expect(screen.getByRole('searchbox', { name: 'Szukaj rozszerzeń i katalogów' })).toBeInTheDocument();
  });
});
