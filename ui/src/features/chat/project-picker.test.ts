import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { chatSession } from '../../../tests/chat-helpers';
import { samplePrefs } from '../../../tests/helpers';
import ProjectPicker from './ProjectPicker.svelte';

const recents = fixtures.recentProjects.filter((entry) => entry.exists);

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', onboardingCompleted: true });
  await harness.load();
  await project.load();
});

async function openPicker() {
  const { session } = chatSession([{ type: 'ready', protocolVersion: 1 }], { project: recents[0].path });
  render(ProjectPicker, { session });
  await fireEvent.click(screen.getByRole('button', { name: /Switch project/ }));

  return screen.findByRole('dialog');
}

describe('project picker', () => {
  it('narrows the list as you search', async () => {
    const dialog = await openPicker();
    const all = within(dialog).getAllByRole('menuitemradio').length;
    expect(all).toBeGreaterThan(1);

    await fireEvent.input(within(dialog).getByRole('searchbox'), { target: { value: recents[1].name } });

    await waitFor(() => expect(within(dialog).getAllByRole('menuitemradio').length).toBeLessThan(all));
    expect(within(dialog).getByRole('menuitemradio', { name: new RegExp(recents[1].name) })).toBeInTheDocument();
  });

  it('removes the ticked projects from the list in edit mode, never the open one', async () => {
    const dialog = await openPicker();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Edit list' }));

    const open = within(dialog).getByRole('checkbox', { name: new RegExp(recents[0].name) });
    expect(open).toBeDisabled();

    const other = within(dialog).getByRole('checkbox', { name: new RegExp(recents[1].name) });
    await fireEvent.click(other);
    expect(other).toBeChecked();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Remove 1 from list' }));

    await waitFor(() => expect(project.recents.some((entry) => entry.path === recents[1].path)).toBe(false));
    expect(project.recents.some((entry) => entry.path === recents[0].path)).toBe(true);
    await waitFor(() => expect(within(dialog).queryByRole('checkbox')).toBeNull());
  });

  it('keeps the remove button off until something is ticked', async () => {
    const dialog = await openPicker();

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Edit list' }));

    expect(within(dialog).getByRole('button', { name: 'Remove 1 from list' })).toBeDisabled();
  });
});
