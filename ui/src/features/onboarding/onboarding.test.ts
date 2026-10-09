import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { intro } from '../intro/intro.svelte';
import Onboarding from './Onboarding.svelte';
import { onboarding } from './onboarding.svelte';

// The component talks to the real stores, which talk to the in-memory mock backend.
beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');

  await prefs.load();
  // happy-dom has no real Web Animations; with motion off every Svelte transition is zero-length.
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
  intro.skip();
});

const click = (name: string | RegExp) => fireEvent.click(screen.getByRole('button', { name }));

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
    expect(piCard).toHaveTextContent('npm install -g @earendil-works/pi-coding-agent');
    expect(screen.getByRole('radio', { name: 'Use Pi as daily driver' })).toHaveAttribute('aria-disabled', 'true');

    await click('Choose binary…');
    await waitFor(() => expect(screen.getByRole('article', { name: 'Pi' })).toHaveTextContent('Untested version'));

    await fireEvent.click(screen.getByRole('radio', { name: 'Use Pi as daily driver' }));
    await click('Continue');

    await next('Pick a project to work in');
    await click('Skip for now');

    await next('You’re ready to go');
    expect(screen.queryByRole('heading', { name: /without asking/ })).toBeNull();

    await click('Open Harness Desktop');

    await waitFor(() => expect(prefs.value?.onboardingCompleted).toBe(true));
    expect(prefs.value?.defaultHarness).toBe('pi');
    expect(prefs.value?.approvalNoticeSeen).toBe(false);
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
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

    await next('You’re ready to go');
    await click('Open Harness Desktop');

    await waitFor(() => expect(prefs.value?.defaultHarness).toBe('omp'));
    expect(prefs.value?.approvalNoticeSeen).toBe(true);
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
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
});
