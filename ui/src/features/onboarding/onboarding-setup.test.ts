import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { CommandName } from '$lib/ipc/commands';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { freshBackend, type Backend } from '../../../tests/ecosystem-backend';
import { withoutChat } from '../../../tests/helpers';
import { intro } from '../intro/intro-state.svelte';
import Onboarding from './Onboarding.svelte';
import { onboarding } from './onboarding-state.svelte';

const DASHBOARD_COMMAND = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-dashboard/main/install.sh | sh";
const OPENCODE_SKILLS_COMMAND = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-skills/main/install.sh | sh";
const MEMORY_COMMAND = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-memory/main/install.sh | sh";

let backend: Backend;

beforeEach(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  backend = freshBackend();

  await prefs.load();
  // happy-dom has no real Web Animations; with motion off every Svelte transition is zero-length.
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
  // OpenCode stands in for a harness the app cannot chat with.
  withoutChat(harness, 'opencode');
  intro.skip();
});

afterEach(() => {
  window.history.replaceState(null, '', '/');
  vi.restoreAllMocks();
});

const click = (name: string | RegExp) => fireEvent.click(screen.getByRole('button', { name }));

/** The footer's actions on the set-up step, which are there once the machine has been read. */
const primary = async (name: 'Install and continue' | 'Continue anyway' | 'Continue') => {
  const button = await screen.findByRole('button', { name });
  // It waits while the machine is read again, so a click only lands once it is ready.
  await waitFor(() => expect(button).toBeEnabled());

  return button;
};
const skip = () => screen.findByRole('button', { name: 'Skip for now' });

/** Names of the commands the app sent, in order. */
const sent = () => backend.spy.mock.calls.map(([name]) => name as CommandName);

/** The way in: the family step, then Continue to the set-up step, where the family's rows and the MCP table are. */
async function openSetup(): Promise<void> {
  onboarding.restart('family');
  render(Onboarding);

  await screen.findByRole('region', { name: 'Harness Skills' });
  await click('Continue');

  expect(await screen.findByRole('heading', { name: 'Get more from your agents' })).toBeInTheDocument();
  await screen.findByRole('region', { name: 'Harness family' });
}

/** One installer's row on the set-up step. */
const installer = (name: 'Harness Dashboard' | 'Harness Memory' | 'Harness Skills') => within(screen.getByRole('region', { name: 'Harness family' })).getByRole('group', { name });

/** The Harness family card, whose Details hold the installers' output and the notes on the skills. */
const family = () => within(screen.getByRole('region', { name: 'Harness family' }));
const familyDetails = () => family().queryByRole('button', { name: /^Details/ });
const openFamilyDetails = async () => fireEvent.click(family().getByRole('button', { name: /^Details/ }));
/** One installer's block of the opened Details, found by its heading. */
const output = (name: string) => within(family().getByRole('region', { name })).getByRole('log', { name: 'Installer output' });

describe('Set up step', () => {
  it('only reads when opened, and skipping changes nothing', async () => {
    await openSetup();

    expect(sent()).toContain('setup_status');
    // Installing is the way forward; skipping is the quieter way out.
    expect(await primary('Install and continue')).toBeEnabled();
    expect(await skip()).toBeEnabled();

    await fireEvent.click(await skip());
    await screen.findByRole('heading', { name: 'You’re ready to go' });

    expect(sent()).not.toContain('setup_install_dashboard');
    expect(sent()).not.toContain('setup_install_memory');
    expect(sent()).not.toContain('setup_register_mcp');
    expect(sent()).not.toContain('setup_register_memory');
    expect(sent()).not.toContain('extension_action');
  });

  it('does not mark the OMP approval notice as seen when a re-run starts past it', async () => {
    await openSetup();
    onboarding.choose('omp');

    await fireEvent.click(await skip());
    await screen.findByRole('heading', { name: 'You’re ready to go' });
    await click('Open Harness Desktop');

    await waitFor(() => expect(prefs.value?.onboardingCompleted).toBe(true));
    expect(prefs.value?.approvalNoticeSeen).toBe(false);
  });

  it('lists every command and file in a confirmation, and Escape closes only that', async () => {
    await openSetup();

    await fireEvent.click(await primary('Install and continue'));
    const confirmation = await screen.findByRole('dialog', { name: 'Run the recommended setup?' });

    expect(within(confirmation).getByText(DASHBOARD_COMMAND)).toBeInTheDocument();
    expect(within(confirmation).getByText(MEMORY_COMMAND)).toBeInTheDocument();
    expect(within(confirmation).getByText('omp plugin marketplace add MSpiechowicz/harness-useful-skills')).toBeInTheDocument();
    expect(within(confirmation).getByText('omp plugin install harness-useful-skills@harness-useful-skills --scope user')).toBeInTheDocument();
    // Each installed harness's MCP file gets the dashboard's, the memory server's and the app's own entries: listed once, with all three.
    // OpenCode is coming soon, so its file is not touched.
    const files = within(confirmation).getByRole('region', { name: 'Files' });
    const listed = within(files).getAllByRole('listitem');
    expect(listed).toHaveLength(1);
    expect(within(listed[0]).getByText('~/.omp/agent/mcp.json')).toBeInTheDocument();
    expect(within(listed[0]).getByText('adds 3 entries')).toBeInTheDocument();
    // Each installer's own changes, under its own heading; the memory server also says it adds itself to the harnesses.
    const dashboardEffects = within(confirmation).getByRole('region', { name: 'The dashboard installer also' });
    expect(within(dashboardEffects).getByText(/Appends a line that puts ~\/\.local\/bin on your PATH/)).toBeInTheDocument();
    // OpenCode's skills come from their own installer, which the confirmation lists like the other two.
    expect(within(confirmation).getByText(OPENCODE_SKILLS_COMMAND)).toBeInTheDocument();
    const skillsEffects = within(confirmation).getByRole('region', { name: 'The Harness Skills installer also' });
    expect(within(skillsEffects).getByText('Needs Node.js 22 or newer.')).toBeInTheDocument();
    const memoryEffects = within(confirmation).getByRole('region', { name: 'The memory installer also' });
    expect(within(memoryEffects).getByText(/Installs the harness-memory command to/)).toBeInTheDocument();
    expect(within(memoryEffects).getByText(/as an MCP server in each detected harness/)).toBeInTheDocument();

    await fireEvent.keyDown(confirmation, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Run the recommended setup?' })).toBeNull());
    expect(screen.getByRole('heading', { name: 'Get more from your agents' })).toBeInTheDocument();
    expect(sent()).not.toContain('setup_install_dashboard');
  });

  it('runs the skills alongside the two installers, each MCP entry after its own server, and locks the flow meanwhile', async () => {
    await prefs.setMemoryEnabled(false);
    await openSetup();

    expect(screen.getByText('7 items to install')).toBeInTheDocument();

    await fireEvent.click(await primary('Install and continue'));
    await screen.findByRole('dialog', { name: 'Run the recommended setup?' });
    await click('Run setup');

    await waitFor(() => expect(screen.getByRole('button', { name: 'Back' })).toBeDisabled());
    expect(screen.getByRole('button', { name: 'Skip for now' })).toBeDisabled();

    // Once the run is over the step goes on by itself.
    await screen.findByRole('heading', { name: 'You’re ready to go' });

    const ran = sent().filter((name) => ['setup_install_dashboard', 'setup_install_memory', 'setup_install_opencode_skills', 'extension_action', 'setup_register_mcp', 'setup_register_memory', 'setup_register_desktop_mcp'].includes(name));
    // The two installers one after the other while the skills already run; each MCP entry after its own server's
    // installer, and the memory server's entry after the dashboard's (they go into the same file).
    const at = (name: (typeof ran)[number]) => ran.indexOf(name);
    expect(at('setup_install_dashboard')).toBeLessThan(at('setup_install_memory'));
    expect(at('extension_action')).toBeLessThan(at('setup_install_memory'));
    expect(at('setup_install_dashboard')).toBeLessThan(at('setup_register_mcp'));
    expect(at('setup_install_memory')).toBeLessThan(at('setup_register_memory'));
    expect([...ran].sort()).toEqual([
      'extension_action',
      'extension_action',
      'setup_install_dashboard',
      'setup_install_memory',
      'setup_install_opencode_skills',
      'setup_register_desktop_mcp',
      'setup_register_mcp',
      'setup_register_memory',
    ]);
    // The three entries go into the same file one after the other, and the app's own needs no installer.
    expect(ran.indexOf('setup_register_mcp')).toBeLessThan(ran.indexOf('setup_register_memory'));
    expect(ran.indexOf('setup_register_memory')).toBeLessThan(ran.indexOf('setup_register_desktop_mcp'));
    expect(backend.calls('setup_install_dashboard')).toMatchObject([{ confirmed: true }]);
    expect(backend.calls('setup_install_memory')).toMatchObject([{ confirmed: true }]);
    // The skills' installer for OpenCode runs on the same confirmation, and does not wait for the other two.
    expect(backend.calls('setup_install_opencode_skills')).toMatchObject([{ confirmed: true }]);
    expect(backend.calls('extension_action').map((call) => call.dryRun)).toEqual([false, false]);
    // OpenCode is coming soon, so only OMP gets the entries.
    expect(backend.calls('setup_register_mcp')).toEqual([{ harness: 'omp' }]);
    expect(backend.calls('setup_register_memory')).toEqual([{ harness: 'omp' }]);
    expect(backend.calls('setup_register_desktop_mcp')).toEqual([{ harness: 'omp' }]);
    // Registering the memory server turned Project memory on, and Settings knows.
    expect(prefs.value?.memoryEnabled).toBe(true);

    expect(screen.getByRole('button', { name: 'Back' })).toBeEnabled();
    expect(await screen.findByText('Dashboard, Memory, Skills, Dashboard MCP, Memory MCP, Desktop MCP')).toBeInTheDocument();

    // Back on the step, each installer kept its own output and state.
    await click('Back');
    await screen.findByRole('region', { name: 'Harness family' });
    const dashboard = installer('Harness Dashboard');
    const memory = installer('Harness Memory');
    expect(within(dashboard).getByText('Installed')).toBeInTheDocument();
    expect(within(memory).getByText('Installed')).toBeInTheDocument();
    expect(within(installer('Harness Skills')).getByText('Installed')).toBeInTheDocument();
    // Each installer keeps its own output, here on the step that ran it, behind the card's Details and not in the rows.
    expect(within(dashboard).queryByRole('log')).toBeNull();
    expect(family().queryByRole('log')).toBeNull();
    await openFamilyDetails();
    expect(within(output('Harness Dashboard')).getByText('Verifying checksum… ok')).toBeInTheDocument();
    expect(within(output('Harness Memory')).getByText('==> Checksum verified')).toBeInTheDocument();
    expect(within(output('OpenCode')).getByText(/Harness Skills 0\.2\.40 installed/)).toBeInTheDocument();

    // The status was read again: OpenCode's row on the family step now holds the version its installer recorded.
    await click('Back');
    await screen.findByRole('region', { name: 'Harness Skills' });
    await fireEvent.click(within(screen.getByRole('region', { name: 'Harness Skills' })).getByRole('button', { name: /^Details/ }));
    expect(await screen.findByText('Version 0.2.40')).toBeInTheDocument();
  });

  it('does not run OpenCode’s skills installer before the confirmation, and not when it is declined', async () => {
    await openSetup();

    await fireEvent.click(await primary('Install and continue'));
    const confirmation = await screen.findByRole('dialog', { name: 'Run the recommended setup?' });
    expect(sent()).not.toContain('setup_install_opencode_skills');

    await fireEvent.click(within(confirmation).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Run the recommended setup?' })).toBeNull());
    expect(sent()).not.toContain('setup_install_opencode_skills');
  });

  it('says why OpenCode’s skills cannot be installed when Node.js is missing, and offers no installer for them', async () => {
    window.history.replaceState(null, '', '/?setup=noNode');
    await openSetup();

    // The reason is a note, so it waits in the family card's Details.
    await waitFor(() => expect(familyDetails()).toBeInTheDocument());
    expect(screen.queryByText('Needs Node.js 22 or newer.')).toBeNull();
    await openFamilyDetails();
    const opencode = family().getByRole('region', { name: 'OpenCode' });
    expect(within(opencode).getByText('Needs Node.js 22 or newer.')).toBeInTheDocument();
    expect(within(opencode).getByRole('heading', { name: 'OpenCode' })).toBeInTheDocument();

    await fireEvent.click(await primary('Install and continue'));
    const confirmation = await screen.findByRole('dialog', { name: 'Run the recommended setup?' });
    expect(within(confirmation).queryByText(OPENCODE_SKILLS_COMMAND)).toBeNull();
    await click('Run setup');
    await screen.findByRole('heading', { name: 'You’re ready to go' });

    expect(sent()).not.toContain('setup_install_opencode_skills');
  });

  it('reads Continue and offers nothing to select when everything is already set up', async () => {
    window.history.replaceState(null, '', '/?setup=configured');
    await openSetup();

    expect(within(installer('Harness Memory')).getByText('Installed')).toBeInTheDocument();
    expect(within(installer('Harness Skills')).getByText('Installed')).toBeInTheDocument();
    // Codex is found where the skills cannot go, which is a note, so Details is there and closed.
    expect(familyDetails()).toHaveAttribute('aria-expanded', 'false');

    expect(await screen.findByRole('region', { name: 'MCP servers' })).toBeInTheDocument();
    expect(await primary('Continue')).toBeEnabled();
    // Nothing is missing, so there is nothing to install or to skip.
    expect(screen.queryByRole('button', { name: 'Install and continue' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Skip for now' })).toBeNull();
    expect(screen.queryAllByRole('switch')).toHaveLength(0);
  });

  it('has no Details on the family card when nothing ran and there is nothing to note', async () => {
    window.history.replaceState(null, '', '/?setup=configured');
    const real = backend.spy.getMockImplementation()!;
    backend.spy.mockImplementation(async (name, args) => {
      const result = await real(name, args);

      return name === 'setup_status' ? { ...result, harnesses: result.harnesses.filter((entry: { skills: { type: string } }) => entry.skills.type !== 'unavailable') } : result;
    });
    await openSetup();

    expect(within(installer('Harness Skills')).getByText('Installed')).toBeInTheDocument();
    expect(familyDetails()).toBeNull();
  });

  it('shows what the installer printed when it fails, and still lets you go on', async () => {
    window.history.replaceState(null, '', '/?setup=fail');
    await openSetup();

    await fireEvent.click(await primary('Install and continue'));
    await screen.findByRole('dialog', { name: 'Run the recommended setup?' });
    await click('Run setup');

    const dashboard = installer('Harness Dashboard');
    expect(familyDetails()).toHaveAttribute('aria-expanded', 'false');
    // The failure line is in the row without opening anything, the output is in Details.
    expect(await within(dashboard).findByText(/The installer did not finish: The installer exited with code 22\./)).toBeInTheDocument();
    // The memory server's installer failed too, on its own row, and did not stop the run.
    const memory = installer('Harness Memory');
    await waitFor(() => expect(within(memory).getByText(/The installer did not finish: error: download failed/)).toBeInTheDocument());
    expect(family().queryByRole('log')).toBeNull();
    await openFamilyDetails();
    await waitFor(() => expect(within(output('Harness Dashboard')).getByText('curl: (22) The requested URL returned error: 404')).toBeInTheDocument());
    expect(within(output('Harness Memory')).getByText('error: download failed')).toBeInTheDocument();

    // Each MCP entry needs its own companion, so neither was attempted; the skills do not, so OMP's went through while OpenCode's own installer failed too.
    // A failed run keeps the step on screen: the forward action is now "Continue anyway", and skipping is no longer offered.
    const anyway = await primary('Continue anyway');
    await waitFor(() => expect(anyway).toBeEnabled());
    expect(screen.queryByRole('button', { name: 'Skip for now' })).toBeNull();
    expect(screen.getByRole('heading', { name: 'Get more from your agents' })).toBeInTheDocument();
    expect(backend.calls('extension_action')).toHaveLength(2);
    const skills = installer('Harness Skills');
    expect(await within(skills).findByText('OpenCode: could not install: error: could not download the Harness Skills package')).toBeInTheDocument();
    expect(within(output('OpenCode')).getByText('error: could not download the Harness Skills package')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Try again' })).toHaveLength(3);
    expect(sent()).not.toContain('setup_register_mcp');
    expect(sent()).not.toContain('setup_register_memory');
    const omp = within(screen.getByRole('region', { name: 'MCP servers' })).getAllByRole('listitem')[0];
    expect(within(within(omp).getByRole('group', { name: 'Harness Dashboard MCP' })).getByText('Skipped')).toBeInTheDocument();
    expect(within(within(omp).getByRole('group', { name: 'Harness Memory MCP' })).getByText('Skipped')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Back' })).toBeEnabled();

    await fireEvent.click(anyway);
    expect(await screen.findByRole('heading', { name: 'You’re ready to go' })).toBeInTheDocument();
  });

  it('shows one row per harness the backend lists, and installs everything that is missing with no choosing', async () => {
    window.history.replaceState(null, '', '/?setup=many');
    await harness.load();
    await openSetup();

    const matrix = await screen.findByRole('region', { name: 'MCP servers' });
    const names = () => within(matrix).getAllByRole('listitem').map((row) => within(row).getAllByText(/^(Oh My Pi|Pi|Claude Code|Codex|OpenCode)$/)[0]?.textContent);
    // Every harness is in every reading, but only this scenario's lists OpenCode last, so waiting for that order waits for the fresh one.
    await waitFor(() => expect(names()).toEqual(['Oh My Pi', 'Pi', 'Claude Code', 'Codex', 'OpenCode']));

    const rows = within(matrix).getAllByRole('listitem');

    // Nothing to tick: every missing piece is part of the one action.
    expect(screen.queryAllByRole('switch')).toHaveLength(0);
    expect(screen.getByText('7 items to install')).toBeInTheDocument();

    // The table is MCP servers only: skills are one row of the family card, summed across the harnesses found.
    expect(within(rows[0]).queryByRole('group', { name: 'Harness Skills' })).toBeNull();
    expect(within(rows[0]).getAllByRole('group')).toHaveLength(3);
    expect(within(installer('Harness Skills')).getByText('Partly installed')).toBeInTheDocument();
    expect(screen.getByText('Installs the skills in Oh My Pi, Pi, and Claude Code')).toBeInTheDocument();

    // A column per MCP server, with the same states.
    // Claude Code chats now, so its MCP entries show their state like the others' (its memory entry is switched off here).
    const claude = rows[2];
    expect(within(within(claude).getByRole('group', { name: 'Harness Dashboard MCP' })).getByText('Installed')).toBeInTheDocument();
    expect(within(within(claude).getByRole('group', { name: 'Harness Memory MCP' })).getByText('Disabled')).toBeInTheDocument();
    expect(within(within(claude).getByRole('group', { name: 'Harness Desktop MCP' })).getByText('Installed')).toBeInTheDocument();
    // The app cannot chat with Codex yet, so it gets no MCP entries and every column says so.
    const codex = rows[3];
    expect(within(within(codex).getByRole('group', { name: 'Harness Dashboard MCP' })).getByText('Coming soon')).toBeInTheDocument();
    expect(within(within(codex).getByRole('group', { name: 'Harness Memory MCP' })).getByText('Coming soon')).toBeInTheDocument();
    expect(within(within(codex).getByRole('group', { name: 'Harness Desktop MCP' })).getByText('Coming soon')).toBeInTheDocument();
  });

  it('adds only the memory server’s missing entries when it is installed already', async () => {
    window.history.replaceState(null, '', '/?setup=memory');
    await openSetup();

    const memoryCell = (row: number) => {
      const rows = within(screen.getByRole('region', { name: 'MCP servers' })).getAllByRole('listitem');

      return within(rows[row]).getByRole('group', { name: 'Harness Memory MCP' });
    };
    // OMP has the entry, PI does not.
    await waitFor(() => expect(memoryCell(0)).toHaveTextContent('Installed'));
    expect(memoryCell(1)).toHaveTextContent('Not installed');

    await fireEvent.click(await primary('Install and continue'));
    const confirmation = await screen.findByRole('dialog', { name: 'Run the recommended setup?' });
    expect(within(confirmation).queryByText(MEMORY_COMMAND)).toBeNull();
    await click('Run setup');

    await screen.findByRole('heading', { name: 'You’re ready to go' });
    expect(sent()).not.toContain('setup_install_memory');
    expect(backend.calls('setup_register_memory')).toEqual([{ harness: 'pi' }]);

    await click('Back');
    await waitFor(() => expect(memoryCell(1)).toHaveTextContent('Installed'));
    expect(memoryCell(1)).toHaveTextContent('Installed');
  });

  it('opens the rows that need attention and keeps the others closed until asked', async () => {
    window.history.replaceState(null, '', '/?setup=many');
    await harness.load();
    await openSetup();

    await within(await screen.findByRole('region', { name: 'MCP servers' })).findByText('Codex');

    // Pi's agent file is refused, so each column's snippet is in view without asking.
    expect(await screen.findAllByText(/The file is not valid JSON\./)).toHaveLength(3);
    expect(screen.getByText('"harness-memory":')).toBeInTheDocument();
    expect(screen.getByText('"harness-desktop":')).toBeInTheDocument();

    // The app cannot chat with Codex yet, so its row has nothing to disclose.
    expect(screen.queryByRole('button', { name: 'Details: Codex' })).toBeNull();
    // A switched-off entry only has a note, so it waits for the disclosure.
    expect(screen.queryByText(/It is switched off in the file/)).toBeNull();
    await fireEvent.click(screen.getByRole('button', { name: 'Details: Claude Code' }));
    expect(await screen.findByText(/It is switched off in the file/)).toBeInTheDocument();
    // A found harness the skills cannot be installed in is listed on the skills row with its reason.
    expect(within(installer('Harness Skills')).queryByRole('region', { name: 'Codex' })).toBeNull();
    await openFamilyDetails();
    const unavailable = family().getByRole('region', { name: 'Codex' });
    expect(within(unavailable).getByText('Harness Skills for Codex are not available yet.')).toBeInTheDocument();
  });

  it('shows OpenCode as coming soon and leaves its config alone', async () => {
    window.history.replaceState(null, '', '/?setup=manualEdit');
    await openSetup();

    const matrix = await screen.findByRole('region', { name: 'MCP servers' });
    const row = () => within(matrix).getAllByRole('listitem').find((item) => within(item).queryByText('OpenCode'))!;
    await waitFor(() => expect(within(row()).getByRole('group', { name: 'Harness Dashboard MCP' })).toHaveTextContent('Coming soon'));
    const opencode = row();
    expect(within(opencode).getByRole('group', { name: 'Harness Memory MCP' })).toHaveTextContent('Coming soon');
    // No note, snippet or file hint while it is not offered.
    expect(within(opencode).queryByRole('button', { name: 'Copy snippet' })).toBeNull();
    expect(within(opencode).queryByRole('button', { name: 'Details: OpenCode' })).toBeNull();

    // Nothing is registered there, even when the rest of the machine is set up.
    await fireEvent.click(await primary('Install and continue'));
    await screen.findByRole('dialog', { name: 'Run the recommended setup?' });
    await click('Run setup');
    await screen.findByRole('heading', { name: 'You’re ready to go' });
    expect(backend.calls('setup_register_mcp')).toEqual([{ harness: 'omp' }]);
    expect(backend.calls('setup_register_memory')).not.toContainEqual({ harness: 'opencode' });
  });

  it('shows which harness the skills failed in, on the skills row, and keeps the rest of the run going', async () => {
    const real = backend.spy.getMockImplementation()!;
    backend.spy.mockImplementation((name, args) => (name === 'extension_action' ? Promise.reject(new Error('plugin host unreachable')) : real(name, args)));
    await openSetup();

    await fireEvent.click(await primary('Install and continue'));
    await screen.findByRole('dialog', { name: 'Run the recommended setup?' });
    await click('Run setup');

    const skills = installer('Harness Skills');
    expect(await within(skills).findByText('Oh My Pi: could not install: plugin host unreachable')).toBeInTheDocument();
    expect(await within(skills).findByText('Failed')).toBeInTheDocument();
    expect(within(skills).getByRole('button', { name: 'Try again' })).toBeInTheDocument();

    // The MCP entries do not depend on the skills.
    expect(await primary('Continue anyway')).toBeEnabled();
    expect(backend.calls('setup_register_mcp')).toContainEqual({ harness: 'omp' });
    expect(backend.calls('setup_register_memory')).toContainEqual({ harness: 'omp' });
  });
});
