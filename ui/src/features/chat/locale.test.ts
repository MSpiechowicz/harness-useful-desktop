import { render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { MessageMeta } from '$lib/chat/types';

import { simulatePolishOs } from '../../../tests/locale';
import { nodeTooltip } from '../sessions/session-format';
import MessageMetaLine from './MessageMeta.svelte';

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('numbers and dates on a system that is not English', () => {
  it('a message reports its tokens with English grouping', () => {
    simulatePolishOs();
    const meta: MessageMeta = {
      entryId: null,
      model: 'claude',
      provider: null,
      stopReason: null,
      errorMessage: null,
      usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 17_612 },
      costUsd: null,
      timestamp: null,
    };

    render(MessageMetaLine, { meta });

    expect(screen.getByText('claude · 17,612 tokens')).toBeInTheDocument();
  });

  it('a session tree row shows its time in English', () => {
    simulatePolishOs();

    const tooltip = nodeTooltip({ kind: 'user', preview: 'hi', timestamp: new Date(2026, 9, 8, 18, 42).toISOString() } as Parameters<typeof nodeTooltip>[0]);

    expect(tooltip).toContain('Oct 8, 2026, 6:42 PM');
  });
});
