import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { intro } from '../intro/intro-state.svelte';
import { featureTags } from './harness-tags';
import Onboarding from './Onboarding.svelte';
import { onboarding } from './onboarding-state.svelte';

// The mock machine: Oh My Pi and OpenCode installed (both can chat), Pi, Claude Code and Codex missing (Codex has no chat yet).
beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');

  await prefs.load();
  // happy-dom has no real Web Animations; with motion off every Svelte transition is zero-length.
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
  intro.skip();
});

beforeEach(async () => {
  localStorage.clear();
  await prefs.update({ defaultHarness: null, onboardingCompleted: false });
  await harness.load();
});

const row = (name: RegExp) => screen.findByRole('option', { name });

async function openStep(step: 'meet' | 'harnesses'): Promise<void> {
  onboarding.restart(step);
  render(Onboarding);
  await screen.findByRole('heading', { name: step === 'meet' ? 'Meet your harnesses' : 'Which harness do you use every day?' });
}

describe('Meet your harnesses', () => {
  it('introduces every harness with its description and what it supports, and nothing about this machine', async () => {
    await openStep('meet');

    for (const summary of harness.summaries) {
      const card = await screen.findByRole('article', { name: summary.info.name });

      expect(card).toHaveTextContent(summary.info.description);
      // Only what the harness really supports: none of the tagged kinds yet for Claude Code.
      expect(within(card).queryAllByRole('listitem')).toHaveLength(featureTags(summary.capabilities).length);
      expect(card).not.toHaveTextContent(/Installed|Not found|Coming soon|Supported/);
      expect(card).not.toHaveTextContent(/\/home|\/usr|curl /);
    }

    // The tags come from what each harness can do: PI has packages where OMP has plugins.
    expect(within(screen.getByRole('article', { name: 'Pi' })).getByText('Packages')).toBeInTheDocument();
    expect(within(screen.getByRole('article', { name: 'Oh My Pi' })).getByText('Plugins')).toBeInTheDocument();
    expect(within(screen.getByRole('article', { name: 'Oh My Pi' })).queryByText('Packages')).toBeNull();
    // Every listed harness chats, so that says nothing about one of them.
    expect(screen.queryByText('Chat')).toBeNull();
    expect(screen.queryByRole('radio')).toBeNull();
  });

  it('names at most three things a harness supports, however many it has', async () => {
    harness.summaries = harness.summaries.map((summary) => ({
      ...summary,
      capabilities: { ...summary.capabilities, plugins: true, packages: true, mcp: true, skills: true, profiles: true },
    }));
    await openStep('meet');

    for (const summary of harness.summaries) {
      const card = await screen.findByRole('article', { name: summary.info.name });

      expect(within(card).getAllByRole('listitem')).toHaveLength(3);
    }
  });
});

describe('Which harness do you use every day?', () => {
  it('opens on the daily driver and marks it in the list', async () => {
    await openStep('harnesses');

    const driver = await row(/^Oh My Pi/);
    expect(driver).toHaveAttribute('aria-selected', 'true');
    expect(driver).toHaveTextContent('Daily driver');
    expect(screen.getByRole('article', { name: 'Oh My Pi' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'Use Oh My Pi as daily driver' })).toBeChecked();
    expect(screen.getByText('Found 2 of 5 on this machine.')).toBeInTheDocument();
    // What the harnesses can do belongs to the previous step.
    expect(screen.queryByRole('list', { name: 'What it supports' })).toBeNull();
  });

  it('keeps the details to what the list row does not say, with the binary and the config together', async () => {
    await openStep('harnesses');

    const omp = harness.get('omp');
    const detail = await screen.findByRole('article', { name: 'Oh My Pi' });

    expect(detail).not.toHaveTextContent(omp?.info.description ?? 'no description');
    expect(within(detail).queryByText('Installed')).toBeNull();
    expect(within(detail).queryByText('Not found')).toBeNull();
    expect(within(detail).getByText('Supported')).toBeInTheDocument();
    expect(within(detail).getByText(`v${omp?.detection.version}`)).toBeInTheDocument();
    expect(within(detail).getByText('Binary').parentElement).toBe(within(detail).getByText('Config').parentElement);

    await fireEvent.click(await row(/^Pi/));
    const missing = screen.getByRole('article', { name: 'Pi' });
    expect(within(missing).queryByText('Not found')).toBeNull();
    expect(missing).not.toHaveTextContent(harness.get('pi')?.info.description ?? 'no description');
    expect(within(missing).getByText('Config')).toBeInTheDocument();
  });

  it('opens on the first harness found when none is the driver yet, else the first missing one', async () => {
    harness.summaries = harness.summaries.map((summary) => ({ ...summary, detection: { ...summary.detection, installed: false } }));
    await openStep('harnesses');

    expect(await row(/^Oh My Pi/)).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('option', { name: /^Oh My Pi/ })).toHaveTextContent('Not found');
    expect(within(screen.getByRole('article', { name: 'Oh My Pi' })).queryByText('Binary')).toBeNull();
  });

  it('shows how to get a missing harness without changing the daily driver', async () => {
    await openStep('harnesses');

    const missing = await row(/^Pi/);
    expect(missing).toHaveTextContent('Not found');
    await fireEvent.click(missing);

    const detail = screen.getByRole('article', { name: 'Pi' });
    expect(missing).toHaveAttribute('aria-selected', 'true');
    expect(detail).toHaveTextContent('Install it with:');
    expect(detail).toHaveTextContent(harness.get('pi')?.detection.installHint?.commands[0]?.command ?? 'no command');
    expect(within(detail).getByRole('button', { name: 'Copy install command' })).toBeInTheDocument();
    expect(within(detail).getByRole('button', { name: 'Install docs' })).toBeEnabled();
    expect(within(detail).getByRole('button', { name: 'Choose binary…' })).toBeEnabled();
    const radio = screen.getByRole('radio', { name: 'Use Pi as daily driver' });
    expect(radio).toHaveAttribute('aria-disabled', 'true');
    expect(radio).toHaveAttribute('aria-checked', 'false');

    await fireEvent.click(radio);
    expect(radio).toHaveAttribute('aria-checked', 'false');
    expect(onboarding.driver).toBe('omp');
    expect(await row(/^Oh My Pi/)).toHaveTextContent('Daily driver');
  });

  it('offers every release line of a missing harness, each labelled with its own copy button', async () => {
    harness.summaries = harness.summaries.map((summary) =>
      summary.info.id === 'opencode'
        ? {
            ...summary,
            detection: {
              ...summary.detection,
              installed: false,
              binaryPath: null,
              version: null,
              installHint: {
                commands: [
                  { label: 'OpenCode 1', command: 'npm install -g opencode-ai' },
                  { label: 'OpenCode 2', command: 'npm install -g @opencode/cli' },
                ],
                docsUrl: 'https://opencode.ai/docs/',
              },
            },
          }
        : summary,
    );
    await openStep('harnesses');
    await fireEvent.click(await row(/^OpenCode/));

    const detail = screen.getByRole('article', { name: 'OpenCode' });
    expect(within(detail).getByText('OpenCode 1')).toBeInTheDocument();
    expect(within(detail).getByText('npm install -g opencode-ai')).toBeInTheDocument();
    expect(within(detail).getByText('OpenCode 2')).toBeInTheDocument();
    expect(within(detail).getByText('npm install -g @opencode/cli')).toBeInTheDocument();
    expect(within(detail).getByRole('button', { name: 'Copy the OpenCode 1 install command' })).toBeInTheDocument();
    expect(within(detail).getByRole('button', { name: 'Copy the OpenCode 2 install command' })).toBeInTheDocument();
  });

  it('moves the daily driver chip in the list when another harness is chosen from its details', async () => {
    await openStep('harnesses');

    const opencode = await row(/^OpenCode/);
    expect(opencode).toHaveTextContent('Installed');
    expect(opencode).not.toHaveTextContent('Daily driver');
    await fireEvent.click(opencode);
    // Looking is not choosing.
    expect(onboarding.driver).toBe('omp');
    expect(screen.getByRole('radio', { name: 'Use OpenCode as daily driver' })).toHaveAttribute('aria-checked', 'false');

    await fireEvent.click(screen.getByRole('radio', { name: 'Use OpenCode as daily driver' }));

    await waitFor(() => expect(opencode).toHaveTextContent('Daily driver'));
    expect(screen.getByRole('option', { name: /^Oh My Pi/ })).toHaveTextContent('Installed');
    expect(screen.getByRole('radio', { name: 'Use OpenCode as daily driver' })).toBeChecked();
  });

  it('walks the list with the arrow keys, moving focus and the details together', async () => {
    await openStep('harnesses');

    const first = await row(/^Oh My Pi/);
    first.focus();

    await fireEvent.keyDown(first, { key: 'ArrowDown' });
    const second = screen.getByRole('option', { name: /^Pi/ });
    expect(second).toHaveFocus();
    expect(second).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('article', { name: 'Pi' })).toBeInTheDocument();
    expect(first).toHaveAttribute('tabindex', '-1');

    await fireEvent.keyDown(second, { key: 'ArrowUp' });
    await fireEvent.keyDown(first, { key: 'ArrowUp' });
    expect(screen.getByRole('option', { name: /^Codex/ })).toHaveFocus();
    expect(onboarding.driver).toBe('omp');
  });

  it('jumps to the first and last harness with Home and End', async () => {
    await openStep('harnesses');

    const first = await row(/^Oh My Pi/);
    first.focus();

    await fireEvent.keyDown(first, { key: 'End' });
    const last = screen.getByRole('option', { name: /^Codex/ });
    expect(last).toHaveFocus();
    expect(last).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('article', { name: 'Codex' })).toBeInTheDocument();

    await fireEvent.keyDown(last, { key: 'Home' });
    expect(first).toHaveFocus();
    expect(first).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('article', { name: 'Oh My Pi' })).toBeInTheDocument();
    expect(onboarding.driver).toBe('omp');
  });

  it('shows a harness the app cannot chat with as coming soon, without a binary it does not have', async () => {
    harness.summaries = harness.summaries.map((summary) =>
      summary.info.id === 'pi' ? { ...summary, capabilities: { ...summary.capabilities, chat: false } } : summary,
    );
    await openStep('harnesses');

    const soon = await row(/^Pi/);
    expect(soon).toHaveTextContent('Coming soon');
    await fireEvent.click(soon);

    const detail = screen.getByRole('article', { name: 'Pi' });
    expect(within(detail).getByText('Coming soon')).toBeInTheDocument();
    expect(within(detail).queryByText('Not found')).toBeNull();
    expect(within(detail).queryByText('Binary')).toBeNull();
    expect(within(detail).getByText('Config')).toBeInTheDocument();
    expect(within(detail).queryByRole('radio')).toBeNull();
  });

  it('keeps the harness being read when a scan turns up another', async () => {
    await openStep('harnesses');

    await fireEvent.click(await row(/^OpenCode/));
    harness.summaries = harness.summaries.map((summary) => (summary.info.id === 'pi' ? { ...summary, detection: { ...summary.detection, installed: true } } : summary));

    await waitFor(() => expect(screen.getByRole('option', { name: /^Pi/ })).toHaveTextContent('Installed'));
    expect(screen.getByRole('option', { name: /^OpenCode/ })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('article', { name: 'OpenCode' })).toBeInTheDocument();
  });
});
