import { cleanup, render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { AgentEvent } from '$lib/ipc/bindings/AgentEvent';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { SessionState } from '$lib/ipc/bindings/SessionState';
import { fixtures } from '$lib/ipc/mock/fixtures';

import { chatSession } from '../../../tests/chat-helpers';
import ThinkingPicker from './ThinkingPicker.svelte';

const READY: AgentEvent = { type: 'ready', protocolVersion: null };

function stateEvent(patch: Partial<SessionState>): AgentEvent {
  return { type: 'state', state: { ...structuredClone(fixtures.agent.state.state), ...patch } };
}

function mount(events: AgentEvent[], harness: HarnessId = 'claude') {
  const { session } = chatSession(events, { harness });
  render(ThinkingPicker, { session });
}

function picker(): HTMLElement | null {
  return screen.queryByRole('button', { name: /^Thinking: / });
}

describe('ThinkingPicker', () => {
  it('stays hidden while the session has no model', () => {
    mount([READY, stateEvent({ model: null, thinkingLevel: null })]);

    expect(picker()).toBeNull();
  });

  it('stays hidden before the session reported its state', () => {
    mount([READY]);

    expect(picker()).toBeNull();
  });

  it('shows the default level for a known model whose level is unknown', () => {
    mount([READY, stateEvent({ thinkingLevel: null })]);

    expect(picker()?.textContent).toContain('default');
  });

  it('shows the level of a known model in every harness', () => {
    for (const harness of ['omp', 'pi', 'opencode', 'claude'] as const) {
      mount([READY, stateEvent({ thinkingLevel: 'high' })], harness);

      expect(picker()?.textContent).toContain('high');
      cleanup();
    }
  });

  it('stays hidden for a model without reasoning', () => {
    const model = { ...structuredClone(fixtures.agent.state.state.model!), reasoning: false };
    mount([READY, stateEvent({ model })]);

    expect(picker()).toBeNull();
  });
});
