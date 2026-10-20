import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { withoutChat } from '../../../tests/helpers';
import { intro } from '../intro/intro-state.svelte';
import Onboarding from './Onboarding.svelte';
import { onboarding } from './onboarding-state.svelte';

// The mock machine: Oh My Pi and OpenCode installed, Pi missing. Oh My Pi has 5 sessions on the machine, 3 plugins and 5 user-level MCP servers.
// OpenCode cannot total its sessions, so it counts the 2 in the demo project.
beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');

  await prefs.load();
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
  intro.skip();
});

beforeEach(async () => {
  localStorage.clear();
  await prefs.update({ defaultHarness: null, onboardingCompleted: false });
  await harness.load();
});

afterEach(() => {
  vi.restoreAllMocks();
});

const detail = (name: string) => screen.findByRole('article', { name });

async function openPicker(): Promise<void> {
  onboarding.restart('harnesses');
  render(Onboarding);
  await screen.findByRole('heading', { name: 'Which harness do you use every day?' });
}

/** Answers `command` with `answer` (a rejection when it is an Error) and everything else as the mock machine does. */
function answering(command: string, answer: unknown): void {
  const invoke = ipc.invoke.bind(ipc);

  vi.spyOn(ipc, 'invoke').mockImplementation(((name: never, args: never) => {
    if ((name as string) !== command) {
      return invoke(name, args);
    }

    return answer instanceof Error ? Promise.reject(answer) : Promise.resolve(answer);
  }) as typeof ipc.invoke);
}

const factsOf = async (name: string) => within(await detail(name)).findByRole('list', { name: `What ${name} has on this machine` });

/** The tile with this label: its number (empty while it is being read) and the tile itself. */
function tileOf(facts: HTMLElement, label: string): { value: string; item: HTMLElement } {
  const item = within(facts).getByText(label).closest('li') as HTMLElement;

  return { value: item.firstElementChild?.textContent?.trim() ?? '', item };
}

const labelsOf = (facts: HTMLElement): (string | null)[] => within(facts).getAllByRole('listitem').map((item) => item.children[1]?.textContent ?? null);

/** An answer that is given when `release` is called. */
function pending(): { promise: Promise<never>; release: (answer: unknown) => void } {
  let release: (answer: unknown) => void = () => undefined;
  const promise = new Promise<never>((resolve) => {
    release = resolve as (answer: unknown) => void;
  });

  return { promise, release };
}

function readsOf(command: string): number {
  return vi.mocked(ipc.invoke).mock.calls.filter(([name]) => name === command).length;
}

describe('What an installed harness has on this machine', () => {
  it('shows the sessions, when it was last used, the plugins and the MCP servers as one number each', async () => {
    await openPicker();

    const facts = await factsOf('Oh My Pi');

    await waitFor(() => expect(tileOf(facts, 'Plugins').value).toBe('3'));
    expect(labelsOf(facts)).toEqual(['Sessions', 'Last used', 'Plugins', 'MCP servers']);
    expect(tileOf(facts, 'Sessions').value).toBe('5');
    expect(tileOf(facts, 'Last used').value).toMatch(/ago$/);
    expect(tileOf(facts, 'MCP servers').value).toBe('5');
  });

  it('counts packages where the harness has packages and no plugins', async () => {
    harness.summaries = harness.summaries.map((summary) =>
      summary.info.id === 'omp' ? { ...summary, capabilities: { ...summary.capabilities, plugins: false, packages: true } } : summary,
    );
    await openPicker();

    const facts = await factsOf('Oh My Pi');

    await waitFor(() => expect(labelsOf(facts)).toEqual(['Sessions', 'Last used', 'Packages', 'MCP servers']));
    expect(within(facts).queryByText('Plugins')).toBeNull();
  });

  it('shows the last use in a few words', async () => {
    answering('harness_session_totals', { count: 4, lastUsed: new Date(Date.now() - 37 * 60_000).toISOString(), capped: false });
    await openPicker();

    const facts = await factsOf('Oh My Pi');

    await waitFor(() => expect(tileOf(facts, 'Last used').value).toBe('37 min. ago'));
  });

  it('leaves out a fact it cannot read and lets the others share the row', async () => {
    const invoke = ipc.invoke.bind(ipc);
    vi.spyOn(ipc, 'invoke').mockImplementation(((name: never, args: never) =>
      (name as string) === 'extensions_view' ? Promise.reject(new Error('unreadable')) : invoke(name, args)) as typeof ipc.invoke);
    await openPicker();

    const facts = await within(await detail('Oh My Pi')).findByRole('list', { name: 'What Oh My Pi has on this machine' });

    await waitFor(() => expect(labelsOf(facts)).toEqual(['Sessions', 'Last used']));
    expect(tileOf(facts, 'Sessions').value).toBe('5');
    expect(facts).not.toHaveTextContent('—');
  });

  it('counts the sessions on the machine without a project to count in', async () => {
    const recents = project.recents;
    const active = project.active;
    project.recents = [];
    project.active = null;
    harness.summaries = harness.summaries.map((summary) =>
      summary.info.id === 'omp' ? { ...summary, capabilities: { ...summary.capabilities, plugins: false, packages: false, mcp: false } } : summary,
    );

    try {
      await openPicker();
      const facts = await factsOf('Oh My Pi');

      await waitFor(() => expect(tileOf(facts, 'Sessions').value).toBe('5'));
      expect(labelsOf(facts)).toEqual(['Sessions', 'Last used']);
      expect(tileOf(facts, 'Last used').value).toMatch(/ago$/);
    } finally {
      project.recents = recents;
      project.active = active;
    }
  });

  it('shows a capped count with a plus', async () => {
    answering('harness_session_totals', { count: 500, lastUsed: '2026-10-09T12:00:30Z', capped: true });
    await openPicker();

    const facts = await factsOf('Oh My Pi');
    await waitFor(() => expect(tileOf(facts, 'Sessions').value).toBe('500+'));
  });

  it('leaves out the last use when the harness has no session yet', async () => {
    answering('harness_session_totals', { count: 0, lastUsed: null, capped: false });
    await openPicker();

    const facts = await factsOf('Oh My Pi');

    await waitFor(() => expect(labelsOf(facts)).toEqual(['Sessions', 'Plugins', 'MCP servers']));
    expect(tileOf(facts, 'Sessions').value).toBe('0');
  });

  it('counts the sessions of the project where the harness cannot total them, and names the project outside the label', async () => {
    await openPicker();
    await fireEvent.click(await screen.findByRole('option', { name: /^OpenCode/ }));

    const facts = await factsOf('OpenCode');

    await waitFor(() => expect(tileOf(facts, 'Sessions').value).toBe('2'));
    const { item } = tileOf(facts, 'Sessions');

    expect(item).toHaveAttribute('title', 'Sessions in demo');
    expect(item).toHaveAccessibleDescription('Sessions in demo');
    expect(within(facts).getByText('Sessions')).toBeVisible();
    expect(within(facts).queryByText(/in demo/)).not.toBeVisible();
  });

  it('shows the labels while the numbers are read, then the numbers where the placeholders were', async () => {
    const totals = pending();
    const extensions = pending();
    const invoke = ipc.invoke.bind(ipc);
    vi.spyOn(ipc, 'invoke').mockImplementation(((name: never, args: never) => {
      if ((name as string) === 'harness_session_totals') {
        return totals.promise;
      }

      return (name as string) === 'extensions_view' ? extensions.promise : invoke(name, args);
    }) as typeof ipc.invoke);
    await openPicker();

    const facts = await factsOf('Oh My Pi');
    const labels = ['Sessions', 'Last used', 'Plugins', 'MCP servers'];

    expect(labelsOf(facts)).toEqual(labels);
    for (const label of labels) {
      expect(tileOf(facts, label).value).toBe('');
    }

    // One answer is not enough: the row is laid out once, when every fact is known or given up on.
    totals.release({ count: 7, lastUsed: null, capped: false });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(labelsOf(facts)).toEqual(labels);
    expect(tileOf(facts, 'Sessions').value).toBe('');

    extensions.release({ plugins: [{}, {}], mcpServers: [{}], packages: [], skills: [] });
    await waitFor(() => expect(tileOf(facts, 'Sessions').value).toBe('7'));
    expect(labelsOf(facts)).toEqual(['Sessions', 'Plugins', 'MCP servers']);
    expect(tileOf(facts, 'Plugins').value).toBe('2');
    expect(tileOf(facts, 'MCP servers').value).toBe('1');
  });

  it('reads the installed harnesses on the intro step, so the picker has the numbers without asking again', async () => {
    vi.spyOn(ipc, 'invoke');
    onboarding.restart('meet');
    render(Onboarding);
    await screen.findByRole('heading', { name: 'Meet your harnesses' });

    await waitFor(() => expect(readsOf('extensions_view')).toBe(2));
    await waitFor(() => expect(readsOf('harness_session_totals')).toBe(2));
    await waitFor(() => expect(readsOf('sessions_list')).toBe(1));

    await fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    await screen.findByRole('heading', { name: 'Which harness do you use every day?' });
    const facts = await factsOf('Oh My Pi');

    expect(tileOf(facts, 'Sessions').value).toBe('5');
    expect(tileOf(facts, 'Plugins').value).toBe('3');
    expect(readsOf('extensions_view')).toBe(2);
    expect(readsOf('harness_session_totals')).toBe(2);
    expect(readsOf('sessions_list')).toBe(1);
  });

  it('shows no sessions and no last use where the harness cannot total them and there is no project, and no empty heading when nothing is known', async () => {
    const recents = project.recents;
    const active = project.active;
    project.recents = [];
    project.active = null;
    harness.summaries = harness.summaries.map((summary) =>
      summary.info.id === 'opencode' ? { ...summary, capabilities: { ...summary.capabilities, plugins: false, packages: false, mcp: false } } : summary,
    );

    try {
      await openPicker();
      await fireEvent.click(await screen.findByRole('option', { name: /^OpenCode/ }));
      const opencode = await detail('OpenCode');

      await waitFor(() => expect(within(opencode).queryByRole('list', { name: /has on this machine/ })).toBeNull());
      expect(within(opencode).queryByText('On this machine')).toBeNull();
      expect(within(opencode).getByRole('button', { name: 'Open config folder' })).toBeEnabled();
    } finally {
      project.recents = recents;
      project.active = active;
    }
  });

  it('leaves out the sessions when they cannot be read, without counting a project instead', async () => {
    answering('harness_session_totals', new Error('unreadable'));
    vi.spyOn(ipc, 'invoke');
    await openPicker();

    const facts = await factsOf('Oh My Pi');

    await waitFor(() => expect(labelsOf(facts)).toEqual(['Plugins', 'MCP servers']));
    expect(readsOf('sessions_list')).toBe(0);
  });

  it('shows no facts for a harness the app cannot chat with, nor for a missing one', async () => {
    withoutChat(harness, 'opencode');
    await openPicker();

    await fireEvent.click(await screen.findByRole('option', { name: /^OpenCode/ }));
    const soon = await detail('OpenCode');
    expect(within(soon).queryByText('On this machine')).toBeNull();
    expect(within(soon).queryByRole('button', { name: 'Open config folder' })).toBeNull();

    await fireEvent.click(await screen.findByRole('option', { name: /^Pi/ }));
    const missing = await detail('Pi');
    expect(within(missing).queryByText('On this machine')).toBeNull();
    expect(within(missing).getByRole('button', { name: 'Install docs' })).toBeEnabled();
  });

  it('opens the config folder and the homepage', async () => {
    await openPicker();
    const omp = await detail('Oh My Pi');
    const info = harness.get('omp');
    const invoke = vi.spyOn(ipc, 'invoke');

    await fireEvent.click(within(omp).getByRole('button', { name: 'Open config folder' }));
    await fireEvent.click(within(omp).getByRole('button', { name: 'Homepage' }));

    expect(invoke).toHaveBeenCalledWith('reveal_path', { path: info?.detection.configDir });
    expect(invoke).toHaveBeenCalledWith('open_external', { url: info?.info.homepage });
  });

  it('disables the config folder button while the folder does not exist', async () => {
    harness.summaries = harness.summaries.map((summary) =>
      summary.info.id === 'omp' ? { ...summary, detection: { ...summary.detection, configDirExists: false } } : summary,
    );
    await openPicker();

    expect(within(await detail('Oh My Pi')).getByRole('button', { name: 'Open config folder' })).toBeDisabled();
  });

  it('reads a harness once, however often its row is looked at again', async () => {
    vi.spyOn(ipc, 'invoke');
    await openPicker();

    await waitFor(() => expect(readsOf('extensions_view')).toBe(1));

    await fireEvent.click(await screen.findByRole('option', { name: /^OpenCode/ }));
    await waitFor(() => expect(readsOf('extensions_view')).toBe(2));

    await fireEvent.click(await screen.findByRole('option', { name: /^Oh My Pi/ }));
    await fireEvent.click(await screen.findByRole('option', { name: /^OpenCode/ }));
    await fireEvent.click(await screen.findByRole('option', { name: /^Oh My Pi/ }));

    const facts = await factsOf('Oh My Pi');
    await waitFor(() => expect(tileOf(facts, 'Plugins').value).toBe('3'));
    expect(readsOf('extensions_view')).toBe(2);
    await waitFor(() => expect(readsOf('sessions_list')).toBe(1));
    expect(readsOf('harness_session_totals')).toBe(2);
  });
});
