import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { configView } from '$lib/stores/views.svelte';

import { samplePrefs } from '../../../tests/helpers';
import ConfigViewer from './ConfigViewer.svelte';
import SectionDetail from './SectionDetail.svelte';
import { baseName, groupSections } from './sections';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp' });
  await harness.load();
});

describe('ConfigViewer', () => {
  it('lists the configuration files grouped by layer and shows the first one', async () => {
    render(ConfigViewer);

    const files = await screen.findByRole('navigation', { name: 'Configuration files' });

    expect(within(files).getByRole('region', { name: 'User layer' })).toBeInTheDocument();
    expect(within(files).queryByRole('region', { name: 'Project layer' })).toBeNull();
    expect(within(files).getAllByRole('button')[0]).toHaveAttribute('aria-current', 'true');
    expect(screen.getByRole('tree')).toBeInTheDocument();
  });

  it('reloads when the project changes', async () => {
    render(ConfigViewer);
    await screen.findByRole('tree');
    const before = configView.data?.sections.length;

    await project.open(fixtures.recentProjects.find((recent) => recent.exists)!.path);

    await waitFor(() => expect(configView.data?.sections.length).toBeGreaterThan(before ?? 0));
    expect(await screen.findByRole('region', { name: 'Project layer' })).toBeInTheDocument();
  });

  it('marks missing and unreadable files and explains them', async () => {
    render(ConfigViewer);
    await screen.findByRole('tree');
    const source = (layer: 'user' | 'project', exists: boolean) => ({ path: `/cfg/${layer}.json`, layer, profile: null, format: 'json' as const, exists });
    const entries = [{ key: 'theme', value: { kind: 'text' as const, value: 'dark' } }];

    configView.data = {
      harness: 'omp',
      notices: [],
      sections: [
        { id: 'user.ok', title: 'Settings', description: null, source: source('user', true), entries, error: null },
        { id: 'project.missing', title: 'Project settings', description: null, source: source('project', false), entries: [], error: null },
        { id: 'project.broken', title: 'Broken settings', description: null, source: source('project', true), entries: [], error: 'Invalid JSON at line 4' },
      ],
    };

    const files = await screen.findByRole('navigation', { name: 'Configuration files' });
    expect(within(files).getAllByText('Not found')).toHaveLength(1);

    await fireEvent.click(within(files).getByText('Project settings').closest('button')!);
    expect(await screen.findByText('No file here yet')).toBeInTheDocument();
    expect(screen.queryByRole('tree')).toBeNull();

    await fireEvent.click(within(files).getByText('Broken settings').closest('button')!);
    expect(await screen.findByRole('alert')).toHaveTextContent('This file could not be read');
    expect(screen.getByRole('alert')).toHaveTextContent('Invalid JSON at line 4');
    expect(screen.queryByRole('tree')).toBeNull();
  });

  it('filters the selected file by key', async () => {
    render(ConfigViewer);
    await screen.findByRole('tree');
    const section = configView.data!.sections.find((candidate) => candidate.entries.length > 0)!;
    await fireEvent.click(within(screen.getByRole('navigation', { name: 'Configuration files' })).getByText(section.title).closest('button')!);
    const key = section.entries[0].key;
    const before = screen.getAllByRole('treeitem').length;

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Filter keys and values' }), { target: { value: key } });

    await waitFor(() => expect(screen.getAllByRole('treeitem').length).toBeLessThanOrEqual(before));
    expect(screen.getAllByRole('treeitem')[0]).toHaveTextContent(key);
  });

  it('points a missing harness to setup', async () => {
    prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'pi' });
    harness.summaries = harness.summaries.map((summary) =>
      summary.info.id === 'pi' ? { ...summary, detection: { ...summary.detection, installed: false, configDirExists: false } } : summary,
    );

    render(ConfigViewer);

    expect(await screen.findByRole('heading', { name: 'Pi is not installed' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Set up PI' })).toBeInTheDocument();
  });
});

describe('sections', () => {
  it('orders layers user, profile, project and drops empty ones', () => {
    const section = (layer: 'user' | 'profile' | 'project') => ({ source: { layer } }) as never;

    expect(groupSections([section('project'), section('user')]).map((group) => group.layer)).toEqual(['user', 'project']);
  });

  it('extracts the file name for either separator', () => {
    expect(baseName('/home/u/.omp/agent/config.yml')).toBe('config.yml');
    expect(baseName('C:\\Users\\u\\.pi\\settings.json')).toBe('settings.json');
  });
});

describe('in Polish', () => {
  const section = (count: number): ConfigSection => ({
    id: 'user.settings',
    title: 'Settings',
    description: null,
    source: { path: '/cfg/settings.json', layer: 'user', profile: null, format: 'json', exists: true },
    entries: Array.from({ length: count }, (_, index) => ({ key: `key${index}`, value: { kind: 'text' as const, value: 'x' } })),
    error: null,
  });

  it('labels the detail and counts its entries with the Polish plural forms', async () => {
    await i18n.load('pl');

    for (const [count, text] of [
      [1, '1 wpis najwyższego poziomu'],
      [2, '2 wpisy najwyższego poziomu'],
      [5, '5 wpisów najwyższego poziomu'],
    ] as const) {
      const { unmount } = render(SectionDetail, { props: { section: section(count) } });

      expect(screen.getByRole('button', { name: 'Pokaż w menedżerze plików' })).toBeInTheDocument();
      expect(screen.getByText(text)).toBeInTheDocument();
      unmount();
    }
  });
});
