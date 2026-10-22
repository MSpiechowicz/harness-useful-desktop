import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { agentSessions } from '$lib/stores/agentSession.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { chatSession } from '../../../tests/chat-helpers';
import { samplePrefs, withoutChat } from '../../../tests/helpers';
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
  it('shows the harness by its full name once, beside its dot, not its short name again', () => {
    const { session } = chatSession([{ type: 'ready', protocolVersion: 1 }], { project: '/home/demo/app' });
    render(HarnessPicker, { session });
    const info = harness.get(session.info.harness)!.info;
    const trigger = screen.getByRole('button', { name: /^Harness: / });

    expect(trigger).toHaveTextContent(new RegExp(`^${info.name}$`));
    expect(trigger.querySelector('.live-dot')).not.toBeNull();
  });

  it('names the chat\'s harness and lists every harness, the missing ones greyed out', async () => {
    const { session, menu } = await openPicker();
    const rows = within(menu).getAllByRole('menuitemradio');

    expect(rows).toHaveLength(harness.summaries.length);
    expect(within(menu).getByRole('menuitemradio', { checked: true })).toHaveTextContent(harness.get(session.info.harness)!.info.name);
    for (const summary of harness.summaries.filter((item) => !item.detection.installed)) {
      expect(within(menu).getByRole('menuitemradio', { name: new RegExp(`^${summary.info.name}\\b`) })).toHaveAttribute('aria-disabled', 'true');
    }
  });

  it('hands the chat to the other harness in the same tab', async () => {
    await harness.pickBinary('pi');
    const { session, menu } = await openPicker();
    agentSessions.sessions = [session];
    agentSessions.activeKey = session.key;
    const change = vi.spyOn(agentSessions, 'changeHarness').mockResolvedValue(null);

    await fireEvent.click(within(menu).getByRole('menuitemradio', { name: /^Pi\b/ }));

    await waitFor(() => expect(change).toHaveBeenCalledWith(session.key, 'pi'));
    change.mockRestore();
    agentSessions.sessions = [];
    agentSessions.activeKey = null;
  });

  it('shows the harness the chat is moving to, with a spinner, until it is ready', async () => {
    const { session } = chatSession([{ type: 'ready', protocolVersion: 1 }], { project: '/home/demo/app' });
    session.switchingTo = 'opencode';
    render(HarnessPicker, { session });

    const button = screen.getByRole('button', { name: /^Harness: / });
    expect(button).toHaveAccessibleName(/^Harness: OpenCode\./);
    expect(within(button).getByRole('status')).toBeInTheDocument();
  });

  it('shows a harness without chat as coming soon and never starts a chat with it', async () => {
    withoutChat(harness, 'opencode');
    const { session, menu } = await openPicker();
    agentSessions.sessions = [session];
    agentSessions.activeKey = session.key;
    const create = vi.spyOn(agentSessions, 'create').mockResolvedValue(null);
    const change = vi.spyOn(agentSessions, 'changeHarness').mockResolvedValue(null);
    const row = within(menu).getByRole('menuitemradio', { name: /^OpenCode\b/ });

    // It is installed, so the hint is about chat, not about installing.
    expect(harness.get('opencode')?.detection.installed).toBe(true);
    expect(row).toHaveAttribute('aria-disabled', 'true');
    expect(row).toHaveTextContent('Chat coming soon');
    expect(within(menu).getByRole('menuitemradio', { name: /^Oh My Pi\b/ })).not.toHaveTextContent('Chat coming soon');

    await fireEvent.click(row);

    expect(create).not.toHaveBeenCalled();
    expect(change).not.toHaveBeenCalled();
    create.mockRestore();
    change.mockRestore();
    agentSessions.sessions = [];
    agentSessions.activeKey = null;
  });
});
