import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { HarnessSetup } from '$lib/ipc/bindings/HarnessSetup';
import type { RecommendedSetup } from '$lib/ipc/bindings/RecommendedSetup';
import type { CommandName } from '$lib/ipc/commands';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { freshBackend, type Backend } from '../../../tests/ecosystem-backend';
import { intro } from '../intro/intro-state.svelte';
import Onboarding from './Onboarding.svelte';
import { onboarding } from './onboarding-state.svelte';

const CODEX_SKILLS_COMMAND = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-skills/main/install.sh | HARNESS_SKILLS_HOST=codex sh";

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

/** Opens the first-run flow on the family step, with the scenario of the page URL. */
async function openFamily(scenario: string): Promise<void> {
  window.history.replaceState(null, '', `/?setup=${scenario}`);
  await harness.load();
  onboarding.restart('family');
  render(Onboarding);

  await screen.findByRole('region', { name: 'Harness Skills' });
}

/** Moves on to the set-up step. */
async function openSetup(scenario: string): Promise<void> {
  await openFamily(scenario);
  await click('Continue');

  await screen.findByRole('region', { name: 'Harness family' });
}

/** Answers `setup_status` with Codex's skills changed, everything else as the mock reads it. */
function withCodexSkills(skills: HarnessSetup['skills']): void {
  const real = backend.spy.getMockImplementation()!;

  backend.spy.mockImplementation(async (name, args) => {
    const result = await real(name, args);

    if (name !== 'setup_status') {
      return result;
    }

    const status = result as RecommendedSetup;

    return { ...status, harnesses: status.harnesses.map((entry) => (entry.harness === 'codex' ? { ...entry, skills } : entry)) };
  });
}

/** The footer's install action, once the machine has been read and it is enabled. */
async function install(): Promise<void> {
  const button = await screen.findByRole('button', { name: 'Install and continue' });
  await waitFor(() => expect(button).toBeEnabled());

  await fireEvent.click(button);
}

const family = () => within(screen.getByRole('region', { name: 'Harness family' }));
const skillsCard = () => within(screen.getByRole('region', { name: 'Harness Skills' }));

describe('Harness Skills for Codex in the recommended setup', () => {
  it('lists the Codex installer and what it changes in the confirmation, and runs nothing before it', async () => {
    await openSetup('many');

    await install();
    const confirmation = await screen.findByRole('dialog', { name: 'Run the recommended setup?' });

    expect(within(confirmation).getByText(CODEX_SKILLS_COMMAND)).toBeInTheDocument();
    const effects = within(confirmation).getByRole('region', { name: 'The Harness Skills installer for Codex also' });
    expect(within(effects).getByText('Registers the useful_skills MCP server with codex mcp add.')).toBeInTheDocument();
    expect(within(effects).getByText('Needs Node.js 22 or newer and the codex command.')).toBeInTheDocument();
    expect(sent()).not.toContain('setup_install_codex_skills');

    await fireEvent.click(within(confirmation).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Run the recommended setup?' })).toBeNull());
    expect(sent()).not.toContain('setup_install_codex_skills');
  });

  it('installs them on the confirmation with their own output, and reads the recorded version afterwards', async () => {
    await openSetup('many');

    await install();
    await screen.findByRole('dialog', { name: 'Run the recommended setup?' });
    await click('Run setup');

    // The run is over once the flow goes on by itself.
    await screen.findByRole('heading', { name: 'You’re ready to go' });
    expect(backend.calls('setup_install_codex_skills')).toMatchObject([{ confirmed: true }]);

    // Back on the step, their output waits behind the card's Details, under Codex's name.
    await click('Back');
    await screen.findByRole('region', { name: 'Harness family' });
    await fireEvent.click(family().getByRole('button', { name: /^Details/ }));
    const output = family().getByRole('region', { name: 'Codex' });
    expect(within(output).getByText(/Harness Skills 0\.3\.13 installed/)).toBeInTheDocument();

    // The next reading holds the version the installer recorded.
    await click('Back');
    await screen.findByRole('region', { name: 'Harness Skills' });
    await fireEvent.click(skillsCard().getByRole('button', { name: /^Details/ }));
    expect(await skillsCard().findByText('Version 0.3.13')).toBeInTheDocument();
  });

  it('offers the installer as an update for an install older than the first Codex release', async () => {
    withCodexSkills({ type: 'script', command: CODEX_SKILLS_COMMAND, sideEffects: ['Adds the Harness Skills marketplace and plugin to Codex.'], outdatedFrom: '0.3.12' });
    await openFamily('many');

    const codexRow = () => skillsCard().getByText('Codex').closest('li') as HTMLElement;
    await fireEvent.click(skillsCard().getByRole('button', { name: /^Details/ }));

    await waitFor(() => expect(within(codexRow()).getByText('Update available')).toBeInTheDocument());
    expect(within(codexRow()).getByText('Version 0.3.12')).toBeInTheDocument();
    expect(codexRow()).toHaveTextContent(CODEX_SKILLS_COMMAND);

    await click('Continue');
    await install();
    await screen.findByRole('dialog', { name: 'Run the recommended setup?' });
    await click('Run setup');

    await screen.findByRole('heading', { name: 'You’re ready to go' });
    expect(backend.calls('setup_install_codex_skills')).toMatchObject([{ confirmed: true }]);
  });

  it('does not run their installer for Codex when it has them already', async () => {
    withCodexSkills({ type: 'installed', version: '0.3.13' });
    await openSetup('many');

    await install();
    await screen.findByRole('dialog', { name: 'Run the recommended setup?' });
    await click('Run setup');
    await screen.findByRole('heading', { name: 'You’re ready to go' });

    expect(backend.calls('extension_action').length).toBeGreaterThan(0);
    expect(sent()).not.toContain('setup_install_codex_skills');
  });

  it('says why they cannot be installed when Node.js is missing, with no installer for them', async () => {
    withCodexSkills({ type: 'unavailable', reason: 'Needs Node.js 22 or newer.', code: 'nodeMissing' });
    await openSetup('many');

    await fireEvent.click(await family().findByRole('button', { name: /^Details/ }));
    const codex = await family().findByRole('region', { name: 'Codex' });
    expect(within(codex).getByText('Needs Node.js 22 or newer.')).toBeInTheDocument();

    expect(sent()).not.toContain('setup_install_codex_skills');
  });
});
