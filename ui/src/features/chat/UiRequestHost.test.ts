import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { UiRequest } from '$lib/ipc/bindings/UiRequest';

import { startedSession } from '../../../tests/chat-helpers';
import UiRequestHost from './UiRequestHost.svelte';

const question = (id: string, title: string): UiRequest => ({
  id,
  kind: { type: 'select', title, options: ['One', 'Two'], details: [null, null], timeoutMs: null },
  purpose: 'question',
});

async function open(request: UiRequest) {
  const backend = await startedSession();
  let seq = 0;
  const send = (next: UiRequest): void => {
    seq += 1;
    backend.session.ingest({ sessionKey: backend.session.key, seq, event: { type: 'uiRequest', request: next } });
    backend.session.flush();
  };

  render(UiRequestHost, { session: backend.session });
  send(request);

  return { ...backend, send };
}

describe('a dialog of several questions', () => {
  afterEach(() => {
    vi.useRealTimers();
    cleanup();
  });

  it('keeps one dialog open from a question to the next and shows the step apart from the title', async () => {
    const { send } = await open(question('q1', 'Which area? (1/2)'));
    const dialog = await screen.findByRole('dialog', { name: 'Which area?' });

    expect(screen.getByText('Question 1 of 2')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: /One/ }));
    expect(screen.getByRole('dialog')).toBe(dialog);

    send(question('q2', 'How far? (2/2)'));

    await waitFor(() => expect(screen.getByRole('dialog', { name: 'How far?' })).toBe(dialog));
    expect(screen.getByText('Question 2 of 2')).toBeInTheDocument();
  });

  it('answers a held question only once', async () => {
    const backend = await open(question('q1', 'Which area? (1/2)'));

    await fireEvent.click(await screen.findByRole('button', { name: /One/ }));
    await fireEvent.click(screen.getByRole('button', { name: /Two/ }));

    await waitFor(() => expect(backend.of('agent_ui_respond')).toHaveLength(1));
  });

  it('closes when the next question never comes', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    await open(question('q1', 'Which area? (1/2)'));

    await fireEvent.click(await screen.findByRole('button', { name: /One/ }));
    await vi.advanceTimersByTimeAsync(2100);

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('closes after the last question and after a cancel', async () => {
    await open(question('q2', 'How far? (2/2)'));

    await fireEvent.click(await screen.findByRole('button', { name: /One/ }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());

    cleanup();
    await open(question('q1', 'Which area? (1/2)'));
    await fireEvent.click(await screen.findByRole('button', { name: 'Cancel' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });
});
