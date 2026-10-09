import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { extensionsView } from '$lib/stores/views.svelte';

import { samplePrefs } from '../../../tests/helpers';
import ExtensionDetail from './ExtensionDetail.svelte';
import { buildItems, filterItems, visibleTabs } from './extension-items';
import ExtensionsViewer from './ExtensionsViewer.svelte';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  await harness.load();
});

const tabNames = () => screen.getAllByRole('tab').map((tab) => tab.textContent?.replace(/\s*\d+$/, '').trim());

async function useHarness(id: 'omp' | 'pi') {
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: id });
  render(ExtensionsViewer);
  await screen.findByRole('tab', { name: /Skills/ });
}

describe('ExtensionsViewer', () => {
  it('shows the tabs OMP supports, each with its count', async () => {
    await useHarness('omp');

    expect(tabNames()).toEqual(['Plugins', 'Marketplaces', 'Skills', 'MCP servers', 'Themes', 'Modules']);

    const plugins = extensionsView.data?.plugins.length ?? -1;
    expect(screen.getByRole('tab', { name: /Plugins/ })).toHaveTextContent(String(plugins));
    expect(within(screen.getByRole('list', { name: 'Plugins' })).getAllByRole('button')).toHaveLength(plugins);
  });

  it('shows the tabs PI supports instead', async () => {
    await useHarness('pi');

    expect(tabNames()).toEqual(['Skills', 'Prompts', 'MCP servers', 'Themes', 'Packages', 'Modules']);
  });

  it('searches within a tab and shows the selected item in the detail pane', async () => {
    await useHarness('omp');
    const first = extensionsView.data!.plugins[0];

    const search = screen.getByRole('searchbox', { name: 'Search plugins' });
    await fireEvent.input(search, { target: { value: 'zzz-no-such-plugin' } });
    expect(await screen.findByRole('status')).toHaveTextContent('No plugins match');

    await fireEvent.input(search, { target: { value: first.name } });
    await fireEvent.click(await screen.findByRole('button', { name: new RegExp(first.name) }));

    expect(screen.getByRole('heading', { level: 2, name: first.name })).toBeInTheDocument();
  });

  it('is read-only: no install, enable or remove actions', async () => {
    await useHarness('omp');

    expect(screen.queryByRole('button', { name: /^(install|uninstall|enable|disable|remove|update)\b/i })).toBeNull();
  });
});

describe('items', () => {
  const view = {
    harness: 'pi',
    plugins: [],
    marketplaces: [],
    skills: [],
    promptTemplates: [],
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
