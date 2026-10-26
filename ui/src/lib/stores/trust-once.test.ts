import { afterEach, describe, expect, it, vi } from 'vitest';

import type { AgentStartRequest } from '$lib/ipc/bindings/AgentStartRequest';
import type { CommandArgs } from '$lib/ipc/commands';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { PROJECT, recordingTransport } from '../../../tests/chat-helpers';
import { samplePrefs } from '../../../tests/helpers';
import { AgentSessionsStore } from './agentSession.svelte';
import { nav } from './nav.svelte';
import { prefs } from './prefs.svelte';
import { forkInChat, resumeInChat } from './session-actions';
import { ToastsStore } from './toasts.svelte';

const OTHER = fixtures.recentProjects[1].path;
const SAVED = { harness: 'claude', id: '0c5ad1e4-0000-4000-8000-000000000001' } as const;

/**
 * The mock backend, where a chat in a folder of `untrusted` starts only when it trusts the folder once (as Claude Code
 * refuses a folder it does not trust). Every start request is kept, refused ones too.
 */
function refusingBackend() {
  const backend = recordingTransport();
  const untrusted = new Set<string>();
  const starts: AgentStartRequest[] = [];
  const transport: Transport = {
    invoke(name, args) {
      if (name === 'agent_start') {
        const { request } = args as CommandArgs<'agent_start'>;
        starts.push(request);

        if (untrusted.has(request.project) && !request.trustOnce) {
          return Promise.reject(new IpcFailure({ code: 'untrusted', message: 'Claude Code does not trust this folder yet.' }));
        }
      }

      return backend.transport.invoke(name, args);
    },
  };
  const notifications = new ToastsStore();
  const store = new AgentSessionsStore(transport, notifications);

  /** Picks "Trust once" on the only toast. */
  const trustOnce = () => {
    expect(notifications.items).toHaveLength(1);
    expect(notifications.items[0].action?.label).toBe('Trust once');
    notifications.items[0].action?.run();
  };

  const actions = (key: string) => backend.of('agent_send').filter((call) => call.key === key).map((call) => call.action);

  return { store, notifications, untrusted, starts, trustOnce, actions };
}

/** Gives `session` a conversation the agent saved, so a restart resumes it. */
async function withConversation(session: { chat: { session: { sessionId: string } | null; items: unknown[] } }) {
  await vi.waitFor(() => expect(session.chat.session?.sessionId).toBeTruthy());
  session.chat.items = [{ kind: 'message', id: 'm1', role: 'user', blocks: [{ type: 'text', text: 'hi' }] }];
}

afterEach(() => {
  prefs.value = null;
});

describe('"Trust once" runs the whole flow that was refused again', () => {
  it('creates the chat again with the chat defaults applied', async () => {
    const { store, notifications, untrusted, starts, trustOnce, actions } = refusingBackend();
    prefs.value = samplePrefs({ chatDefaults: [{ harness: 'claude', provider: null, modelId: null, thinkingLevel: 'minimal' }] });
    untrusted.add(PROJECT);

    expect(await store.create('claude', PROJECT)).toBeNull();
    expect(store.starting).toHaveLength(0);
    expect(notifications.items[0].title).toMatch(new RegExp(` does not trust ${PROJECT.split('/').at(-1)} yet$`));
    expect(notifications.items[0].project).toBe(PROJECT);

    trustOnce();

    await vi.waitFor(() => expect(store.sessions).toHaveLength(1));
    const chat = store.sessions[0];
    expect(store.activeKey).toBe(chat.key);
    expect(chat.info.trustedOnce).toBe(true);
    expect(starts.map((request) => request.trustOnce)).toEqual([false, true]);
    await vi.waitFor(() => expect(actions(chat.key)).toContainEqual({ type: 'setThinkingLevel', level: 'minimal' }));
  });

  it('resumes a saved session in the chat, loading its history into the empty chat it takes over', async () => {
    const { store, untrusted, starts, trustOnce, actions } = refusingBackend();
    const empty = await store.create('claude', PROJECT);
    untrusted.add(PROJECT);
    nav.go('sessions');

    expect(await resumeInChat(store, SAVED, PROJECT)).toBe(false);

    trustOnce();

    await vi.waitFor(() => expect(store.get(empty!.key)).toBeUndefined());
    const resumed = store.sessions[0];
    expect(store.sessions).toHaveLength(1);
    expect(starts.at(-1)).toMatchObject({ resume: SAVED, trustOnce: true });
    await vi.waitFor(() => expect(actions(resumed.key)).toContainEqual({ type: 'getHistory', cursor: null, limit: expect.any(Number) }));
    expect(nav.current).toBe('chat');
  });

  it('forks the saved session, never just resuming it', async () => {
    const { store, untrusted, starts, trustOnce, actions } = refusingBackend();
    untrusted.add(PROJECT);

    expect(await forkInChat(store, SAVED, 'entry-7', PROJECT)).toBe(false);

    trustOnce();

    await vi.waitFor(() => expect(store.sessions).toHaveLength(1));
    const forked = store.sessions[0];
    expect(starts.at(-1)).toMatchObject({ resume: SAVED, trustOnce: true });
    await vi.waitFor(() => expect(actions(forked.key)).toContainEqual({ type: 'fork', entryId: 'entry-7' }));
  });

  it('hands the chat to the other harness in place, keeping its draft and name', async () => {
    const { store, untrusted, trustOnce } = refusingBackend();
    const first = await store.create('omp', PROJECT);
    const second = await store.create('omp', PROJECT);
    const third = await store.create('omp', PROJECT);
    second!.draft = 'half a thought';
    second!.aiLabel = { name: 'Release notes', description: '' };
    second!.userNamed = true;
    untrusted.add(PROJECT);

    expect(await store.changeHarness(second!.key, 'claude')).toBeNull();
    expect(store.get(second!.key)?.switchingTo).toBeNull();

    trustOnce();

    await vi.waitFor(() => expect(store.get(second!.key)).toBeUndefined());
    const fresh = store.sessions[1];
    expect(store.sessions.map((session) => session.key)).toEqual([first!.key, fresh.key, third!.key]);
    expect(fresh.info).toMatchObject({ harness: 'claude', trustedOnce: true });
    expect(fresh.draft).toBe('half a thought');
    expect(fresh).toMatchObject({ aiLabel: { name: 'Release notes' }, userNamed: true });
  });

  it('restarts the chat in its tab with its draft and history', async () => {
    const { store, untrusted, starts, trustOnce, actions } = refusingBackend();
    const first = await store.create('claude', PROJECT);
    const second = await store.create('claude', PROJECT);
    await withConversation(first as never);
    const sessionId = first!.chat.session!.sessionId;
    first!.draft = 'half-written';
    untrusted.add(PROJECT);

    expect(await store.restart(first!.key, {})).toBeNull();
    expect(store.sessions.map((session) => session.key)).toEqual([second!.key]);

    trustOnce();

    await vi.waitFor(() => expect(store.sessions).toHaveLength(2));
    const fresh = store.sessions[0];
    expect(store.sessions[1]).toBe(second);
    expect(fresh.draft).toBe('half-written');
    expect(starts.at(-1)).toMatchObject({ resume: { harness: 'claude', id: sessionId }, trustOnce: true });
    await vi.waitFor(() => expect(actions(fresh.key).some((action) => action.type === 'getHistory')).toBe(true));
  });
});

describe('reopening last run\'s chats in folders the harness does not trust yet', () => {
  const chat = (harness: 'claude' | 'omp', project: string, id: string) => ({ harness, project, sessionId: id, label: null, userNamed: false });

  it('lists them in one toast whose "Trust once" reopens them in their places', async () => {
    const { store, notifications, untrusted, starts, trustOnce, actions } = refusingBackend();
    untrusted.add(OTHER);
    const chats = [chat('claude', OTHER, 'c-1'), chat('omp', PROJECT, 'o-1'), chat('claude', OTHER, 'c-2')];

    expect(await store.restore(chats, 0)).toBe(1);
    expect(store.sessions).toHaveLength(1);
    expect(store.starting).toHaveLength(0);
    expect(notifications.items).toHaveLength(1);
    expect(notifications.items[0].title).toBe('2 chats stayed closed');
    expect(notifications.items[0].description).toContain(` does not trust ${OTHER.split('/').at(-1)} yet. Trust once to reopen them.`);

    const omp = store.sessions[0];
    trustOnce();

    await vi.waitFor(() => expect(store.sessions).toHaveLength(3));
    expect(store.sessions[1]).toBe(omp);
    expect(store.sessions.map((session) => session.info.harness)).toEqual(['claude', 'omp', 'claude']);
    expect(starts.slice(3).map((request) => [request.resume?.id, request.trustOnce])).toEqual([
      ['c-1', true],
      ['c-2', true],
    ]);
    expect(store.activeKey).toBe(store.sessions[0].key);
    for (const reopened of [store.sessions[0], store.sessions[2]]) {
      await vi.waitFor(() => expect(actions(reopened.key).some((action) => action.type === 'getHistory')).toBe(true));
    }
  });
});

describe('the toast of a folder to trust', () => {
  it('says so when the settings file holding the trust answers cannot be read, and still offers to trust once', async () => {
    const notifications = new ToastsStore();
    const transport: Transport = {
      invoke: () => Promise.reject(new IpcFailure({ code: 'untrustedUnreadable', message: "Claude Code's settings file could not be read, so this folder counts as untrusted." })),
    };
    const store = new AgentSessionsStore(transport, notifications);

    expect(await store.start({ harness: 'claude', project: PROJECT, resume: null, approvalOverride: null, trustOnce: false, utility: false, memoryChat: null })).toBeNull();

    const toast = notifications.items[0];
    expect(toast.description).toContain('could not be read, so this folder counts as untrusted. Fix the file, or trust the folder once here.');
    expect(toast.description).toContain('It applies to every chat in this folder until you quit the app.');
    expect(toast.action?.label).toBe('Trust once');
  });

  it('goes away when another project becomes the active one', () => {
    const notifications = new ToastsStore();
    notifications.push({ title: 'About this project', project: PROJECT });
    notifications.push({ title: 'About nothing in particular' });

    notifications.leaveProjects(PROJECT);
    expect(notifications.items).toHaveLength(2);

    notifications.leaveProjects(OTHER);
    expect(notifications.items.map((item) => item.title)).toEqual(['About nothing in particular']);
  });
});
