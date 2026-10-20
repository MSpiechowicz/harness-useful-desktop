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

function readsOf(command: string): number {
  return vi.mocked(ipc.invoke).mock.calls.filter(([name]) => name === command).length;
}

describe('What an installed harness has on this machine', () => {
  it('shows the sessions, when it was last used, and the plugins and MCP servers', async () => {
    await openPicker();

    const omp = await detail('Oh My Pi');
    const facts = await within(omp).findByRole('list', { name: 'What Oh My Pi has on this machine' });

    await within(facts).findByText('Plugins · MCP servers');
    const [sessions, lastUsed, installed] = within(facts).getAllByRole('listitem');

    expect(sessions).toHaveTextContent(/^5 Sessions$/);
    expect(lastUsed).toHaveTextContent(/^.+ ago Last used$/);
    expect(installed).toHaveTextContent(/^3 · 5 Plugins · MCP servers$/);
  });

  it('counts packages where the harness has packages and no plugins', async () => {
    harness.summaries = harness.summaries.map((summary) =>
      summary.info.id === 'omp' ? { ...summary, capabilities: { ...summary.capabilities, plugins: false, packages: true } } : summary,
    );
    await openPicker();

    const facts = await within(await detail('Oh My Pi')).findByRole('list', { name: 'What Oh My Pi has on this machine' });

    await waitFor(() => expect(within(facts).getByText('Packages · MCP servers')).toBeInTheDocument());
    expect(within(facts).queryByText(/Plugins/)).toBeNull();
  });

  it('leaves out a fact it cannot read and lets the others share the row', async () => {
    const invoke = ipc.invoke.bind(ipc);
    vi.spyOn(ipc, 'invoke').mockImplementation(((name: never, args: never) =>
      (name as string) === 'extensions_view' ? Promise.reject(new Error('unreadable')) : invoke(name, args)) as typeof ipc.invoke);
    await openPicker();

    const facts = await within(await detail('Oh My Pi')).findByRole('list', { name: 'What Oh My Pi has on this machine' });

    await within(facts).findByText('Last used');
    await waitFor(() => expect(within(facts).getAllByRole('listitem')).toHaveLength(2));
    expect(facts).not.toHaveTextContent('MCP servers');
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
      const omp = await detail('Oh My Pi');
      await within(omp).findByText('Last used');
      const [sessions, lastUsed] = within(omp).getAllByRole('listitem');

      expect(sessions).toHaveTextContent(/^5 Sessions$/);
      expect(lastUsed).toHaveTextContent(/^.+ ago Last used$/);
    } finally {
      project.recents = recents;
      project.active = active;
    }
  });

  it('shows a capped count with a plus', async () => {
    answering('harness_session_totals', { count: 500, lastUsed: '2026-10-09T12:00:30Z', capped: true });
    await openPicker();

    const facts = await factsOf('Oh My Pi');
    await within(facts).findByText('Sessions');
    const [sessions] = within(facts).getAllByRole('listitem');

    expect(sessions).toHaveTextContent(/^500\+ Sessions$/);
  });

  it('leaves out the last use when the harness has no session yet', async () => {
    answering('harness_session_totals', { count: 0, lastUsed: null, capped: false });
    await openPicker();

    const facts = await factsOf('Oh My Pi');

    await within(facts).findByText('Plugins · MCP servers');
    expect(within(facts).getAllByRole('listitem')).toHaveLength(2);
    expect(within(facts).getAllByRole('listitem')[0]).toHaveTextContent(/^0 Sessions$/);
    expect(facts).not.toHaveTextContent('Last used');
  });

  it('counts the sessions of the project where the harness cannot total them', async () => {
    await openPicker();
    await fireEvent.click(await screen.findByRole('option', { name: /^OpenCode/ }));

    const facts = await factsOf('OpenCode');
    await within(facts).findByText('Sessions in demo');
    const [sessions] = within(facts).getAllByRole('listitem');

    expect(sessions).toHaveTextContent(/^2 Sessions in demo$/);
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

    await within(facts).findByText('Plugins · MCP servers');
    expect(within(facts).getAllByRole('listitem')).toHaveLength(1);
    expect(facts).not.toHaveTextContent('Sessions');
    expect(facts).not.toHaveTextContent('Last used');
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

    const facts = await within(await detail('Oh My Pi')).findByRole('list', { name: 'What Oh My Pi has on this machine' });
    await within(facts).findByText('Plugins · MCP servers');
    expect(readsOf('extensions_view')).toBe(2);
    await waitFor(() => expect(readsOf('sessions_list')).toBe(1));
    expect(readsOf('harness_session_totals')).toBe(2);
  });
});
