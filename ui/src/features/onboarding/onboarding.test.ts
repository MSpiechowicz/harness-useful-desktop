import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import { ipc } from '$lib/ipc/client';
import { harness } from '$lib/stores/harness.svelte';
import { HARNESS_SETUP_POLL_MS } from '$lib/stores/live';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { intro } from '../intro/intro-state.svelte';
import Onboarding from './Onboarding.svelte';
import { onboarding } from './onboarding-state.svelte';

// The component talks to the real stores, which talk to the in-memory mock backend.
beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');

  await prefs.load();
  // happy-dom has no real Web Animations; with motion off every Svelte transition is zero-length.
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
  intro.skip();
});

// The notices seen are remembered in the webview's storage: a test must not inherit the previous one's.
beforeEach(() => localStorage.clear());

const click = (name: string | RegExp) => fireEvent.click(screen.getByRole('button', { name }));

/** The set-up step offers to skip once it has read the machine and found something to install. */
async function skipSetup(): Promise<void> {
  await fireEvent.click(await screen.findByRole('button', { name: 'Skip for now' }));
}

async function next(heading: string | RegExp): Promise<void> {
  await waitFor(() => expect(screen.getByRole('heading', { name: heading })).toBeInTheDocument());
}

describe('Onboarding', () => {
  it('walks through setup for PI and skips the approval notice', async () => {
    render(Onboarding);

    await next(/find your harness/);
    await click('Get started');

    await next('Which agent do you use every day?');
    expect(await screen.findByRole('article', { name: 'Oh My Pi' })).toHaveTextContent('Supported');
    const piCard = screen.getByRole('article', { name: 'Pi' });
    expect(piCard).toHaveTextContent('Not found');
    expect(piCard).toHaveTextContent('curl -fsSL https://pi.dev/install.sh | sh');
    expect(screen.getByRole('radio', { name: 'Use Pi as daily driver' })).toHaveAttribute('aria-disabled', 'true');

    await click('Choose binary…');
    await waitFor(() => expect(screen.getByRole('article', { name: 'Pi' })).toHaveTextContent('Untested version'));

    await fireEvent.click(screen.getByRole('radio', { name: 'Use Pi as daily driver' }));
    await click('Continue');

    await next('Pick a project to work in');
    await click('Skip for now');

    await next('Meet the Harness family');
    await click('Continue');

    await next('Get more from your agents');
    await skipSetup();

    await next('You’re ready to go');
    expect(screen.queryByRole('heading', { name: /without asking/ })).toBeNull();

    await click('Open Harness Desktop');

    await waitFor(() => expect(prefs.value?.onboardingCompleted).toBe(true));
    expect(prefs.value?.defaultHarness).toBe('pi');
    expect(prefs.value?.approvalNoticeSeen).toBe(false);
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  describe('finding a harness without being asked', () => {
    afterEach(async () => {
      vi.useRealTimers();
      await ipc.invoke('harness_clear_binary', { harness: 'pi' });
      await harness.load();
    });

    it('shows a harness installed in a terminal while the step is open, and has no Re-scan button', async () => {
      await ipc.invoke('harness_clear_binary', { harness: 'pi' });
      await harness.load();
      vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
      onboarding.restart('harnesses');
      render(Onboarding);

      await next('Which agent do you use every day?');
      expect(await screen.findByRole('article', { name: 'Pi' })).toHaveTextContent('Not found');
      expect(screen.queryByRole('button', { name: /re-scan/i })).toBeNull();

      // Installed behind the app's back, as a terminal would do.
      await ipc.invoke('harness_pick_binary', { harness: 'pi' });
      await vi.advanceTimersByTimeAsync(HARNESS_SETUP_POLL_MS);

      await waitFor(() => expect(screen.getByRole('article', { name: 'Pi' })).toHaveTextContent('Untested version'));
    });
  });

  it('explains the OMP approval default once, when OMP is the daily driver', async () => {
    onboarding.restart('harnesses');
    render(Onboarding);

    await next('Which agent do you use every day?');
    await fireEvent.click(await screen.findByRole('radio', { name: 'Use Oh My Pi as daily driver' }));
    await click('Continue');
    await next('Pick a project to work in');
    await click('Skip for now');

    await next('OMP can run tools without asking');
    expect(screen.getByText(/never changes it behind your back/)).toBeInTheDocument();
    await click('Got it');

    await next('Meet the Harness family');
    await click('Continue');

    await next('Get more from your agents');
    await skipSetup();

    await next('You’re ready to go');
    await click('Open Harness Desktop');

    await waitFor(() => expect(prefs.value?.defaultHarness).toBe('omp'));
    expect(prefs.value?.approvalNoticeSeen).toBe(true);
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('lists OpenCode next to the others and says its chat is still to come', async () => {
    onboarding.restart('harnesses');
    render(Onboarding);
    await next('Which agent do you use every day?');

    // OpenCode is listed, greyed out, but not offered as a daily driver until the app can chat with it.
    expect(await screen.findAllByRole('radio')).toHaveLength(2);
    expect(screen.queryByRole('radio', { name: 'Use OpenCode as daily driver' })).not.toBeInTheDocument();
    const card = screen.getByRole('article', { name: 'OpenCode' });
    expect(card).toHaveTextContent('Installed');
    expect(card).toHaveTextContent('Coming soon');
    expect(screen.getByRole('article', { name: 'Oh My Pi' })).not.toHaveTextContent('Coming soon');
  });

  it('explains the OpenCode permission default once, when OpenCode is the daily driver', async () => {
    // OpenCode cannot be picked on the cards any more, but a default saved earlier still makes it the driver.
    await prefs.update({ approvalNoticeSeen: false, defaultHarness: 'opencode' });
    onboarding.restart('harnesses');
    render(Onboarding);

    await next('Which agent do you use every day?');
    await click('Continue');
    await next('Pick a project to work in');
    // The chat tile does not promise a chat the app cannot start yet.
    expect(screen.getByText('Chat with OpenCode is coming soon.')).toBeInTheDocument();
    await click('Skip for now');

    await next('OpenCode can run tools without asking');
    expect(screen.getByText('permission')).toBeInTheDocument();
    expect(screen.getByText(/never changes it behind your back/)).toBeInTheDocument();
    await click('Got it');

    await next('Meet the Harness family');
    await click('Continue');
    await next('Get more from your agents');
    await skipSetup();

    // A daily driver without chat gets a tip that is true for it: new chats open with the harness that can chat.
    await next('You’re ready to go');
    expect(screen.getByText('OpenCode cannot chat yet, so new chats open with Oh My Pi.')).toBeInTheDocument();
    await click('Open Harness Desktop');

    await waitFor(() => expect(prefs.value?.defaultHarness).toBe('opencode'));
    // The saved flag is the OMP notice's: this one is remembered on its own and OMP's is still to come.
    expect(prefs.value?.approvalNoticeSeen).toBe(false);
    await waitFor(() => expect(onboarding.approvalNotice).toBeNull());
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('finishes on a machine where OpenCode is the only harness, pointing to the sessions it can read', async () => {
    await prefs.update({ approvalNoticeSeen: false, defaultHarness: null, onboardingCompleted: false });
    harness.summaries = harness.summaries.map((summary) => ({ ...summary, detection: { ...summary.detection, installed: summary.info.id === 'opencode' } }));
    onboarding.restart('harnesses');
    render(Onboarding);

    // With nothing picked, the only installed harness is the daily driver and its notice is next.
    await next('Which agent do you use every day?');
    expect(onboarding.driver).toBe('opencode');
    await click('Continue');
    await next('Pick a project to work in');
    await click('Skip for now');
    await next('OpenCode can run tools without asking');
    await click('Got it');
    await next('Meet the Harness family');
    await click('Continue');
    await next('Get more from your agents');
    await skipSetup();

    await next('You’re ready to go');
    expect(screen.getByText('Read what you ran with OpenCode. Chat with it is coming soon.')).toBeInTheDocument();
    await click('Open Harness Desktop');

    await waitFor(() => expect(prefs.value?.onboardingCompleted).toBe(true));
    expect(prefs.value?.defaultHarness).toBe('opencode');
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    await harness.load();
  });

  it('is only closable when it was started again', async () => {
    render(Onboarding);
    expect(screen.queryByRole('dialog')).toBeNull();

    onboarding.restart();

    expect(await screen.findByRole('dialog')).toBeInTheDocument();
    await click('Close setup');

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('lets you continue with no harness installed', async () => {
    onboarding.restart('harnesses');
    render(Onboarding);
    await next('Which agent do you use every day?');

    // As if the user had no harness on this machine.
    harness.summaries = harness.summaries.map((summary) => ({ ...summary, detection: { ...summary.detection, installed: false } }));

    expect(await screen.findByRole('button', { name: 'Continue without a harness' })).toBeEnabled();
    expect(screen.getByText(/No harness found yet/)).toBeInTheDocument();
  });

  it('welcomes in Polish and keeps the language through the steps', async () => {
    await i18n.load('pl');
    onboarding.restart();
    render(Onboarding);

    await next(/Znajdźmy twój harness/);
    await click('Zaczynamy');

    await next('Którego agenta używasz na co dzień?');
    expect(await screen.findByRole('radiogroup', { name: 'Codzienny harness' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Szukaj ponownie' })).toBeNull();
    expect(screen.getByRole('radiogroup', { name: 'Codzienny harness' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Wstecz' })).toBeInTheDocument();
  });
});
