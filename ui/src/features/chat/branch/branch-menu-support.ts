import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, expect, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { defaultScenario } from '$lib/ipc/mock/scenarios';
import { createMockTransport } from '$lib/ipc/mock/transport';
import { branchSync } from '$lib/stores/branch-sync.svelte';
import { git } from '$lib/stores/git.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { startedSession } from '../../../../tests/chat-helpers';
import { samplePrefs } from '../../../../tests/helpers';
import BranchChip from './BranchChip.svelte';

export const PROJECT = '/home/demo/Projects/demo';

/** Registers the hooks every branch menu test needs: the mock backend, the prefs, a clean store after each test. */
export function useBranchMenu(): void {
  beforeAll(async () => {
    vi.stubEnv('VITE_TRANSPORT', 'mock');
    prefs.value = samplePrefs({ motion: 'off', onboardingCompleted: true });
    await harness.load();
    await project.load();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    history.replaceState(null, '', '/');
    branchSync.follow(null, null);
    git.activate(null);
    nav.go('chat');
    toasts.items = [];
  });
}

/** A fresh in-memory backend, opened the way a preview is (`?sync=conflict`), with the chip of a chat in the demo project. */
export async function chip(search = '') {
  history.replaceState(null, '', `/${search}`);
  const backend = createMockTransport(defaultScenario(), 0);
  vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: unknown) => backend.invoke(name as never, args as never)) as typeof ipc.invoke);

  git.activate(PROJECT);
  await waitFor(() => expect(git.status).not.toBeNull());
  branchSync.follow(PROJECT, null);
  await waitFor(() => expect(branchSync.state).not.toBeNull());

  const chat = await startedSession();
  render(BranchChip, { session: chat.session });

  return chat;
}

/** The note of the menu that contains `text`: its title is split around branch names, so the text query cannot find it. */
export async function note(dialog: HTMLElement, text: string) {
  return waitFor(() => {
    const found = within(dialog)
      .getAllByRole('status')
      .find((element) => element.textContent?.includes(text));

    expect(found).toBeDefined();

    return found as HTMLElement;
  });
}

export async function openMenu() {
  await fireEvent.click(screen.getByRole('button', { name: /^Branch / }));

  return screen.findByRole('dialog', { name: 'Branch' });
}
