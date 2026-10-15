import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';

import { i18n } from '$lib/i18n/i18n.svelte';
import { initialChatState } from '$lib/chat/state';
import type { MessageMeta } from '$lib/chat/types';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { harness } from '$lib/stores/harness.svelte';

import { chatSession } from '../../../tests/chat-helpers';
import { chatStatus } from './chat-status';
import ChatControls from './ChatControls.svelte';
import MessageMetaLine from './MessageMeta.svelte';
import ToolResult from './ToolResult.svelte';

beforeEach(() => {
  harness.summaries = fixtures.harnesses;
});

function lines(count: number): string {
  return Array.from({ length: count }, (_, index) => `line ${index + 1}`).join('\n');
}

describe('chat in Polish', () => {
  it('translates the approval badge and its tooltip words', async () => {
    await i18n.load('pl');
    const { session } = chatSession([], { configuredApprovalMode: 'yolo', approvalOverride: null });

    render(ChatControls, { session });

    expect(screen.getByText('Zatwierdzanie: yolo')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Tryb zatwierdzania: yolo. Zmień' })).toBeInTheDocument();
  });

  it('picks the Polish plural form for a count of lines', async () => {
    await i18n.load('pl');

    const { unmount } = render(ToolResult, { text: lines(32), running: false, failed: false });
    expect(screen.getByRole('button', { name: 'Pokaż całość (32 linie)' })).toBeInTheDocument();
    unmount();

    render(ToolResult, { text: lines(25), running: false, failed: false });
    const button = screen.getByRole('button', { name: 'Pokaż całość (25 linii)' });
    await fireEvent.click(button);
    expect(screen.getByRole('button', { name: 'Pokaż mniej' })).toBeInTheDocument();
  });

  it('counts tokens with the Polish plural and grouping', async () => {
    await i18n.load('pl');
    const meta: MessageMeta = {
      entryId: null,
      model: 'claude',
      provider: null,
      stopReason: null,
      errorMessage: null,
      usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 22 },
      costUsd: null,
      timestamp: null,
    };

    render(MessageMetaLine, { meta });

    expect(screen.getByText('claude · 22 tokeny')).toBeInTheDocument();
  });

  it('words the agent status, with its queue, in Polish', async () => {
    await i18n.load('pl');
    const chat = { ...initialChatState('k'), phase: 'idle' as const, queue: { steering: ['a', 'b'], followUp: ['c'], liveSteered: 0 } };

    expect(chatStatus(chat).label).toBe('Bezczynny · 3 w kolejce');
  });
});
