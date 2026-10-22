import { waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';

import { prefs } from '$lib/stores/prefs.svelte';

import { startedSession } from '../../../tests/chat-helpers';
import { samplePrefs } from '../../../tests/helpers';
import { applyChatDefaults, rememberModel, rememberThinking, savedChatDefaults } from './chat-defaults';

afterEach(() => {
  prefs.value = null;
});

describe('chat defaults', () => {
  it('remembers the last model and thinking level per harness', () => {
    prefs.value = samplePrefs();

    rememberModel('omp', { provider: 'openai', id: 'gpt-5' } as never);
    rememberThinking('omp', 'high');
    rememberThinking('pi', 'low');

    expect(savedChatDefaults('omp')).toEqual({ harness: 'omp', provider: 'openai', modelId: 'gpt-5', thinkingLevel: 'high' });
    expect(savedChatDefaults('pi')).toEqual({ harness: 'pi', provider: null, modelId: null, thinkingLevel: 'low' });
  });

  it('starts a new chat on the saved model, then the saved thinking level', async () => {
    const { session, of } = await startedSession();
    prefs.value = samplePrefs({ chatDefaults: [{ harness: 'omp', provider: 'openai', modelId: 'gpt-5', thinkingLevel: 'high' }] });

    await applyChatDefaults(session);

    const actions = of('agent_send').map((call) => call.action);
    expect(actions).toContainEqual({ type: 'setModel', provider: 'openai', modelId: 'gpt-5' });
    expect(actions.findIndex((action) => action.type === 'setThinkingLevel')).toBeGreaterThan(actions.findIndex((action) => action.type === 'setModel'));
    expect(actions).toContainEqual({ type: 'setThinkingLevel', level: 'high' });
  });

  it('skips a model or level the agent no longer offers', async () => {
    const { session, of } = await startedSession();
    prefs.value = samplePrefs({ chatDefaults: [{ harness: 'omp', provider: 'gone', modelId: 'retired', thinkingLevel: 'ludicrous' }] });

    await applyChatDefaults(session);

    const types = of('agent_send').map((call) => call.action.type);
    expect(types).not.toContain('setModel');
    expect(types).not.toContain('setThinkingLevel');
  });
});

describe('chat defaults over the model a Claude Code chat starts on', () => {
  // The mock agent starts on Sonnet 4.5 at medium, as a chat that Harness Skills' default model role started.
  it('keeps that model and level when nothing is saved', async () => {
    prefs.value = samplePrefs();

    const { session, of } = await startedSession('claude');
    await waitFor(() => expect(session.chat.session?.model?.id).toBe('claude-sonnet-4-5'));
    await applyChatDefaults(session);

    const types = of('agent_send').map((call) => call.action.type);
    expect(types).not.toContain('setModel');
    expect(types).not.toContain('setThinkingLevel');
    expect(session.chat.session?.model?.id).toBe('claude-sonnet-4-5');
    expect(session.chat.session?.thinkingLevel).toBe('medium');
  });

  it('puts a new chat on the saved model and level instead', async () => {
    prefs.value = samplePrefs({ chatDefaults: [{ harness: 'claude', provider: 'anthropic', modelId: 'claude-haiku-4-5', thinkingLevel: 'high' }] });

    const { session, of } = await startedSession('claude');

    await waitFor(() => expect(session.chat.session?.thinkingLevel).toBe('high'));
    expect(session.chat.session?.model?.id).toBe('claude-haiku-4-5');
    expect(of('agent_send').map((call) => call.action)).toContainEqual({ type: 'setModel', provider: 'anthropic', modelId: 'claude-haiku-4-5' });
  });
});
