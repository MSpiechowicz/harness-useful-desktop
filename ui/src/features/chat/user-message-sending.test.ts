import { render, screen } from '@testing-library/svelte';
import { beforeAll, describe, expect, it } from 'vitest';

import type { MessageItem } from '$lib/chat/types';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import UserMessage from './UserMessage.svelte';

beforeAll(() => {
  prefs.value = samplePrefs({ motion: 'off' });
});

function item(status: 'sending' | 'sent', timestamp: number | null): MessageItem {
  const meta = timestamp === null ? null : { entryId: null, model: null, provider: null, stopReason: null, errorMessage: null, usage: null, costUsd: null, timestamp };

  return { kind: 'message', key: 'p:1', order: 0, role: 'user', blocks: [{ type: 'text', text: 'Fix it' }], streaming: false, slots: {}, meta, send: { id: '1', status, words: 'Fix it' } };
}

describe('a message of the user that waits for its echo', () => {
  it('says it is sending in the footer, keeps the footer actions in place and cannot be used again yet', () => {
    render(UserMessage, { item: item('sending', null), onUseAgain: () => {} });

    expect(screen.getByText('Sending…')).toBeTruthy();
    expect(screen.getByRole('article').getAttribute('aria-busy')).toBe('true');
    expect(screen.getByRole('button', { name: 'Copy prompt' })).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Use again' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('shows the time instead once the echo has taken its place', () => {
    render(UserMessage, { item: item('sent', Date.now()), onUseAgain: () => {} });

    expect(screen.queryByText('Sending…')).toBeNull();
    expect(screen.getByRole('article').getAttribute('aria-busy')).toBeNull();
    expect((screen.getByRole('button', { name: 'Use again' }) as HTMLButtonElement).disabled).toBe(false);
  });
});
