import { describe, expect, it } from 'vitest';

import type { AgentEnvelope } from '../bindings/AgentEnvelope';
import type { AgentEvent } from '../bindings/AgentEvent';
import type { AgentStartRequest } from '../bindings/AgentStartRequest';
import { fixtures } from './fixtures';
import { defaultScenario } from './scenarios';
import { createMockTransport } from './transport';

const REQUEST: AgentStartRequest = { harness: 'omp', project: fixtures.recentProjects[0].path, resume: null, approvalOverride: null, trustOnce: false };

async function start(request: AgentStartRequest = REQUEST) {
  const transport = createMockTransport(defaultScenario(), 0);
  const events: AgentEnvelope[] = [];
  const info = await transport.invoke('agent_start', { request, onEvent: (envelope) => events.push(envelope) });

  return { transport, info, events, types: () => events.map((envelope) => envelope.event.type) };
}

const prompt = (message: string) => ({ type: 'prompt', message, streamingBehavior: null }) as const;

describe('mock agent', () => {
  it('announces itself right after start, with a fresh key and ordered sequence numbers', async () => {
    const { info, events, types } = await start();

    await expect.poll(types).toEqual(['ready', 'state']);
    expect(events.every((envelope) => envelope.sessionKey === info.key)).toBe(true);
    expect(events.map((envelope) => envelope.seq)).toEqual([1, 2]);
  });

  it('replays the basic scenario for an ordinary prompt and shows the typed text as the user message', async () => {
    const { transport, info, events, types } = await start();

    await transport.invoke('agent_send', { key: info.key, action: prompt('hello there') });
    await expect.poll(() => types().at(-1)).toBe('settled');

    const user = events.map((envelope) => envelope.event).find((event) => event.type === 'messageEnd' && event.message.role === 'user');
    expect(user?.type === 'messageEnd' && user.message.blocks).toEqual([{ type: 'text', text: 'hello there' }]);
    expect(types()).toContain('toolEnd');
  });

  it('picks a scenario from keywords in the prompt', async () => {
    const error = await start();
    await error.transport.invoke('agent_send', { key: error.info.key, action: prompt('trigger an error') });
    await expect.poll(() => error.types().at(-1)).toBe('settled');

    const crash = await start();
    await crash.transport.invoke('agent_send', { key: crash.info.key, action: prompt('please crash') });
    await expect.poll(() => crash.types().at(-1)).toBe('exited');

    expect(error.events.some((envelope) => envelope.event.type === 'promptResult' && envelope.event.status === 'error')).toBe(true);
  });

  it('keeps ids unique across turns of one session', async () => {
    const { transport, info, events, types } = await start();

    await transport.invoke('agent_send', { key: info.key, action: prompt('one') });
    await expect.poll(() => types().filter((type) => type === 'settled').length).toBe(1);
    await transport.invoke('agent_send', { key: info.key, action: prompt('two') });
    await expect.poll(() => types().filter((type) => type === 'settled').length).toBe(2);

    const starts = events.flatMap((envelope) => (envelope.event.type === 'messageStart' ? [envelope.event.messageId] : []));
    expect(new Set(starts).size).toBe(starts.length);
  });

  it('pauses at an approval request until the webview answers, then continues', async () => {
    const { transport, info, events, types } = await start();

    await transport.invoke('agent_send', { key: info.key, action: prompt('needs approval') });
    await expect.poll(() => types().includes('uiRequest')).toBe(true);
    await new Promise((resolve) => setTimeout(resolve, 30));

    const request = events.find((envelope) => envelope.event.type === 'uiRequest')?.event as Extract<AgentEvent, { type: 'uiRequest' }>;
    expect(types().at(-1)).toBe('uiRequest');

    await transport.invoke('agent_ui_respond', { key: info.key, response: { id: request.request.id, outcome: { type: 'value', value: 'Approve' } } });
    await expect.poll(() => types().at(-1)).toBe('settled');

    expect(events.some((envelope) => envelope.event.type === 'toolEnd' && !envelope.event.isError)).toBe(true);
  });

  it('switches to the denied recording when the request is denied', async () => {
    const { transport, info, events, types } = await start();

    await transport.invoke('agent_send', { key: info.key, action: prompt('needs approval') });
    await expect.poll(() => types().includes('uiRequest')).toBe(true);

    const request = events.find((envelope) => envelope.event.type === 'uiRequest')?.event as Extract<AgentEvent, { type: 'uiRequest' }>;
    await transport.invoke('agent_ui_respond', { key: info.key, response: { id: request.request.id, outcome: { type: 'value', value: 'Deny' } } });
    await expect.poll(() => types().at(-1)).toBe('settled');

    expect(events.some((envelope) => envelope.event.type === 'toolEnd' && envelope.event.isError)).toBe(true);
    expect(events.some((envelope) => envelope.event.type === 'promptResult' && envelope.event.status === 'aborted')).toBe(true);
  });

  it('stops a turn on abort and reports it', async () => {
    const { transport, info, events, types } = await start();

    await transport.invoke('agent_send', { key: info.key, action: prompt('needs approval') });
    await expect.poll(() => types().includes('uiRequest')).toBe(true);
    await transport.invoke('agent_send', { key: info.key, action: { type: 'abort' } });

    await expect.poll(() => types().at(-1)).toBe('settled');
    expect(events.some((envelope) => envelope.event.type === 'promptResult' && envelope.event.status === 'aborted')).toBe(true);
  });

  it('refuses a plain prompt while busy but queues steering and follow-ups', async () => {
    const { transport, info, events, types } = await start();

    await transport.invoke('agent_send', { key: info.key, action: prompt('needs approval') });
    await expect.poll(() => types().includes('uiRequest')).toBe(true);

    await expect(transport.invoke('agent_send', { key: info.key, action: prompt('too soon') })).rejects.toMatchObject({ code: 'invalidInput' });
    await transport.invoke('agent_send', { key: info.key, action: { type: 'followUp', message: 'later' } });

    const queue = events.map((envelope) => envelope.event).filter((event) => event.type === 'queueUpdate');
    expect(queue.at(-1)).toMatchObject({ followUp: ['later'] });
  });

  it('refuses to run a command the user has not confirmed', async () => {
    const { transport, info } = await start();

    await expect(transport.invoke('agent_bash', { key: info.key, command: 'ls', confirmed: false })).rejects.toMatchObject({ code: 'forbidden' });
    await expect(transport.invoke('agent_bash', { key: info.key, command: 'ls', confirmed: true })).resolves.toMatchObject({ exitCode: 0 });
  });

  it('applies the launch options a harness supports', async () => {
    const omp = await start({ ...REQUEST, approvalOverride: 'alwaysAsk', trustOnce: true });
    const pi = await start({ ...REQUEST, harness: 'pi', approvalOverride: 'alwaysAsk', trustOnce: true });

    expect(omp.info).toMatchObject({ approvalOverride: 'alwaysAsk', trustedOnce: false, harness: 'omp' });
    expect(pi.info).toMatchObject({ approvalOverride: null, trustedOnce: true, harness: 'pi' });
  });

  it('lists running sessions, forgets stopped ones and refuses unknown keys', async () => {
    const { transport, info } = await start();

    expect(await transport.invoke('agent_list', undefined)).toHaveLength(1);
    await transport.invoke('agent_stop', { key: info.key });
    expect(await transport.invoke('agent_list', undefined)).toHaveLength(0);
    await expect(transport.invoke('agent_send', { key: info.key, action: { type: 'getState' } })).rejects.toMatchObject({ code: 'notFound' });
  });

  it('refuses a project that is neither active nor recent, and a ninth session', async () => {
    const { transport } = await start();

    await expect(transport.invoke('agent_start', { request: { ...REQUEST, project: '/etc' }, onEvent: () => {} })).rejects.toMatchObject({ code: 'forbidden' });

    for (let count = 1; count < 8; count += 1) {
      await transport.invoke('agent_start', { request: REQUEST, onEvent: () => {} });
    }

    await expect(transport.invoke('agent_start', { request: REQUEST, onEvent: () => {} })).rejects.toMatchObject({ code: 'unsupported' });
  });
});
