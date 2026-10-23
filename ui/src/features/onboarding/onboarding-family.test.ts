import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { CommandName } from '$lib/ipc/commands';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { freshBackend, type Backend } from '../../../tests/ecosystem-backend';
import { intro } from '../intro/intro-state.svelte';
import Onboarding from './Onboarding.svelte';
import { onboarding } from './onboarding-state.svelte';

let backend: Backend;

beforeEach(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  backend = freshBackend();

  await prefs.load();
  // happy-dom has no real Web Animations; with motion off every Svelte transition is zero-length.
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
  intro.skip();
});

afterEach(() => {
  window.history.replaceState(null, '', '/');
  vi.restoreAllMocks();
});

const click = (name: string | RegExp) => fireEvent.click(screen.getByRole('button', { name }));

const sent = () => backend.spy.mock.calls.map(([name]) => name as CommandName);

async function openFamily(): Promise<void> {
  onboarding.restart('family');
  render(Onboarding);

  expect(await screen.findByRole('heading', { name: 'Meet the Harness family' })).toBeInTheDocument();
  await screen.findByRole('region', { name: 'Harness Skills' });
}

describe('Meet the Harness family step', () => {
  it('shows a card of the same build for the dashboard, the memory server and the skills, and installs nothing', async () => {
    await openFamily();

    const dashboard = screen.getByRole('region', { name: 'Harness Dashboard' });
    const memory = screen.getByRole('region', { name: 'Harness Memory' });
    const skills = screen.getByRole('region', { name: 'Harness Skills' });

    expect(within(dashboard).getByText('Not installed')).toBeInTheDocument();
    expect(within(memory).getByText('Not installed')).toBeInTheDocument();
    expect(within(skills).getByText('Not installed')).toBeInTheDocument();
    expect(within(memory).getByText(/Installs the Harness Memory MCP server, so every agent can look up and add notes/)).toBeInTheDocument();
    expect(within(skills).getByText(/Skills that make an agent’s work repeatable/)).toBeInTheDocument();

    // Each card keeps its facts behind the same disclosure.
    for (const card of [dashboard, memory, skills]) {
      expect(within(card).getByRole('button', { name: /^Details/ })).toBeInTheDocument();
    }

    // The harness table belongs to the next step.
    expect(screen.queryByRole('region', { name: 'MCP servers' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Install and continue' })).toBeNull();
    expect(sent()).toContain('setup_status');
    expect(sent().filter((name) => name.startsWith('setup_install') || name === 'extension_action')).toEqual([]);
  });

  it('says plainly what each installer changes, and what the skills run in each harness', async () => {
    await openFamily();

    const memory = screen.getByRole('region', { name: 'Harness Memory' });
    expect(within(memory).getByRole('button', { name: /^Details/ })).toHaveTextContent('4 changes');

    await fireEvent.click(within(memory).getByRole('button', { name: /^Details/ }));

    expect(memory).toHaveTextContent('raw.githubusercontent.com/MSpiechowicz/harness-useful-memory/main/install.sh');
    expect(within(memory).getByText(/Installs the harness-memory command to/)).toBeInTheDocument();
    expect(within(memory).getByText('harness-memory mcp')).toBeInTheDocument();

    const skills = screen.getByRole('region', { name: 'Harness Skills' });
    // OMP's two commands, and the five things OpenCode's own installer changes.
    expect(within(skills).getByRole('button', { name: /^Details/ })).toHaveTextContent('7 changes');

    await fireEvent.click(within(skills).getByRole('button', { name: /^Details/ }));

    expect(skills).toHaveTextContent('omp plugin marketplace add MSpiechowicz/harness-useful-skills');
    expect(skills).toHaveTextContent('omp plugin install harness-useful-skills@harness-useful-skills --scope user');
    expect(skills).toHaveTextContent('harness-useful-skills/main/install.sh');
    expect(within(skills).getByText(/Adds the/)).toHaveTextContent('harness-useful-skills MCP server to opencode.json');
  });

  it('says why the skills cannot be installed for OpenCode when Node.js is missing, instead of leaving it out', async () => {
    window.history.replaceState(null, '', '/?setup=noNode');
    await openFamily();

    // Only OMP's two commands are left to run once the fresh reading is in.
    const skills = screen.getByRole('region', { name: 'Harness Skills' });
    await waitFor(() => expect(within(skills).getByRole('button', { name: /^Details/ })).toHaveTextContent('2 changes'));
    await fireEvent.click(within(skills).getByRole('button', { name: /^Details/ }));

    expect(await within(skills).findByText('Needs Node.js 22 or newer.')).toBeInTheDocument();
    expect(within(skills).getByText('Not available')).toBeInTheDocument();
    expect(skills).not.toHaveTextContent('install.sh');
  });

  it('shows what is installed already, and leaves the Continue and Back buttons working', async () => {
    window.history.replaceState(null, '', '/?setup=configured');
    await openFamily();

    await waitFor(() => expect(within(screen.getByRole('region', { name: 'Harness Skills' })).getByText('Installed')).toBeInTheDocument());
    // OpenCode's installer records its own version, which its row shows.
    await fireEvent.click(within(screen.getByRole('region', { name: 'Harness Skills' })).getByRole('button', { name: /^Details/ }));
    expect(await screen.findByText('Version 0.2.40')).toBeInTheDocument();
    expect(within(screen.getByRole('region', { name: 'Harness Dashboard' })).getByText('Installed')).toBeInTheDocument();
    expect(within(screen.getByRole('region', { name: 'Harness Memory' })).getByText('Installed')).toBeInTheDocument();

    await click('Continue');
    expect(await screen.findByRole('heading', { name: 'Get more from your agents' })).toBeInTheDocument();

    await click('Back');
    expect(await screen.findByRole('heading', { name: 'Meet the Harness family' })).toBeInTheDocument();
  });

  it('sits right before the set-up step, adding one step to the flow', async () => {
    expect(onboarding.steps.indexOf('family')).toBe(onboarding.steps.indexOf('setup') - 1);
    expect(onboarding.steps.at(-1)).toBe('done');
    expect(onboarding.steps).toHaveLength(onboarding.needsApprovalNotice ? 8 : 7);
  });
});
