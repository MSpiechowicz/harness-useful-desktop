import { describe, expect, it, vi } from 'vitest';

import type { ChatAlertEvent } from '$lib/ipc/bindings/ChatAlertEvent';
import type { ChatAlertsView } from '$lib/ipc/bindings/ChatAlertsView';
import type { ViewId } from '$lib/stores/nav.svelte';
import { ToastsStore } from '$lib/stores/toasts.svelte';

import { type AlertChats, type AlertScreen, ChatAlerts } from './alerts.svelte';

/** Chats the test can open and name, standing in for the tab list. */
class FakeChats implements AlertChats {
  sessions: { key: string }[] = [];
  starting: { key: string }[] = [];
  activeKey: string | null = null;
  names: Record<string, string> = {};
  /** Chats titled only by the opening words of their first message: the title is on screen, but it is no name. */
  openingWords: Record<string, string> = {};

  title(key: string): string {
    return this.names[key] ?? this.openingWords[key] ?? 'Unnamed';
  }

  givenName(key: string): string | null {
    return this.names[key] ?? null;
  }

  select(key: string): void {
    this.activeKey = key;
  }

  open(key: string, name: string): void {
    this.sessions.push({ key });
    this.names[key] = name;
  }

  openUnnamed(key: string, openingWords: string): void {
    this.sessions.push({ key });
    this.openingWords[key] = openingWords;
  }
}

class FakeScreen implements AlertScreen {
  current: ViewId = 'chat';

  go(view: ViewId): void {
    this.current = view;
  }
}

/** A backend that listens, remembers the channel, and records every view it is told. */
function setup(options: { failing?: boolean } = {}) {
  const chats = new FakeChats();
  const screen = new FakeScreen();
  const toasts = new ToastsStore();
  const views: ChatAlertsView[] = [];
  const listens: ((event: ChatAlertEvent) => void)[] = [];

  const invoke = vi.fn(async (name: string, args: { channel?: (event: ChatAlertEvent) => void; view?: ChatAlertsView }) => {
    if (name === 'chat_alerts_listen') {
      listens.push(args.channel!);

      return null;
    }

    if (options.failing) {
      throw new Error('backend gone');
    }

    views.push(args.view!);

    return null;
  });
  const alerts = new ChatAlerts({ invoke } as never, chats, screen, toasts);
  const send = (event: ChatAlertEvent) => listens[0](event);
  const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

  return { alerts, chats, screen, toasts, views, listens, send, settle, invoke };
}

describe('the toast of an alert', () => {
  it('names the chat and takes its tone and stay from the kind', () => {
    const { alerts, chats, toasts, send } = setup();
    chats.open('a', 'Swift Otter');
    chats.open('b', 'Bold Lynx');
    chats.open('c', 'Calm Heron');
    alerts.start();

    send({ type: 'toast', key: 'a', kind: 'finished' });
    send({ type: 'toast', key: 'b', kind: 'needsYou' });
    send({ type: 'toast', key: 'c', kind: 'failed' });

    const shown = toasts.items.map(({ title, description, tone, duration }) => ({ title, description, tone, stays: duration === 0 }));

    expect(shown).toEqual([
      { title: 'Chat finished', description: 'Swift Otter', tone: 'success', stays: false },
      { title: 'Chat needs you', description: 'Bold Lynx', tone: 'warning', stays: true },
      { title: 'Chat failed', description: 'Calm Heron', tone: 'danger', stays: true },
    ]);
  });

  it('opens its chat in the Chat view from the Open action, and goes away', () => {
    const { alerts, chats, screen, toasts, send } = setup();
    chats.open('a', 'Swift Otter');
    chats.open('b', 'Bold Lynx');
    chats.activeKey = 'b';
    screen.current = 'settings';
    alerts.start();

    send({ type: 'toast', key: 'a', kind: 'finished' });
    const toast = toasts.items[0];

    expect(toast.action?.label).toBe('Open');
    toast.action?.run();

    expect(chats.activeKey).toBe('a');
    expect(screen.current).toBe('chat');
  });

  it('is replaced by the next alert of the same chat, and leaves other chats alone', () => {
    const { alerts, chats, toasts, send } = setup();
    chats.open('a', 'Swift Otter');
    chats.open('b', 'Bold Lynx');
    alerts.start();

    send({ type: 'toast', key: 'a', kind: 'needsYou' });
    send({ type: 'toast', key: 'b', kind: 'finished' });
    send({ type: 'toast', key: 'a', kind: 'finished' });

    expect(toasts.items.map((toast) => [toast.description, toast.title, toast.duration === 0])).toEqual([
      ['Bold Lynx', 'Chat finished', false],
      ['Swift Otter', 'Chat finished', false],
    ]);
  });

  it('is dismissed when its chat comes on screen, and only then', () => {
    const { alerts, chats, toasts, send } = setup();
    chats.open('a', 'Swift Otter');
    chats.open('b', 'Bold Lynx');
    alerts.start();
    send({ type: 'toast', key: 'a', kind: 'failed' });
    send({ type: 'toast', key: 'b', kind: 'failed' });

    alerts.follow({ shown: null, titles: {} });
    expect(toasts.items).toHaveLength(2);

    alerts.follow({ shown: 'a', titles: {} });
    expect(toasts.items.map((toast) => toast.description)).toEqual(['Bold Lynx']);
  });

  it('is not shown for the chat the user is reading', () => {
    const { alerts, chats, screen, toasts, send } = setup();
    chats.open('a', 'Swift Otter');
    chats.open('b', 'Bold Lynx');
    chats.activeKey = 'a';
    alerts.start();

    send({ type: 'toast', key: 'a', kind: 'needsYou' });
    expect(toasts.items).toEqual([]);

    send({ type: 'toast', key: 'b', kind: 'needsYou' });
    expect(toasts.items.map((toast) => toast.description)).toEqual(['Bold Lynx']);

    screen.current = 'settings';
    send({ type: 'toast', key: 'a', kind: 'failed' });
    expect(toasts.items.map((toast) => toast.description)).toEqual(['Bold Lynx', 'Swift Otter']);
  });

  it('is dismissed when its chat is closed, and the other chats keep theirs', () => {
    const { alerts, chats, toasts, send } = setup();
    chats.open('a', 'Swift Otter');
    chats.open('b', 'Bold Lynx');
    alerts.start();
    send({ type: 'toast', key: 'a', kind: 'failed' });
    send({ type: 'toast', key: 'b', kind: 'needsYou' });

    chats.sessions = chats.sessions.filter((session) => session.key !== 'a');
    alerts.follow(alerts.view());

    expect(toasts.items.map((toast) => toast.description)).toEqual(['Bold Lynx']);
  });

  it('gives a later alert a fresh toast once the earlier one was dismissed', () => {
    const { alerts, chats, toasts, send } = setup();
    chats.open('a', 'Swift Otter');
    alerts.start();
    send({ type: 'toast', key: 'a', kind: 'failed' });
    toasts.dismiss(toasts.items[0].id);

    send({ type: 'toast', key: 'a', kind: 'finished' });

    expect(toasts.items.map((toast) => toast.title)).toEqual(['Chat finished']);
  });
});

describe('opening a chat on request', () => {
  it('shows an existing chat, from wherever the user is', () => {
    const { alerts, chats, screen, send } = setup();
    chats.open('a', 'Swift Otter');
    screen.current = 'routines';
    alerts.start();

    send({ type: 'open', key: 'a' });

    expect(chats.activeKey).toBe('a');
    expect(screen.current).toBe('chat');
  });

  it('does nothing for a chat that is not open', () => {
    const { alerts, chats, screen, send } = setup();
    chats.open('a', 'Swift Otter');
    chats.activeKey = 'a';
    screen.current = 'routines';
    alerts.start();

    send({ type: 'open', key: 'gone' });

    expect(chats.activeKey).toBe('a');
    expect(screen.current).toBe('routines');
  });

  it('does nothing from the Open action of a toast whose chat was closed meanwhile', () => {
    const { alerts, chats, screen, toasts, send } = setup();
    chats.open('a', 'Swift Otter');
    screen.current = 'changes';
    alerts.start();
    send({ type: 'toast', key: 'a', kind: 'finished' });
    chats.sessions = [];

    toasts.items[0].action?.run();

    expect(screen.current).toBe('changes');
  });
});

describe('the view told to the backend', () => {
  it('names the active chat in the Chat view and no chat in any other view', () => {
    const { alerts, chats, screen } = setup();
    chats.open('a', 'Swift Otter');
    chats.open('b', 'Bold Lynx');
    chats.activeKey = 'b';

    expect(alerts.view()).toEqual({ shown: 'b', titles: { a: 'Swift Otter', b: 'Bold Lynx' } });

    for (const other of ['settings', 'changes', 'sessions'] as const) {
      screen.current = other;

      expect(alerts.view().shown).toBeNull();
    }
  });

  it('leaves out chats that are still starting, and shows none while one is the active tab', () => {
    const { alerts, chats } = setup();
    chats.open('a', 'Swift Otter');
    chats.sessions.push({ key: 'starting-1' });
    chats.starting.push({ key: 'starting-1' });
    chats.activeKey = 'starting-1';

    expect(alerts.view()).toEqual({ shown: null, titles: { a: 'Swift Otter' } });
  });

  it('leaves out a chat titled only by the opening words of its first message, so the notification never shows them', () => {
    const { alerts, chats } = setup();
    chats.open('a', 'Swift Otter');
    chats.openUnnamed('b', 'Fix the login bug for the customer acme');

    expect(alerts.view().titles).toEqual({ a: 'Swift Otter' });

    chats.names.b = 'Login fix';

    expect(alerts.view().titles).toEqual({ a: 'Swift Otter', b: 'Login fix' });
  });

  it('still shows the full title in the toast of a chat without a name', () => {
    const { alerts, chats, toasts, send } = setup();
    chats.openUnnamed('b', 'Fix the login bug for the customer acme');
    alerts.start();

    send({ type: 'toast', key: 'b', kind: 'finished' });

    expect(toasts.items.map((toast) => toast.description)).toEqual(['Fix the login bug for the customer acme']);
  });

  it('follows a rename', () => {
    const { alerts, chats } = setup();
    chats.open('a', 'Swift Otter');

    chats.names.a = 'Fix the build';

    expect(alerts.view().titles).toEqual({ a: 'Fix the build' });
  });

  it('is sent once the backend listens, and again only when it changed', async () => {
    const { alerts, chats, screen, views, settle } = setup();
    chats.open('a', 'Swift Otter');
    chats.activeKey = 'a';

    alerts.start();
    alerts.follow(alerts.view());
    await settle();

    expect(views).toEqual([{ shown: 'a', titles: { a: 'Swift Otter' } }]);

    alerts.follow(alerts.view());
    alerts.follow(alerts.view());
    await settle();
    expect(views).toHaveLength(1);

    screen.current = 'settings';
    alerts.follow(alerts.view());
    alerts.follow(alerts.view());
    await settle();

    expect(views).toEqual([
      { shown: 'a', titles: { a: 'Swift Otter' } },
      { shown: null, titles: { a: 'Swift Otter' } },
    ]);
  });

  it('is held back until the backend listens, then goes out as it is by then', async () => {
    const { alerts, chats, views, settle } = setup();
    chats.open('a', 'Swift Otter');

    alerts.follow({ shown: null, titles: { early: 'Too early' } });
    expect(views).toEqual([]);

    alerts.start();
    chats.names.a = 'Renamed';
    await settle();

    expect(views).toEqual([{ shown: null, titles: { a: 'Renamed' } }]);
  });

  it('does not fail the user when the backend refuses it, and sends it again on the next change', async () => {
    const { alerts, chats, settle, invoke } = setup({ failing: true });
    chats.open('a', 'Swift Otter');

    alerts.start();
    await settle();
    alerts.follow(alerts.view());
    await settle();

    expect(invoke.mock.calls.filter(([name]) => name === 'chat_alerts_view').length).toBeGreaterThan(1);
  });
});

describe('listening', () => {
  it('starts once however often the shell asks', async () => {
    const { alerts, listens, settle } = setup();

    alerts.start();
    alerts.start();
    await settle();
    alerts.start();

    expect(listens).toHaveLength(1);
  });
});
