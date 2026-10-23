import { waitFor } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { SessionMessagesPage } from '$lib/ipc/bindings/SessionMessagesPage';
import { ipc } from '$lib/ipc/client';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { fakeTransport } from '../../../tests/helpers';
import { PROJECT, recordingTransport, storeWithMock } from '../../../tests/chat-helpers';
import { nav } from './nav.svelte';
import { forkInChat, resumeInChat } from './session-actions';
import { SessionDetail } from './sessions.svelte';
import { AgentSessionsStore } from './agentSession.svelte';
import { toasts, ToastsStore } from './toasts.svelte';
import { viewState } from './view-state.svelte';

const REF = fixtures.sessions.list.omp[1].ref;
const TOTAL = fixtures.sessions.messages.messages.length;

describe('session detail', () => {
  it('loads the first transcript page with the session totals, and the touched files', async () => {
    const detail = new SessionDetail(recordingTransport().transport);

    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('ready'));

    expect(detail.stats).toEqual(fixtures.sessions.messages.stats);
    expect(detail.messages).toHaveLength(Math.min(TOTAL, 100));
    expect(detail.touched).toEqual(fixtures.sessions.filesTouched);
  });

  it('pages through the transcript by cursor until the end', async () => {
    const backend = recordingTransport();
    const detail = new SessionDetail(backend.transport, 5);

    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('ready'));

    const pages = Math.ceil(TOTAL / 5);
    expect(detail.messages).toHaveLength(Math.min(5, TOTAL));
    expect(detail.total).toBe(TOTAL);

    while (detail.nextCursor !== null) {
      const before = detail.messages.length;
      await detail.loadMore();
      expect(detail.messages.length).toBeGreaterThan(before);
    }

    expect(detail.messages.map((message) => message.id)).toEqual(fixtures.sessions.messages.messages.map((message) => message.id));

    const requests = backend.of('session_messages');
    expect(requests).toHaveLength(pages);
    expect(requests.every((request) => request.limit === 5)).toBe(true);
    expect(requests[0].cursor).toBeNull();
    expect(requests.slice(1).every((request) => request.cursor !== null)).toBe(true);
  });

  it('follows a session being written by re-reading only its newest loaded page', async () => {
    const backend = recordingTransport();
    const detail = new SessionDetail(backend.transport, 5);
    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('ready'));
    await detail.loadMore();
    const shown = detail.messages.map((message) => message.id);
    const lastCursor = backend.of('session_messages').at(-1)?.cursor;

    await detail.follow();

    expect(backend.of('session_messages').at(-1)?.cursor).toBe(lastCursor);
    expect(detail.messages.map((message) => message.id)).toEqual(shown);
    expect(detail.transcriptStatus).toBe('ready');
  });

  it('keeps the transcript on screen when following fails', async () => {
    const backend = recordingTransport();
    let fail = false;
    const flaky: Transport = {
      invoke: (name, args) => (fail ? Promise.reject(IpcFailure.from({ code: 'io', message: 'busy' })) : backend.transport.invoke(name, args)),
    };
    const detail = new SessionDetail(flaky, 5);
    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('ready'));
    const shown = detail.messages;

    fail = true;
    await detail.follow();

    expect(detail.messages).toBe(shown);
    expect(detail.transcriptStatus).toBe('ready');
  });

  it('does not read the newest page again while the previous look is still running', async () => {
    const backend = recordingTransport();
    let hold = false;
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const slow: Transport = {
      invoke: (name, args) => (hold && name === 'session_messages' ? gate.then(() => backend.transport.invoke(name, args)) : backend.transport.invoke(name, args)),
    };
    const detail = new SessionDetail(slow, 5);
    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('ready'));
    const before = backend.of('session_messages').length;

    hold = true;
    const looks = [detail.follow(), detail.follow(), detail.follow()];
    release();
    await Promise.all(looks);

    expect(backend.of('session_messages')).toHaveLength(before + 1);
  });

  it('looks again at a newly opened session even if the look at the previous one never got an answer', async () => {
    const backend = recordingTransport();
    let hang = false;
    const stuck: Transport = {
      invoke: (name, args) => (hang && name === 'session_messages' ? new Promise(() => {}) : backend.transport.invoke(name, args)),
    };
    const detail = new SessionDetail(stuck, 5);
    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('ready'));

    hang = true;
    void detail.follow();
    hang = false;
    const other = fixtures.sessions.list.omp[2].ref;
    detail.open(other, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('ready'));
    const reads = backend.of('session_messages').length;

    await detail.follow();

    expect(backend.of('session_messages')).toHaveLength(reads + 1);
  });

  it('reads a transcript that failed to load again when followed, and shows it once it works', async () => {
    const backend = recordingTransport();
    let fail = true;
    const flaky: Transport = {
      invoke: (name, args) => (fail && name === 'session_messages' ? Promise.reject(IpcFailure.from({ code: 'io', message: 'busy' })) : backend.transport.invoke(name, args)),
    };
    const detail = new SessionDetail(flaky, 5);
    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('error'));

    fail = false;
    await detail.follow();

    expect(detail.transcriptStatus).toBe('ready');
    expect(detail.messages.length).toBeGreaterThan(0);
  });

  it('refreshes the totals when following a session whose newest page is the first', async () => {
    const totals = { messages: 4, toolCalls: 1, contextTokens: null, costUsd: null, models: [], startedAt: null, endedAt: null };
    let reply = totals;
    const transport = fakeTransport({
      session_files_touched: () => [],
      session_messages: () => ({ messages: [], nextCursor: null, total: 0, stats: reply, truncated: false }),
    });
    const detail = new SessionDetail(transport);
    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.stats).toEqual(totals));

    reply = { ...totals, messages: 6 };
    await detail.follow();

    expect(detail.stats?.messages).toBe(6);
  });

  it('keeps the totals when following a later page, which does not carry them', async () => {
    const backend = recordingTransport();
    const detail = new SessionDetail(backend.transport, 5);
    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.stats).not.toBeNull());
    await detail.loadMore();
    const stats = detail.stats;

    await detail.follow();

    expect(backend.of('session_messages').at(-1)?.cursor).not.toBeNull();
    expect(detail.stats).toBe(stats);
  });

  it('shows tool results on the card of their call, even across a page boundary', async () => {
    const detail = new SessionDetail(recordingTransport().transport, 2);

    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('ready'));
    while (detail.nextCursor !== null) {
      await detail.loadMore();
    }

    const answered = new Set(fixtures.sessions.messages.messages.flatMap((message) => (message.toolCallId ? [message.toolCallId] : [])));
    const calls = Object.values(detail.transcript.toolCalls);
    expect(answered.size).toBeGreaterThan(0);
    for (const id of answered) {
      // A call whose result sits on a later page still ends up finished; calls without a result stay pending.
      expect(['ok', 'error']).toContain(detail.transcript.toolCalls[id]?.status);
    }
    expect(calls.filter((call) => !answered.has(call.id)).every((call) => call.status === 'pending')).toBe(true);
    // Tool results fold into cards, so they never appear as rows of their own.
    expect(detail.transcript.items.every((item) => item.kind !== 'message' || item.role !== 'toolResult')).toBe(true);
  });

  it('ignores a slow answer for a session that is no longer selected', async () => {
    let releaseFirst: (page: SessionMessagesPage) => void = () => {};
    const slow = new Promise<SessionMessagesPage>((resolve) => (releaseFirst = resolve));
    const second = fixtures.sessions.list.omp[2].ref;
    const transport = fakeTransport({
      session_files_touched: () => [],
      session_messages: ({ session }: { session: typeof REF }) => (session.id === REF.id ? slow : { messages: [], nextCursor: null, total: 0, stats: null, truncated: false }),
    });
    const detail = new SessionDetail(transport);

    detail.open(REF, PROJECT);
    detail.open(second, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('ready'));
    releaseFirst(fixtures.sessions.messages);
    await new Promise((resolve) => setTimeout(resolve, 20));

    expect(detail.ref?.id).toBe(second.id);
    expect(detail.stats).toBeNull();
    expect(detail.messages).toHaveLength(0);
  });

  it('drops the totals and touched files of the previous session the moment another is selected', async () => {
    const second = fixtures.sessions.list.omp[2].ref;
    const never = new Promise<never>(() => {});
    const transport = fakeTransport({
      session_files_touched: ({ session }: { session: typeof REF }) => (session.id === REF.id ? fixtures.sessions.filesTouched : never),
      session_messages: ({ session }: { session: typeof REF }) => (session.id === REF.id ? fixtures.sessions.messages : never),
    });
    const detail = new SessionDetail(transport);
    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.stats).not.toBeNull());
    await waitFor(() => expect(detail.touched).not.toHaveLength(0));

    detail.open(second, PROJECT);

    expect(detail.stats).toBeNull();
    expect(detail.touched).toEqual([]);
    // The transcript itself stays on screen until the new one arrives or the wait gets long.
    expect(detail.messages.length).toBeGreaterThan(0);
  });

  it('shows no totals of the previous session when the next one cannot be read', async () => {
    const second = fixtures.sessions.list.omp[2].ref;
    const transport = fakeTransport({
      session_files_touched: ({ session }: { session: typeof REF }) => (session.id === REF.id ? fixtures.sessions.filesTouched : Promise.reject(IpcFailure.from({ code: 'io', message: 'Busy.' }))),
      session_messages: ({ session }: { session: typeof REF }) => (session.id === REF.id ? fixtures.sessions.messages : Promise.reject(IpcFailure.from({ code: 'io', message: 'Busy.' }))),
    });
    const detail = new SessionDetail(transport);
    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.stats).not.toBeNull());

    detail.open(second, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('error'));

    expect(detail.stats).toBeNull();
    expect(detail.touched).toEqual([]);
  });

  it('says when a session is longer than the app reads, and forgets it for the next session', async () => {
    const backend = recordingTransport();
    const cut: Transport = {
      invoke: async (name, args) => {
        const reply = await backend.transport.invoke(name, args);
        const long = name === 'session_messages' && (args as { session: typeof REF }).session.id === REF.id;

        return (long ? { ...(reply as SessionMessagesPage), truncated: true } : reply) as never;
      },
    };
    const detail = new SessionDetail(cut, 5);

    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('ready'));
    expect(detail.truncated).toBe(true);

    detail.open(fixtures.sessions.list.omp[0].ref, PROJECT);
    expect(detail.truncated).toBe(false);
  });

  it('reads a session that changed under its pages again from the start, with no error', async () => {
    const backend = recordingTransport();
    let changed = false;
    const changing: Transport = {
      invoke: (name, args) => {
        const later = name === 'session_messages' && (args as { cursor: string | null }).cursor !== null;
        if (changed && later) {
          changed = false;
          return Promise.reject(IpcFailure.from({ code: 'changed', message: 'the session changed while it was read: open it again' }));
        }

        return backend.transport.invoke(name, args);
      },
    };
    const detail = new SessionDetail(changing, 5);
    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('ready'));
    await detail.loadMore();
    expect(detail.messages).toHaveLength(Math.min(10, TOTAL));

    changed = true;
    await detail.loadMore();

    expect(backend.of('session_messages').map((request) => request.cursor === null)).toEqual([true, false, true]);
    expect(detail.transcriptError).toBeNull();
    expect(detail.transcriptStatus).toBe('ready');
    expect(detail.messages).toHaveLength(Math.min(5, TOTAL));
    expect(detail.loadingMore).toBe(false);

    // Following the session does the same.
    await detail.loadMore();
    changed = true;
    await detail.follow();

    expect(backend.of('session_messages').at(-1)?.cursor).toBeNull();
    expect(detail.messages).toHaveLength(Math.min(5, TOTAL));
    expect(detail.transcriptError).toBeNull();
  });

  it('reports a transcript that cannot be read', async () => {
    const transport: Transport = {
      invoke: async (name) => {
        if (name === 'session_messages') {
          throw IpcFailure.from({ code: 'notFound', message: 'That session no longer exists.' });
        }

        return [] as never;
      },
    };
    const detail = new SessionDetail(transport);

    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('error'));

    expect(detail.transcriptError?.message).toBe('That session no longer exists.');
  });
});

describe('resuming and forking', () => {
  it('resumes a saved session in a new agent and opens the chat', async () => {
    const { store, of } = storeWithMock();
    nav.go('sessions');

    expect(await resumeInChat(store, REF, PROJECT)).toBe(true);

    expect(of('agent_start')).toHaveLength(1);
    expect(of('agent_start')[0]).toMatchObject({ request: { harness: REF.harness, project: PROJECT, resume: REF } });
    expect(store.sessions).toHaveLength(1);
    expect(nav.current).toBe('chat');
    await waitFor(() => expect(of('agent_send').some(({ action }) => action.type === 'getHistory')).toBe(true));
  });

  it('resumes into an empty chat instead of opening another tab', async () => {
    const { store, of } = storeWithMock();
    const empty = await store.create('omp', PROJECT);
    expect(empty?.chat.items).toHaveLength(0);
    nav.go('sessions');

    expect(await resumeInChat(store, REF, PROJECT)).toBe(true);

    expect(store.sessions).toHaveLength(1);
    expect(store.sessions[0].key).not.toBe(empty!.key);
    expect(store.activeKey).toBe(store.sessions[0].key);
    expect(store.title(store.sessions[0].key)).not.toBe('Chat 2');
    expect(of('agent_stop').map(({ key }) => key)).toContain(empty!.key);
  });

  it('keeps a chat the user has typed in and opens the session next to it', async () => {
    const { store } = storeWithMock();
    const busy = await store.create('omp', PROJECT);
    busy!.draft = 'half-written question';

    await resumeInChat(store, REF, PROJECT);

    expect(store.sessions).toHaveLength(2);
    expect(store.get(busy!.key)).toBeDefined();
  });

  it('keeps the previous session on screen while a quick switch loads', async () => {
    const detail = new SessionDetail(recordingTransport().transport);
    detail.open(REF, PROJECT);
    await waitFor(() => expect(detail.transcriptStatus).toBe('ready'));
    const other = fixtures.sessions.list.omp[0].ref;

    detail.open(other, PROJECT);

    expect(detail.transcriptStatus).toBe('ready');
    expect(detail.messages.length).toBeGreaterThan(0);
  });

  it('opens the chat that already runs a session instead of starting a second agent', async () => {
    const { store, of } = storeWithMock();
    const running = { harness: 'omp', id: fixtures.agent.state.state.sessionId } as const;

    await store.create('omp', PROJECT);
    await waitFor(() => expect(store.sessions[0].chat.session?.sessionId).toBe(running.id));
    nav.go('sessions');

    await resumeInChat(store, running, PROJECT);

    expect(of('agent_start')).toHaveLength(1);
    expect(store.sessions).toHaveLength(1);
    expect(nav.current).toBe('chat');
  });

  it('forks from an entry: resume, then fork, then reload the history', async () => {
    const { store, calls } = storeWithMock();
    nav.go('sessions');

    expect(await forkInChat(store, REF, 'n09', PROJECT)).toBe(true);

    const names = calls.map((call) => (call[0] === 'agent_send' ? `send:${call[1].action.type}` : call[0]));
    const start = names.indexOf('agent_start');
    const fork = names.indexOf('send:fork');
    const history = names.indexOf('send:getHistory');

    expect(start).toBeGreaterThanOrEqual(0);
    expect(fork).toBeGreaterThan(start);
    expect(history).toBeGreaterThan(fork);
    expect(calls[fork][1]).toMatchObject({ action: { type: 'fork', entryId: 'n09' } });
    expect(calls[start][1]).toMatchObject({ request: { resume: REF } });
    expect(nav.current).toBe('chat');
  });

  it('forks inside the chat that already runs the session, without a second agent', async () => {
    const { store, of } = storeWithMock();
    const running = { harness: 'omp', id: fixtures.agent.state.state.sessionId } as const;
    const chat = await store.create('omp', PROJECT);
    await waitFor(() => expect(chat?.chat.session?.sessionId).toBe(running.id));
    await chat!.loadHistory();
    expect(chat!.chat.items.length).toBeGreaterThan(0);
    const shown = chat!.chat.items.length;
    // What the fork's path no longer contains.
    chat!.chat.items = [...chat!.chat.items, { kind: 'notice', id: 'stale', level: 'info', message: 'Before the fork' } as never];
    await store.create('omp', PROJECT);
    expect(store.activeKey).not.toBe(chat!.key);
    nav.go('sessions');

    expect(await forkInChat(store, running, 'n09', PROJECT)).toBe(true);

    expect(of('agent_start')).toHaveLength(2);
    expect(store.sessions).toHaveLength(2);
    expect(store.activeKey).toBe(chat!.key);
    expect(nav.current).toBe('chat');
    expect(chat!.chat.items).toHaveLength(shown);
    expect(of('agent_send').filter(({ action }) => action.type === 'fork')).toEqual([{ key: chat!.key, action: { type: 'fork', entryId: 'n09' } }]);
  });

  it('leaves the open chat running when its fork is refused', async () => {
    const backend = recordingTransport();
    const refusing: Transport = {
      invoke: (name, args) =>
        name === 'agent_send' && (args as { action: { type: string } }).action.type === 'fork'
          ? Promise.reject(IpcFailure.from({ code: 'invalidInput', message: 'Cannot fork there.' }))
          : backend.transport.invoke(name, args),
    };
    const store = new AgentSessionsStore(refusing, new ToastsStore());
    const running = { harness: 'omp', id: fixtures.agent.state.state.sessionId } as const;
    const chat = await store.create('omp', PROJECT);
    await waitFor(() => expect(chat?.chat.session?.sessionId).toBe(running.id));
    nav.go('sessions');

    expect(await forkInChat(store, running, 'n09', PROJECT)).toBe(false);

    expect(backend.of('agent_start')).toHaveLength(1);
    expect(backend.of('agent_stop')).toHaveLength(0);
    expect(store.get(chat!.key)).toBeDefined();
    expect(nav.current).toBe('sessions');
  });

  it('stops the agent again when the fork is refused', async () => {
    const backend = recordingTransport();
    const refusing: Transport = {
      invoke: (name, args) =>
        name === 'agent_send' && (args as { action: { type: string } }).action.type === 'fork'
          ? Promise.reject(IpcFailure.from({ code: 'invalidInput', message: 'Cannot fork there.' }))
          : backend.transport.invoke(name, args),
    };
    const notifications = new ToastsStore();
    const store = new AgentSessionsStore(refusing, notifications);
    nav.go('sessions');

    expect(await forkInChat(store, REF, 'n09', PROJECT)).toBe(false);

    expect(store.sessions).toHaveLength(0);
    expect(backend.of('agent_stop')).toHaveLength(1);
    expect(nav.current).toBe('sessions');
    expect(notifications.items.at(-1)?.description).toBe('Cannot fork there.');
  });
});
