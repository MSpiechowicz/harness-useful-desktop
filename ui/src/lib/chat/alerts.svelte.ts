import { t } from '$lib/i18n/i18n.svelte';
import type { ChatAlertEvent } from '$lib/ipc/bindings/ChatAlertEvent';
import type { ChatAlertKind } from '$lib/ipc/bindings/ChatAlertKind';
import type { ChatAlertsView } from '$lib/ipc/bindings/ChatAlertsView';
import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { nav, type ViewId } from '$lib/stores/nav.svelte';
import { type ToastInput, type ToastsStore, toasts } from '$lib/stores/toasts.svelte';

/** The open chats, as far as alerts need them. */
export interface AlertChats {
  sessions: readonly { key: string }[];
  /** Chats whose agent is still starting: they are tabs, but no alert is ever about them. */
  starting: readonly { key: string }[];
  activeKey: string | null;
  title(key: string): string;
  /** The name a person or the naming helper gave the chat, or null when its title is only the opening words of its first message or a placeholder. */
  givenName(key: string): string | null;
  select(key: string): void;
}

/** The view on screen. */
export interface AlertScreen {
  current: ViewId;
  go(view: ViewId): void;
}

/** The toast one alert shows: finished is a passing note, the other two stay until the user has seen them. */
function toastFor(kind: ChatAlertKind, title: string, open: () => void): ToastInput {
  const action = { label: t('chat.alert.open'), run: open };

  switch (kind) {
    case 'finished':
      return { title: t('chat.alert.finished'), description: title, tone: 'success', action };
    case 'needsYou':
      return { title: t('chat.alert.needsYou'), description: title, tone: 'warning', duration: 0, action };
    case 'failed':
      return { title: t('chat.alert.failed'), description: title, tone: 'danger', duration: 0, action };
  }
}

/**
 * Chat alerts, the page's half. The backend decides which chat deserves an alert and delivers the OS notification and the chime. This
 * side shows the toast while the window is in front, opens a chat when asked, and tells the backend which chat is on screen and what
 * every chat is called, so it can decide.
 */
export class ChatAlerts {
  private started = false;
  /** Whether the backend listens: until then a view sent to it would be lost, and the first one is sent when it does. */
  private ready = false;
  /** The view the backend was last told, as text, so an unchanged one is not sent again. */
  private told: string | null = null;
  /** The toast of each chat: a chat has at most one. */
  private toastOf = new Map<string, number>();

  constructor(
    private transport: Pick<Transport, 'invoke'> = ipc,
    private chats: AlertChats = agentSessions,
    private screen: AlertScreen = nav,
    private toastStore: ToastsStore = toasts,
  ) {}

  /**
   * What the backend should know now: the chat on screen (only in the Chat view) and the name of every open chat that has a real one.
   * A chat named only after its first message is left out: the OS notification would show words of the message, maybe on a lock screen.
   */
  view(): ChatAlertsView {
    const titles: Record<string, string> = {};

    for (const { key } of this.chats.sessions) {
      const name = this.isStarting(key) ? null : this.chats.givenName(key);

      if (name !== null) {
        titles[key] = name;
      }
    }

    const active = this.chats.activeKey;
    const shown = this.screen.current === 'chat' && active !== null && !this.isStarting(active) ? active : null;

    return { shown, titles };
  }

  /**
   * Starts listening, once for the life of the page: the backend keeps one listener per page load, so a remounted shell must not ask
   * again. The first view goes out as soon as the backend listens.
   */
  start(): void {
    if (this.started) {
      return;
    }

    this.started = true;

    this.transport
      .invoke('chat_alerts_listen', { channel: (event) => this.handle(event) })
      .catch(() => {
        // Without a listener there are no alerts to show.
      })
      .finally(() => {
        this.ready = true;
        this.send(this.view());
      });
  }

  /**
   * The view changed (or may have): its chat's toast has been seen, the toast of a chat that is no longer open has nothing to open, and
   * the backend learns the new view when it differs.
   */
  follow(view: ChatAlertsView): void {
    if (view.shown !== null) {
      this.dismiss(view.shown);
    }

    for (const key of [...this.toastOf.keys()]) {
      if (!this.chats.sessions.some((session) => session.key === key)) {
        this.dismiss(key);
      }
    }

    if (this.ready) {
      this.send(view);
    }
  }

  /** Opens a chat: its tab, in the Chat view. A chat that is gone is not opened. */
  open(key: string): void {
    if (!this.chats.sessions.some((session) => session.key === key)) {
      return;
    }

    this.chats.select(key);
    this.screen.go('chat');
  }

  private handle(event: ChatAlertEvent): void {
    if (event.type === 'open') {
      this.open(event.key);

      return;
    }

    this.show(event.key, event.kind);
  }

  /** Shows the alert as the chat's one toast: a newer alert takes the place of the older one. A chat on screen needs none. */
  private show(key: string, kind: ChatAlertKind): void {
    this.dismiss(key);

    if (this.view().shown === key) {
      return;
    }

    const id = this.toastStore.push({
      ...toastFor(kind, this.chats.title(key), () => this.open(key)),
      onclose: () => {
        if (this.toastOf.get(key) === id) {
          this.toastOf.delete(key);
        }
      },
    });

    this.toastOf.set(key, id);
  }

  private dismiss(key: string): void {
    const shown = this.toastOf.get(key);

    if (shown !== undefined) {
      this.toastStore.dismiss(shown);
    }
  }

  private send(view: ChatAlertsView): void {
    const text = JSON.stringify(view);

    if (text === this.told) {
      return;
    }

    this.told = text;

    this.transport.invoke('chat_alerts_view', { view }).catch(() => {
      // The backend only decides with it, so the next change sends it again. Nothing for the user to do.
      if (this.told === text) {
        this.told = null;
      }
    });
  }

  private isStarting(key: string): boolean {
    return this.chats.starting.some((chat) => chat.key === key);
  }
}

export const chatAlerts = new ChatAlerts();
