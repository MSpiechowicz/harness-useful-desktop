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

const DASHBOARD_COMMAND = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-dashboard/main/install.sh | sh";

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

/** Names of the commands the app sent, in order. */
const sent = () => backend.spy.mock.calls.map(([name]) => name as CommandName);

async function openSetup(): Promise<void> {
  onboarding.restart('setup');
  render(Onboarding);

  expect(await screen.findByRole('heading', { name: 'Get more from your agents' })).toBeInTheDocument();
  await screen.findByRole('region', { name: 'Harness Dashboard' });
}

describe('Recommended setup step', () => {
  it('only reads when opened, and skipping changes nothing', async () => {
    await openSetup();

    expect(sent()).toContain('setup_status');
    expect(screen.getByRole('button', { name: 'Skip for now' })).toBeEnabled();

    await click('Skip for now');
    await screen.findByRole('heading', { name: 'You’re ready to go' });

    expect(sent()).not.toContain('setup_install_dashboard');
    expect(sent()).not.toContain('setup_register_mcp');
    expect(sent()).not.toContain('extension_action');
  });

  it('does not mark the OMP approval notice as seen when a re-run starts past it', async () => {
    await openSetup();
    onboarding.choose('omp');

    await click('Skip for now');
    await screen.findByRole('heading', { name: 'You’re ready to go' });
    await click('Open Harness Desktop');

    await waitFor(() => expect(prefs.value?.onboardingCompleted).toBe(true));
    expect(prefs.value?.approvalNoticeSeen).toBe(false);
  });

  it('lists every command and file in a confirmation, and Escape closes only that', async () => {
    await openSetup();

    await click('Install missing');
    const confirmation = await screen.findByRole('dialog', { name: 'Run the recommended setup?' });

    expect(within(confirmation).getByText(DASHBOARD_COMMAND)).toBeInTheDocument();
    expect(within(confirmation).getByText('omp plugin marketplace add MSpiechowicz/harness-useful-skills')).toBeInTheDocument();
    expect(within(confirmation).getByText('omp plugin install harness-useful-skills@harness-useful-skills --scope user')).toBeInTheDocument();
    expect(within(confirmation).getByText('~/.omp/agent/mcp.json')).toBeInTheDocument();
    expect(within(confirmation).getByText(/Appends a line that puts ~\/\.local\/bin on your PATH/)).toBeInTheDocument();

    await fireEvent.keyDown(confirmation, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Run the recommended setup?' })).toBeNull());
    expect(screen.getByRole('heading', { name: 'Get more from your agents' })).toBeInTheDocument();
    expect(sent()).not.toContain('setup_install_dashboard');
  });

  it('runs the dashboard, then the skills, then the MCP entry once confirmed, and locks the flow meanwhile', async () => {
    await openSetup();

    await click('Install missing');
    await screen.findByRole('dialog', { name: 'Run the recommended setup?' });
    await click('Run setup');

    await waitFor(() => expect(screen.getByRole('button', { name: 'Back' })).toBeDisabled());
    expect(screen.getByRole('button', { name: 'Skip for now' })).toBeDisabled();

    await waitFor(() => expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled());

    const ran = sent().filter((name) => ['setup_install_dashboard', 'extension_action', 'setup_register_mcp'].includes(name));
    // The dashboard first; the skills and the agent entry then run together, in no fixed order.
    expect(ran[0]).toBe('setup_install_dashboard');
    expect([...ran].sort()).toEqual(['extension_action', 'extension_action', 'setup_install_dashboard', 'setup_register_mcp']);
    expect(backend.calls('setup_install_dashboard')).toMatchObject([{ confirmed: true }]);
    expect(backend.calls('extension_action').map((call) => call.dryRun)).toEqual([false, false]);
    expect(backend.calls('setup_register_mcp')).toEqual([{ harness: 'omp' }]);

    expect(screen.getByRole('button', { name: 'Back' })).toBeEnabled();
    expect(within(screen.getByRole('region', { name: 'Harness Dashboard' })).getByText('Installed')).toBeInTheDocument();
    expect(within(screen.getByRole('log', { name: 'Installer output' })).getByText('Verifying checksum… ok')).toBeInTheDocument();
  });

  it('reads Continue and offers nothing to select when everything is already set up', async () => {
    window.history.replaceState(null, '', '/?setup=configured');
    await openSetup();

    expect(await screen.findByRole('region', { name: 'Your harnesses' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled();
    // Nothing is missing, so there is no action to offer.
    expect(screen.queryByRole('button', { name: 'Install missing' })).toBeNull();
    expect(screen.queryAllByRole('switch')).toHaveLength(0);
  });

  it('shows what the installer printed when it fails, and still lets you go on', async () => {
    window.history.replaceState(null, '', '/?setup=fail');
    await openSetup();

    await click('Install missing');
    await screen.findByRole('dialog', { name: 'Run the recommended setup?' });
    await click('Run setup');

    const log = await screen.findByRole('log', { name: 'Installer output' });
    await waitFor(() => expect(within(log).getByText('curl: (22) The requested URL returned error: 404')).toBeInTheDocument());
    expect(await screen.findByText(/The installer did not finish: The installer exited with code 22\./)).toBeInTheDocument();

    // The agent entry needs the dashboard, so it was not attempted; the skills do not, so they went through.
    await waitFor(() => expect(screen.getByRole('button', { name: 'Continue' })).toBeEnabled());
    expect(screen.getByRole('button', { name: 'Try again' })).toBeEnabled();
    expect(sent()).not.toContain('setup_register_mcp');
    expect(screen.getByText('Skipped')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Back' })).toBeEnabled();
  });

  it('shows one row per harness the backend lists, and installs everything that is missing with no choosing', async () => {
    window.history.replaceState(null, '', '/?setup=many');
    await harness.load();
    await openSetup();

    const matrix = await screen.findByRole('region', { name: 'Your harnesses' });
    await within(matrix).findByText('OpenCode');

    const rows = within(matrix).getAllByRole('listitem');
    expect(rows.map((row) => within(row).getAllByText(/^(Oh My Pi|Pi|Claude Code|Codex|OpenCode)$/)[0]?.textContent)).toEqual(['Oh My Pi', 'Pi', 'Claude Code', 'Codex', 'OpenCode']);

    // Nothing to tick: every missing piece is part of the one action.
    expect(screen.queryAllByRole('switch')).toHaveLength(0);
    expect(screen.getByText('4 items to install')).toBeInTheDocument();
  });

  it('opens the rows that need attention and keeps the others closed until asked', async () => {
    window.history.replaceState(null, '', '/?setup=many');
    await harness.load();
    await openSetup();

    await screen.findByText('OpenCode');

    // Pi's agent file is refused, so its snippet is in view without asking.
    expect(await screen.findByText(/The file is not valid JSON\./)).toBeInTheDocument();

    // Codex only has a note, so it waits for the disclosure.
    expect(screen.queryByText('Not supported yet.')).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Details: Codex' }));
    expect(await screen.findByText('Not supported yet.')).toBeInTheDocument();
  });
});
