import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { agentSessions } from '$lib/stores/agentSession.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { chatSession } from '../../../tests/chat-helpers';
import { samplePrefs } from '../../../tests/helpers';
import HarnessPicker from './HarnessPicker.svelte';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', onboardingCompleted: true });
  await harness.load();
});

async function openPicker() {
  const { session } = chatSession([{ type: 'ready', protocolVersion: 1 }], { project: '/home/demo/app' });
  render(HarnessPicker, { session });
  await fireEvent.click(screen.getByRole('button', { name: /^Harness: / }));

  return { session, menu: await screen.findByRole('menu') };
}

describe('harness picker', () => {
  it('names the chat\'s harness and lists every harness, the missing ones greyed out', async () => {
    const { session, menu } = await openPicker();
    const rows = within(menu).getAllByRole('menuitemradio');

    expect(rows).toHaveLength(harness.summaries.length);
    expect(within(menu).getByRole('menuitemradio', { checked: true })).toHaveTextContent(harness.get(session.info.harness)!.info.name);
    for (const summary of harness.summaries.filter((item) => !item.detection.installed)) {
      expect(within(menu).getByRole('menuitemradio', { name: new RegExp(`^${summary.info.name}\\b`) })).toHaveAttribute('aria-disabled', 'true');
    }
  });

  it('opens a chat with the other harness in the same project', async () => {
    const other = harness.installed.find((item) => item.info.id !== 'omp') ?? harness.installed[0];
    const { menu } = await openPicker();
    const create = vi.spyOn(agentSessions, 'create').mockResolvedValue(null);

    await fireEvent.click(within(menu).getByRole('menuitemradio', { name: new RegExp(`^${other.info.name}\\b`) }));

    await waitFor(() => {
      if (other.info.id !== 'omp') {
        expect(create).toHaveBeenCalledWith(other.info.id, '/home/demo/app');
      }
    });
    create.mockRestore();
  });
});
