import { describe, expect, it } from 'vitest';

import type { AgentAction } from '$lib/ipc/bindings/AgentAction';
import type { AgentReply } from '$lib/ipc/bindings/AgentReply';
import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';
import { ToastsStore } from '$lib/stores/toasts.svelte';

import { sampleInfo, STREAMING } from '../../../tests/chat-helpers';
import { fakeTransport } from '../../../tests/helpers';
import type { Attachment } from './attachments';
import { hasPendingSend } from './pending-send';
import { ChatSession } from './session.svelte';
import type { MessageItem } from './types';

const HOOKS = '<harness-hook-context>\n<hook name="Harness Memory">Search first.</hook>\n</harness-hook-context>\n\n';

/** A session whose harness answers each action through `reply`, held until `release` when a test needs the send to stay open. */
function setup(reply: (action: AgentAction) => AgentReply | Promise<AgentReply> = () => ({ type: 'ack' })) {
  const actions: AgentAction[] = [];
  const transport = fakeTransport({
    agent_send: ({ action }) => {
      actions.push(action);

      return reply(action);
    },
  });
  const notifications = new ToastsStore();
  const session = new ChatSession(sampleInfo(), transport, notifications);
  let seq = 0;

  const emit = (event: Parameters<ChatSession['ingest']>[0]['event']) => {
    seq += 1;
    session.ingest({ sessionKey: session.key, seq, event });
    session.flush();
  };
  emit({ type: 'ready', protocolVersion: 1 });

  return { session, actions, notifications, emit };
}

function echo(id: string, text: string): ChatMessage {
  return { id, role: 'user', blocks: [{ type: 'text', text }] } as unknown as ChatMessage;
}

function users(session: ChatSession): MessageItem[] {
  return session.chat.items.filter((item): item is MessageItem => item.kind === 'message' && item.role === 'user');
}

function wordsOf(item: MessageItem): string {
  return item.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n');
}

describe('a prompt shown before the harness echoes it', () => {
  it('appears in the transcript at once, before the send has been answered', async () => {
    let answer!: () => void;
    const { session } = setup(() => new Promise((resolve) => (answer = () => resolve({ type: 'ack' }))));

    const sending = session.submit('Fix the login test', 'send');

    expect(users(session)).toHaveLength(1);
    expect(wordsOf(users(session)[0])).toBe('Fix the login test');
    expect(users(session)[0].send?.status).toBe('sending');

    answer();
    expect(await sending).toBe(true);
    expect(users(session)).toHaveLength(1);
  });

  it('shows the thumbnails of attached images and the file list while it waits', () => {
    const { session } = setup(() => new Promise(() => {}));
    const image = { id: 'a1', name: 'shot.png', url: 'data:image/png;base64,AAA', data: 'AAA', mimeType: 'image/png' } as Attachment;

    void session.submit('Look at this', 'send', { images: [image], files: [{ name: 'notes.md', path: '/p/notes.md', bytes: 10 }] });

    const item = users(session)[0];
    expect(wordsOf(item)).toContain('Look at this');
    expect(wordsOf(item)).toContain('/p/notes.md');
    expect(item.blocks.some((block) => block.type === 'image')).toBe(true);
    expect(session.images.byKey[item.key]).toEqual(['data:image/png;base64,AAA']);
  });

  it('keeps the thumbnails of an attached image once the echo replaces the waiting message', () => {
    const { session, emit } = setup(() => new Promise(() => {}));
    const image = { id: 'a1', name: 'shot.webp', url: 'data:image/webp;base64,AAA', data: 'AAA', mimeType: 'image/webp' } as Attachment;

    void session.submit('Can you implement it?', 'send', { images: [image], files: [] });
    emit({ type: 'messageStart', messageId: 'u1', role: 'user' });
    emit({
      type: 'messageEnd',
      messageId: 'u1',
      message: { id: 'u1', role: 'user', blocks: [{ type: 'image', mimeType: 'image/webp' }, { type: 'text', text: `${HOOKS}Can you implement it?` }] } as unknown as ChatMessage,
    });

    const item = users(session)[0];
    expect(item.send?.status).not.toBe('sending');
    expect(session.images.byKey[item.key] ?? session.images.byKey[`p:${item.send?.id}`]).toEqual(['data:image/webp;base64,AAA']);
  });

  it('keeps the thumbnails when the echo carries no image blocks of its own', () => {
    const { session, emit } = setup(() => new Promise(() => {}));
    const image = { id: 'a1', name: 'shot.webp', url: 'data:image/webp;base64,AAA', data: 'AAA', mimeType: 'image/webp' } as Attachment;

    void session.submit('Can you implement it?', 'send', { images: [image], files: [] });
    emit({ type: 'messageStart', messageId: 'u1', role: 'user' });
    emit({ type: 'messageEnd', messageId: 'u1', message: echo('u1', `${HOOKS}Can you implement it?`) });

    const item = users(session)[0];
    expect(session.images.byKey[item.key] ?? session.images.byKey[`p:${item.send?.id}`]).toEqual(['data:image/webp;base64,AAA']);
  });

  it('is replaced in place by the echo: one message, the same position, the real key and the time', () => {
    const { session, emit } = setup(() => new Promise(() => {}));
    emit({ type: 'agentStart' });
    emit({ type: 'agentEnd', messages: [] } as never);
    emit({ type: 'messageEnd', messageId: 'old', message: echo('old', 'Earlier question') });
    void session.submit('Next question', 'send');
    const waiting = users(session)[1];

    emit({ type: 'messageStart', messageId: 'u2', role: 'user' });
    emit({ type: 'messageEnd', messageId: 'u2', message: { ...echo('u2', 'Next question'), timestamp: 1_700_000_000_000 } as ChatMessage });
    emit({ type: 'messageEnd', messageId: 'a1', message: { id: 'a1', role: 'assistant', blocks: [{ type: 'text', text: 'On it' }] } as unknown as ChatMessage });

    const all = users(session);
    expect(all).toHaveLength(2);
    expect(all[1].key).toBe('m:u2');
    expect(all[1].order).toBe(waiting.order);
    expect(all[1].send).toEqual(expect.objectContaining({ id: waiting.send?.id, status: 'sent' }));
    expect(all[1].meta?.timestamp).toBe(1_700_000_000_000);
    expect(session.chat.items.map((item) => item.key)).toEqual(['m:old', 'm:u2', 'm:a1']);
  });

  it('matches an echo that carries the hook context the app put in front of the words', () => {
    const { session, emit } = setup(() => new Promise(() => {}));
    void session.submit('Fix the login test', 'send');

    emit({ type: 'messageEnd', messageId: 'u1', message: echo('u1', `${HOOKS}Fix the login test`) });

    expect(users(session)).toHaveLength(1);
    expect(users(session)[0].key).toBe('m:u1');
    expect(session.firstPrompt).toBe('Fix the login test');
  });

  it('matches two quick prompts to their echoes in order, even when they say the same', () => {
    const { session, emit } = setup(() => new Promise(() => {}));
    void session.submit('same words', 'send');
    void session.submit('same words', 'send');
    void session.submit('other words', 'send');
    const ids = users(session).map((item) => item.send?.id);

    emit({ type: 'messageEnd', messageId: 'u1', message: echo('u1', `${HOOKS}same words`) });
    expect(users(session).map((item) => item.key)).toEqual(['m:u1', `p:${ids[1]}`, `p:${ids[2]}`]);

    emit({ type: 'messageEnd', messageId: 'u2', message: echo('u2', 'same words') });
    emit({ type: 'messageEnd', messageId: 'u3', message: echo('u3', 'other words') });
    expect(users(session).map((item) => item.key)).toEqual(['m:u1', 'm:u2', 'm:u3']);
    expect(users(session).map((item) => item.send?.id)).toEqual(ids);
  });

  it('does not take a repeated delivery of the same echo for the next waiting message', () => {
    const { session, emit } = setup(() => new Promise(() => {}));
    void session.submit('same words', 'send');
    void session.submit('same words', 'send');

    emit({ type: 'messageEnd', messageId: 'u1', message: echo('u1', 'same words') });
    emit({ type: 'messageEnd', messageId: 'u1', message: echo('u1', 'same words') });

    expect(users(session).map((item) => item.send?.status)).toEqual(['sent', 'sending']);
  });

  it('leaves a message a hook sent, and words nobody is waiting for, as ordinary messages', () => {
    const { session, emit } = setup(() => new Promise(() => {}));
    void session.submit('mine', 'send');

    emit({ type: 'messageEnd', messageId: 'h1', message: echo('h1', '<harness-hook-followup name="Check">Run the tests.</harness-hook-followup>') });
    emit({ type: 'messageEnd', messageId: 'x1', message: echo('x1', 'typed in another window') });

    expect(users(session).map((item) => item.send?.status)).toEqual(['sending', undefined, undefined]);
  });

  it('keeps the message while the echo is late, and still takes the echo over once it comes', async () => {
    const { session, emit } = setup();

    expect(await session.submit('slow harness', 'send')).toBe(true);
    expect(users(session)[0].send?.status).toBe('sending');

    emit({ type: 'messageEnd', messageId: 'u1', message: echo('u1', 'slow harness') });
    expect(users(session)).toHaveLength(1);
    expect(users(session)[0].key).toBe('m:u1');
  });

  it('is taken back out when the harness refuses it, and the toast says why', async () => {
    const { session, notifications } = setup((action) => {
      if (action.type === 'prompt') {
        throw { code: 'internal', message: 'agent is busy' };
      }

      return { type: 'ack' };
    });

    expect(await session.submit('refused', 'send')).toBe(false);

    expect(users(session)).toHaveLength(0);
    expect(notifications.items).toHaveLength(1);
  });

  it('is not shown for steering or a follow-up, which wait in the queue above the composer', async () => {
    const { session, actions, emit } = setup();
    emit({ type: 'agentStart' });

    await session.submit('change course', 'steer');
    await session.submit('and later', 'followUp');

    expect(actions.map((action) => action.type)).toContain('steer');
    expect(actions.map((action) => action.type)).toContain('followUp');
    expect(users(session)).toHaveLength(0);
  });

  it('is not shown for a slash command, which the harness may run without making a message of it', async () => {
    const { session, actions } = setup();

    await session.submit('/skill:review the diff', 'send');

    expect(actions.some((action) => action.type === 'prompt')).toBe(true);
    expect(users(session)).toHaveLength(0);
  });

  it('does not stay behind when the chat starts over', () => {
    const { session } = setup(() => new Promise(() => {}));
    void session.submit('about to be lost', 'send');

    session.startOver();

    expect(users(session)).toHaveLength(0);
  });

  it('does not stay behind when the history is read again', async () => {
    const { session } = setup((action) => (action.type === 'getHistory' ? { type: 'history', messages: [], nextCursor: null, total: 0 } : new Promise(() => {})));
    void session.submit('about to be lost', 'send');

    await session.reloadHistory();

    expect(users(session)).toHaveLength(0);
  });

  it('is not shown for a message the agent cannot take because it has ended', async () => {
    const { session, emit } = setup();
    emit({ type: 'exited', code: 0 } as never);

    await session.submit('too late', 'send');

    expect(users(session)).toHaveLength(0);
  });
});

describe('a prompt the harness accepted but never echoed', () => {
  const FAILED = { type: 'promptResult', requestId: null, status: 'error', error: 'model overloaded', retryable: true } as const;
  const image = { id: 'a1', name: 'shot.png', url: 'data:image/png;base64,AAA', data: 'AAA', mimeType: 'image/png' } as Attachment;

  const lastPrompt = (actions: AgentAction[]) => actions.filter((action) => action.type === 'prompt').at(-1);

  function stuck() {
    const harness = setup();
    void harness.session.submit('Fix the login test', 'send');

    return harness;
  }

  it.each([
    ['the prompt fails after it was accepted', FAILED],
    ['the process exits', { type: 'exited', code: 1 }],
    ['its own turn settles without the echo', [{ type: 'agentStart' }, { type: 'settled', aborted: null }]],
  ] as const)('stops waiting when %s, and keeps the words in the transcript', (_name, event) => {
    const { session, emit } = stuck();

    for (const step of Array.isArray(event) ? event : [event]) {
      emit(step as never);
    }

    expect(users(session)).toHaveLength(1);
    expect(wordsOf(users(session)[0])).toBe('Fix the login test');
    expect(users(session)[0].send?.status).not.toBe('sending');
    expect(hasPendingSend(session.chat)).toBe(false);
    expect(session.chat.lastUserText).toBe('Fix the login test');
  });

  it('keeps waiting through a late settle from before its turn, so the echo still takes its place (OMP order)', () => {
    const { session, emit } = stuck();

    emit({ type: 'settled', aborted: null });
    expect(users(session)[0].send?.status).toBe('sending');

    emit({ type: 'agentStart' });
    emit({ type: 'messageStart', messageId: 'u1', role: 'user' });
    emit({ type: 'messageEnd', messageId: 'u1', message: echo('u1', `${HOOKS}Fix the login test`) });

    expect(users(session)).toHaveLength(1);
    expect(users(session)[0].key).toBe('m:u1');
  });

  it('lets Retry send the words of that message again', async () => {
    const { session, emit, actions } = setup();
    await session.submit('Earlier words', 'send');
    emit({ type: 'messageEnd', messageId: 'u0', message: echo('u0', 'Earlier words') });
    emit({ type: 'settled', aborted: null });
    void session.submit('Fix the login test', 'send');

    emit(FAILED);
    await session.retry();

    expect(lastPrompt(actions)).toEqual(expect.objectContaining({ message: 'Fix the login test' }));
  });

  it('shows a later message that starts as its own, not as an empty one waiting for the old words', () => {
    const { session, emit } = stuck();
    emit(FAILED);

    emit({ type: 'messageStart', messageId: 'u9', role: 'user' });

    expect(users(session)).toHaveLength(2);
    expect(users(session)[0].send?.status).not.toBe('sending');
  });

  it('is not taken over by a later message that says the same words', async () => {
    const { session, emit } = stuck();
    emit(FAILED);

    await session.retry();
    emit({ type: 'messageEnd', messageId: 'u1', message: echo('u1', 'Fix the login test') });

    expect(users(session)).toHaveLength(2);
    expect(users(session).map((item) => item.send?.status)).toEqual(['sent', 'sent']);
    expect(users(session)[1].key).toBe('m:u1');
  });

  it('keeps the thumbnails of its images, and does not hand them to the next message with images', () => {
    const { session, emit } = setup(() => new Promise(() => {}));
    void session.submit('Look at this', 'send', { images: [image], files: [] });
    const waiting = users(session)[0];

    emit(FAILED);
    emit({ type: 'messageEnd', messageId: 'u2', message: { id: 'u2', role: 'user', blocks: [{ type: 'image', mimeType: 'image/png' }, { type: 'text', text: 'Other' }] } as unknown as ChatMessage });

    expect(session.images.byKey[waiting.key]).toEqual(['data:image/png;base64,AAA']);
    expect(session.images.byKey['m:u2']).toBeUndefined();
  });
});

describe('a chat that starts over while a prompt waits', () => {
  const image = { id: 'a1', name: 'shot.png', url: 'data:image/png;base64,AAA', data: 'AAA', mimeType: 'image/png' } as Attachment;

  it('leaves nothing waiting and no thumbnail behind', () => {
    const { session } = setup(() => new Promise(() => {}));
    void session.submit('about to be lost', 'send', { images: [image], files: [] });

    session.startOver();

    expect(users(session)).toHaveLength(0);
    expect(hasPendingSend(session.chat)).toBe(false);
    expect(session.images.byKey).toEqual({});
  });

  it('leaves nothing waiting and no thumbnail behind when the history is read again', async () => {
    const { session } = setup((action) => (action.type === 'getHistory' ? { type: 'history', messages: [], nextCursor: null, total: 0 } : new Promise(() => {})));
    void session.submit('about to be lost', 'send', { images: [image], files: [] });

    await session.reloadHistory();

    expect(users(session)).toHaveLength(0);
    expect(hasPendingSend(session.chat)).toBe(false);
    expect(session.images.byKey).toEqual({});
  });
});

describe('streaming guard', () => {
  it('sends no pending bubble while a turn is running', async () => {
    const { session, emit } = setup();
    emit(STREAMING[1]);

    await session.submit('during', 'send');

    expect(users(session)).toHaveLength(0);
  });
});
