import { describe, expect, it } from 'vitest';

import type { AgentEnvelope } from '$lib/ipc/bindings/AgentEnvelope';
import type { AgentEvent } from '$lib/ipc/bindings/AgentEvent';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { sampleInfo } from '../../../tests/chat-helpers';
import { HarnessModels } from './harness-models.svelte';

const SCOPE = { harness: 'omp', project: '/p' } as const;
const MODEL = { provider: 'openai', id: 'gpt-5' };

/** A harness whose helper answers the asks about models and levels, or whose start fails while `down` is set. */
function harness() {
  const actions: string[] = [];
  const state = { down: false };

  const transport: Transport = {
    async invoke(name, args) {
      if (name === 'agent_start') {
        if (state.down) {
          throw IpcFailure.from({ code: 'untrusted', message: 'Not trusted.' });
        }

        const onEvent = (args as { onEvent: (envelope: AgentEnvelope) => void }).onEvent;
        queueMicrotask(() => onEvent({ sessionKey: 'helper', seq: 1, event: { type: 'ready', protocolVersion: 1 } as AgentEvent }));

        return sampleInfo({ key: 'helper' }) as never;
      }

      if (name === 'agent_send') {
        const action = (args as { action: { type: string } }).action;
        actions.push(action.type);

        if (action.type === 'getAvailableModels') {
          return { type: 'models', models: [{ ...MODEL, name: 'GPT-5', reasoning: true, contextWindow: null, maxTokens: null, input: [] }] } as never;
        }

        if (action.type === 'getAvailableThinkingLevels') {
          return { type: 'thinkingLevels', levels: ['low', 'high'] } as never;
        }
      }

      return { type: 'ack' } as never;
    },
  };

  return { catalog: new HarnessModels(transport), actions, state };
}

describe('HarnessModels', () => {
  it('asks a helper for the models once per harness and folder, and for the levels once per model after switching to it', async () => {
    const { catalog, actions } = harness();

    expect(catalog.modelsOf(SCOPE)).toBeNull();
    await Promise.all([catalog.loadModels(SCOPE), catalog.loadModels(SCOPE)]);
    await catalog.loadModels(SCOPE);
    expect(catalog.modelsOf(SCOPE)?.map((model) => model.id)).toEqual(['gpt-5']);
    expect(actions).toEqual(['getAvailableModels']);

    await catalog.loadLevels(SCOPE, MODEL);
    await catalog.loadLevels(SCOPE, MODEL);
    expect(catalog.levelsOf(SCOPE, MODEL)).toEqual(['low', 'high']);
    expect(actions).toEqual(['getAvailableModels', 'setModel', 'getAvailableThinkingLevels']);

    // Another folder is another ask.
    expect(catalog.modelsOf({ ...SCOPE, project: '/other' })).toBeNull();
  });

  it('keeps nothing of an ask that failed, and asks again at the next look', async () => {
    const { catalog, state } = harness();
    state.down = true;

    await catalog.loadModels(SCOPE);
    expect(catalog.modelsOf(SCOPE)).toBeNull();
    expect(catalog.failed(SCOPE)).toBe(true);

    state.down = false;
    await catalog.loadModels(SCOPE);

    expect(catalog.failed(SCOPE)).toBe(false);
    expect(catalog.modelsOf(SCOPE)).toHaveLength(1);
  });
});
