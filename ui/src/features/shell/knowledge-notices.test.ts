import { render } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { KnowledgePulse } from '$lib/ipc/bindings/KnowledgePulse';
import { ipc } from '$lib/ipc/client';
import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import KnowledgeNotices from './KnowledgeNotices.svelte';

const PULSE: KnowledgePulse = { documents: 3, generation: 1, held: 0, offline: false };

beforeEach(() => {
  toasts.items = [];
  knowledgeChanges.pulse = PULSE;
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('the notice to restart the harnesses', () => {
  it('stays quiet while the database was not upgraded', () => {
    render(KnowledgeNotices);

    expect(toasts.items).toEqual([]);
  });

  it('asks once, and stays until the user answers, in a grey toast with one action', async () => {
    render(KnowledgeNotices);
    knowledgeChanges.pulse = { ...PULSE, restartHarnesses: true };
    await vi.waitFor(() => expect(toasts.items).toHaveLength(1));

    knowledgeChanges.pulse = { ...PULSE, restartHarnesses: true, generation: 2 };
    await Promise.resolve();

    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0]).toMatchObject({
      title: 'Harness Knowledge was updated',
      description: 'Restart your harnesses so their agents use the new version.',
      tone: 'neutral',
      duration: 0,
      action: { label: 'Got it' },
    });
  });

  it('tells the backend when the toast is closed, so it does not come back', async () => {
    const invoke = vi.spyOn(ipc, 'invoke').mockResolvedValue(null as never);
    render(KnowledgeNotices);
    knowledgeChanges.pulse = { ...PULSE, restartHarnesses: true };
    await vi.waitFor(() => expect(toasts.items).toHaveLength(1));

    toasts.dismiss(toasts.items[0].id);

    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('knowledge_restart_notice_dismiss', undefined));
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('takes the toast away without asking again when the notice ended elsewhere', async () => {
    const invoke = vi.spyOn(ipc, 'invoke').mockResolvedValue(null as never);
    render(KnowledgeNotices);
    knowledgeChanges.pulse = { ...PULSE, restartHarnesses: true };
    await vi.waitFor(() => expect(toasts.items).toHaveLength(1));

    knowledgeChanges.pulse = { ...PULSE, restartHarnesses: null };

    await vi.waitFor(() => expect(toasts.items).toEqual([]));
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe('the notice that a teammate has to update the database', () => {
  it('says so once while it is true, and again after it was put right and came back', async () => {
    render(KnowledgeNotices);
    knowledgeChanges.pulse = { ...PULSE, upgradeWaits: true };
    await vi.waitFor(() => expect(toasts.items).toHaveLength(1));
    expect(toasts.items[0]).toMatchObject({ description: 'Waiting for a teammate to update Remote knowledge.', tone: 'warning' });

    knowledgeChanges.pulse = { ...PULSE, upgradeWaits: true, generation: 2 };
    await Promise.resolve();
    expect(toasts.items).toHaveLength(1);

    knowledgeChanges.pulse = { ...PULSE, upgradeWaits: false };
    await Promise.resolve();
    knowledgeChanges.pulse = { ...PULSE, upgradeWaits: true };

    await vi.waitFor(() => expect(toasts.items).toHaveLength(2));
  });
});
