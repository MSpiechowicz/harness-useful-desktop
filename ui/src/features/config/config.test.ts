import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import type { ConfigSection } from '$lib/ipc/bindings/ConfigSection';
import type { Notice } from '$lib/ipc/bindings/Notice';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { viewState } from '$lib/stores/view-state.svelte';
import { configView } from '$lib/stores/views.svelte';

import { samplePrefs } from '../../../tests/helpers';
import { configMode } from './config-mode.svelte';
import ConfigViewer from './ConfigViewer.svelte';
import { baseName, groupSections } from './sections';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp' });
  await harness.load();
});

const source = (layer: 'user' | 'project', exists: boolean, format: 'json' | 'yaml' = 'json') => ({ path: `/cfg/${layer}.json`, layer, profile: null, format, exists });
const text = (key: string) => ({ key, value: { kind: 'text' as const, value: 'dark' } });

/** Replaces the loaded configuration once the viewer is up, as a re-read from disk would. */
function load(sections: ConfigSection[], notices: Notice[] = []): void {
  configView.data = { harness: 'omp', notices, sections };
}

const details = () => screen.findByRole('region', { name: 'Details' });

describe('ConfigViewer', () => {
  afterEach(() => {
    configMode.set('structured');
    viewState.configQuery = '';
    viewState.configSection = null;
  });

  it('lists the configuration files grouped by layer, one line each, and shows the first one', async () => {
    render(ConfigViewer);

    const files = await screen.findByRole('navigation', { name: 'Configuration files' });
    const user = within(files).getByRole('region', { name: 'User layer' });

    expect(within(files).queryByRole('region', { name: 'Project layer' })).toBeNull();
    expect(within(user).getByRole('button', { name: /User/ })).toHaveAttribute('aria-expanded', 'true');

    const rows = within(user).getAllByRole('button').slice(1);
    expect(rows[0]).toHaveAttribute('aria-current', 'true');
    expect(rows[0]).toHaveAttribute('title', expect.stringMatching(/\.(ya?ml|json)$/));
    expect(within(rows[0]).queryByText(/\.(ya?ml|json)$/)).toBeNull();
    expect(await details()).toBeInTheDocument();
  });

  it('folds a layer under its heading and opens it again', async () => {
    render(ConfigViewer);
    await details();
    load([
      { id: 'user.a', title: 'Alpha', description: null, source: source('user', true), entries: [text('theme')], error: null },
      { id: 'user.b', title: 'Beta', description: null, source: source('user', true), entries: [text('theme')], error: null },
      { id: 'user.c', title: 'Gamma', description: null, source: source('user', true), entries: [text('theme')], error: null },
    ]);
    const files = await screen.findByRole('navigation', { name: 'Configuration files' });
    const user = within(files).getByRole('region', { name: 'User layer' });
    const heading = within(user).getByRole('button', { name: /User/ });

    await fireEvent.click(heading);

    // A folded layer hides every row, the selected one too. The detail stays on that file.
    expect(heading).toHaveAttribute('aria-expanded', 'false');
    expect(within(user).getAllByRole('button')).toHaveLength(1);
    expect(screen.getByRole('heading', { name: 'Alpha' })).toBeInTheDocument();

    await fireEvent.click(heading);

    expect(heading).toHaveAttribute('aria-expanded', 'true');
    expect(within(user).getAllByRole('button')).toHaveLength(4);
  });

  it('reloads when the project changes', async () => {
    render(ConfigViewer);
    await details();
    const before = configView.data?.sections.length;

    await project.open(fixtures.recentProjects.find((recent) => recent.exists)!.path);

    await waitFor(() => expect(configView.data?.sections.length).toBeGreaterThan(before ?? 0));
    expect(await screen.findByRole('region', { name: 'Project layer' })).toBeInTheDocument();
  });

  it('marks missing and unreadable files and explains them', async () => {
    render(ConfigViewer);
    await details();

    load([
      { id: 'user.ok', title: 'Settings', description: null, source: source('user', true), entries: [text('theme')], error: null },
      { id: 'project.missing', title: 'Project settings', description: null, source: source('project', false), entries: [], error: null },
      { id: 'project.broken', title: 'Broken settings', description: null, source: source('project', true), entries: [], error: 'Invalid JSON at line 4' },
    ]);

    const files = await screen.findByRole('navigation', { name: 'Configuration files' });
    expect(within(files).getAllByText('Not found')).toHaveLength(1);
    expect(within(files).getAllByText('Error')).toHaveLength(1);

    await fireEvent.click(within(files).getByText('Project settings').closest('button')!);
    expect(await screen.findByText('No file here yet')).toBeInTheDocument();
    expect(screen.queryByRole('searchbox')).toBeNull();
    expect(screen.queryByRole('radiogroup', { name: 'View' })).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
    expect(screen.getByRole('menuitem', { name: 'Reveal in file manager' })).toBeDisabled();
    expect(screen.queryByRole('menuitem', { name: 'Copy path' })).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));

    await fireEvent.click(within(files).getByText('Broken settings').closest('button')!);
    expect(await screen.findByRole('alert')).toHaveTextContent('This file could not be read');
    expect(screen.getByRole('alert')).toHaveTextContent('Invalid JSON at line 4');
    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
    expect(screen.getByRole('menuitem', { name: 'Reveal in file manager' })).toBeEnabled();
    expect(screen.queryByRole('searchbox')).toBeNull();
  });

  it('filters the file list by the search, by title, file name or content', async () => {
    render(ConfigViewer);
    await details();
    load([
      { id: 'user.settings', title: 'Settings', description: null, source: source('user', true), entries: [text('theme')], error: null },
      { id: 'user.mcp', title: 'MCP servers', description: null, source: source('user', true), entries: [text('server')], error: null },
    ]);

    const files = await screen.findByRole('navigation', { name: 'Configuration files' });
    expect(within(files).getAllByRole('button', { name: /Settings|MCP servers/ })).toHaveLength(2);

    await fireEvent.input(await screen.findByRole('searchbox', { name: 'Search keys and values' }), { target: { value: 'sett' } });
    expect(within(files).getByText('Settings')).toBeInTheDocument();
    expect(within(files).queryByText('MCP servers')).toBeNull();

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search keys and values' }), { target: { value: 'server' } });
    expect(within(files).getByText('MCP servers')).toBeInTheDocument();
    expect(within(files).queryByText('Settings')).toBeNull();

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search keys and values' }), { target: { value: 'zzz' } });
    expect(within(files).getByText('No file matches your search.')).toBeInTheDocument();
  });

  it('searches the selected file from the top bar and counts the matches', async () => {
    render(ConfigViewer);
    await details();
    load([{ id: 'user.ok', title: 'Settings', description: null, source: source('user', true), entries: [text('theme'), text('themeDark'), text('model')], error: null }]);

    const box = await screen.findByRole('searchbox', { name: 'Search keys and values' });
    await fireEvent.input(box, { target: { value: 'theme' } });

    expect(await screen.findByText('2 matches')).toBeInTheDocument();
    expect(viewState.configQuery).toBe('theme');

    await fireEvent.keyDown(box, { key: 'Escape' });

    expect(viewState.configQuery).toBe('');
  });

  it('switches between the structured view and the source and remembers the choice', async () => {
    render(ConfigViewer);
    await details();
    load([{ id: 'user.ok', title: 'Settings', description: null, source: source('user', true), entries: [text('theme')], error: null }]);

    const modes = await screen.findByRole('radiogroup', { name: 'View' });
    expect(within(modes).getByRole('radio', { name: 'Structured' })).toBeChecked();

    await fireEvent.click(within(modes).getByRole('radio', { name: 'Source' }));

    expect(within(modes).getByRole('radio', { name: 'Source' })).toBeChecked();
    expect(configMode.value).toBe('source');
    expect(window.localStorage.getItem('harness.configMode')).toBe('source');
  });

  it('shows the profiles as a card, not as a notice', async () => {
    render(ConfigViewer);
    await details();
    load(configView.data!.sections, [{ level: 'info', message: 'Profiles: home-lab, work. Start OMP with --profile <name> to use one.' }]);

    const card = (await screen.findByRole('heading', { name: /Profiles/ })).closest('section')!;

    expect(within(card).getByText('home-lab')).toBeInTheDocument();
    expect(within(card).getByText('work')).toBeInTheDocument();
    expect(within(card).getByText('--profile <name>')).toBeInTheDocument();
    expect(screen.queryByText(/^Profiles: /)).toBeNull();
  });

  it('folds a long list of profiles into a count and unfolds it on request', async () => {
    render(ConfigViewer);
    await details();
    const names = Array.from({ length: 9 }, (_, index) => `profile-${index}`);
    load(configView.data!.sections, [{ level: 'info', message: `Profiles: ${names.join(', ')}. Start OMP with --profile <name> to use one.` }]);

    const more = await screen.findByRole('button', { name: 'Show all 9' });
    expect(screen.queryByText('profile-8')).toBeNull();

    await fireEvent.click(more);

    expect(screen.getByText('profile-8')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Show fewer' })).toHaveAttribute('aria-expanded', 'true');
  });

  it('keeps other notices in the banner', async () => {
    render(ConfigViewer);
    await details();
    load(configView.data!.sections, [{ level: 'warning', message: 'The project layer is not trusted.' }]);

    expect(await screen.findByText('The project layer is not trusted.')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: /Profiles/ })).toBeNull();
  });

  it('summarises the file in the overview and file cards', async () => {
    render(ConfigViewer);
    await details();
    const secret = { key: 'token', value: { kind: 'secret' as const, value: { type: 'value' as const } } };
    load([
      {
        id: 'user.ok',
        title: 'Settings',
        description: 'Global settings',
        source: source('user', true, 'yaml'),
        entries: [text('theme'), { key: 'auth', value: { kind: 'map', value: [secret, text('user')] } }],
        error: null,
      },
    ]);

    const aside = await details();
    const overview = within(aside).getByRole('heading', { name: 'Overview' }).closest('section')!;

    for (const [label, value] of [['top-level entries', '2'], ['values', '3'], ['levels deep', '2'], ['secrets masked', '1']]) {
      expect(within(overview).getByText(label).nextElementSibling).toHaveTextContent(value);
    }

    expect(within(aside).getByText('Layer').nextElementSibling).toHaveTextContent('User');
    expect(within(aside).getByText('Format').nextElementSibling).toHaveTextContent('YAML');
    expect(within(aside).getByTitle('/cfg/user.json')).toBeInTheDocument();
    expect(screen.getByText(/Global settings/)).toBeInTheDocument();
  });

  it('copies the file path from the menu', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal('navigator', { ...navigator, clipboard: { writeText } });
    render(ConfigViewer);
    await details();
    load([{ id: 'user.ok', title: 'Settings', description: null, source: source('user', true), entries: [text('theme')], error: null }]);

    await fireEvent.click(await screen.findByRole('button', { name: 'More actions' }));
    expect(await screen.findByRole('menuitem', { name: /all$/ })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('menuitem', { name: 'Copy path' }));

    expect(writeText).toHaveBeenCalledWith('/cfg/user.json');
    vi.unstubAllGlobals();
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

describe('ConfigViewer top bar', () => {
  const sections: ConfigSection[] = [
    {
      id: 'user.a',
      title: 'Settings',
      description: null,
      source: source('user', true),
      entries: [text('theme'), text('themeDark'), { key: 'auth', value: { kind: 'map', value: [text('user')] } }],
      error: null,
    },
    { id: 'user.b', title: 'Other file', description: null, source: source('user', true), entries: [text('model')], error: null },
  ];

  beforeEach(() => {
    prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp' });
  });

  afterEach(() => {
    configMode.set('structured');
    viewState.configQuery = '';
    viewState.configSection = null;
  });

  async function open(mode: 'structured' | 'source') {
    configMode.set(mode);
    render(ConfigViewer);
    await details();
    load(sections);

    return screen.findByRole('searchbox', { name: 'Search keys and values' });
  }

  /** The source line the search stepped to, by its index in the file, or null before any step. */
  const stepped = () => document.querySelector('[data-index][class*="inset_2px"]')?.getAttribute('data-index') ?? null;

  it('steps through the matches of the source with Enter and Shift+Enter', async () => {
    const box = await open('source');
    await fireEvent.input(box, { target: { value: 'theme' } });
    await screen.findByText('2 matches');
    expect(stepped()).toBeNull();

    expect(await fireEvent.keyDown(box, { key: 'Enter' })).toBe(false);
    await waitFor(() => expect(stepped()).toBe('1'));

    await fireEvent.keyDown(box, { key: 'Enter' });
    await waitFor(() => expect(stepped()).toBe('2'));

    await fireEvent.keyDown(box, { key: 'Enter', shiftKey: true });
    await waitFor(() => expect(stepped()).toBe('1'));

    await fireEvent.keyDown(box, { key: 'Enter', shiftKey: true });
    await waitFor(() => expect(stepped()).toBe('2'));
  });

  it('leaves Enter alone in the structured view, which filters instead of stepping', async () => {
    const box = await open('structured');
    await fireEvent.input(box, { target: { value: 'theme' } });
    await screen.findByText('2 matches');

    expect(await fireEvent.keyDown(box, { key: 'Enter' })).toBe(true);
    expect(await fireEvent.keyDown(box, { key: 'Enter', shiftKey: true })).toBe(true);
    expect(stepped()).toBeNull();

    // Nothing was queued for later: the source starts at its first step when it is switched to.
    await fireEvent.click(within(screen.getByRole('radiogroup', { name: 'View' })).getByRole('radio', { name: 'Source' }));
    await screen.findByRole('list', { name: /Settings/ });
    expect(stepped()).toBeNull();
  });

  for (const mode of ['source', 'structured'] as const) {
    it(`flips the menu between Collapse all and Expand all in the ${mode} view`, async () => {
      await open(mode);
      const content = () => (mode === 'source' ? screen.queryAllByRole('listitem') : screen.queryAllByRole('region', { name: 'Appearance' }));
      const before = content().length;
      expect(before).toBeGreaterThan(0);

      await fireEvent.click(await screen.findByRole('button', { name: 'More actions' }));
      await fireEvent.click(await screen.findByRole('menuitem', { name: 'Collapse all' }));

      await waitFor(() => expect(content().length).toBeLessThan(before));

      await fireEvent.click(await screen.findByRole('button', { name: 'More actions' }));
      expect(screen.queryByRole('menuitem', { name: 'Collapse all' })).toBeNull();
      await fireEvent.click(await screen.findByRole('menuitem', { name: 'Expand all' }));

      await waitFor(() => expect(content()).toHaveLength(before));
      await fireEvent.click(await screen.findByRole('button', { name: 'More actions' }));
      expect(await screen.findByRole('menuitem', { name: 'Collapse all' })).toBeInTheDocument();
    });
  }

  it('clears the search when another file is picked from the list', async () => {
    const box = await open('structured');
    await fireEvent.input(box, { target: { value: 'e' } });
    expect(viewState.configQuery).toBe('e');

    const files = await screen.findByRole('navigation', { name: 'Configuration files' });
    await fireEvent.click(within(files).getByText('Other file').closest('button')!);

    expect(viewState.configSection).toBe('user.b');
    expect(viewState.configQuery).toBe('');
  });

  it('drops a palette search for a file that is gone when the first file takes over', async () => {
    viewState.configSection = 'user.gone';
    viewState.configQuery = 'theme';
    configMode.set('structured');
    render(ConfigViewer);
    await details();
    load(sections);

    await waitFor(() => expect(viewState.configSection).toBe('user.a'));
    expect(viewState.configQuery).toBe('');
  });

  it('keeps a search set with no file chosen yet', async () => {
    viewState.configQuery = 'theme';
    render(ConfigViewer);
    await details();
    load(sections);

    await waitFor(() => expect(viewState.configSection).toBe('user.a'));
    expect(viewState.configQuery).toBe('theme');
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

  it('labels the top bar and counts matches with the Polish plural forms', async () => {
    await i18n.load('pl');
    prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp' });
    render(ConfigViewer);
    await screen.findByRole('region', { name: 'Szczegóły' });

    for (const [count, text] of [
      [1, '1 dopasowanie'],
      [2, '2 dopasowania'],
      [5, '5 dopasowań'],
    ] as const) {
      configView.data = { harness: 'omp', notices: [], sections: [section(count)] };
      viewState.configQuery = 'key';

      expect(await screen.findByText(text)).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Więcej akcji' })).toBeInTheDocument();
    }

    await i18n.load('en');
  });
});
