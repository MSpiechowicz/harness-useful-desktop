import { describe, expect, it } from 'vitest';

import type { AgentEnvelope } from '$lib/ipc/bindings/AgentEnvelope';
import type { AgentEvent } from '$lib/ipc/bindings/AgentEvent';
import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { sampleInfo } from '../../../tests/chat-helpers';
import { requestAiLabel } from './ai-name';

const reply = (text: string): ChatMessage => ({ id: null, role: 'assistant', blocks: [{ type: 'text', text }], model: null, provider: null, stopReason: null, timestamp: null, toolCallId: null, toolName: null, isError: null }) as unknown as ChatMessage;

/** A helper agent that answers the naming prompt with `text`, recording what it was asked. */
function helper(text: string | null) {
  const calls: { name: string; args: unknown }[] = [];
  let onEvent: (envelope: AgentEnvelope) => void = () => {};
  let seq = 0;
  const emit = (event: AgentEvent): void => onEvent({ sessionKey: 'helper', seq: (seq += 1), event });

  const transport: Transport = {
    async invoke(name, args) {
      calls.push({ name, args });

      if (name === 'agent_start') {
        onEvent = (args as { onEvent: typeof onEvent }).onEvent;
        queueMicrotask(() => emit({ type: 'ready', protocolVersion: 1 } as AgentEvent));

        return sampleInfo({ key: 'helper' }) as never;
      }

      if (name === 'agent_send' && (args as { action: { type: string } }).action.type === 'prompt' && text !== null) {
        queueMicrotask(() => {
          emit({ type: 'messageEnd', messageId: 'a1', message: reply(text) } as AgentEvent);
          emit({ type: 'settled' } as AgentEvent);
        });
      }

      return { type: 'ack' } as never;
    },
  };

  return { transport, calls };
}

const REQUEST = { harness: 'omp', project: '/p', model: { provider: 'anthropic', id: 'sonnet' }, prompt: 'Fix the flaky login test' } as const;

describe('requestAiLabel', () => {
  it('asks one tool-less helper on the chat\'s model and stops it', async () => {
    const { transport, calls } = helper('{"name": "Login test fix", "description": "Debug the flaky login test."}');

    expect(await requestAiLabel(transport, REQUEST)).toEqual({ name: 'Login test fix', description: 'Debug the flaky login test.' });

    const start = calls.find((call) => call.name === 'agent_start')!.args as { request: Record<string, unknown> };
    expect(start.request).toMatchObject({ utility: true, resume: null });
    expect(calls.filter((call) => call.name === 'agent_send').map((call) => (call.args as { action: { type: string } }).action.type)).toEqual(['setModel', 'prompt']);
    expect(calls.at(-1)).toMatchObject({ name: 'agent_stop', args: { key: 'helper' } });
  });

  it('gives null, and still stops the helper, when the reply is not a label', async () => {
    const { transport, calls } = helper('I cannot do that.');

    expect(await requestAiLabel(transport, REQUEST)).toBeNull();
    expect(calls.at(-1)?.name).toBe('agent_stop');
  });

  it('skips naming quietly in a folder the harness does not trust: no prompt, no retry, nothing to show', async () => {
    const calls: string[] = [];
    const transport: Transport = {
      invoke: (name) => {
        calls.push(name);

        return Promise.reject(new IpcFailure({ code: 'untrusted', message: 'Claude Code does not trust this folder yet.' }));
      },
    };

    expect(await requestAiLabel(transport, { ...REQUEST, harness: 'claude' })).toBeNull();
    expect(calls).toEqual(['agent_start']);
  });

  it('gives null when the helper cannot start', async () => {
    const transport: Transport = { invoke: () => Promise.reject(new Error('no agent')) };

    expect(await requestAiLabel(transport, REQUEST)).toBeNull();
  });
});
