import { heldTitle } from '$lib/chat/memory-held';
import { t } from '$lib/i18n/i18n.svelte';
import type { HeldItem } from '$lib/ipc/bindings/HeldItem';
import type { HeldKind } from '$lib/ipc/bindings/HeldKind';
import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

/** What the user did with a waiting save, kept for the chat to say so under the card. */
export interface HeldOutcome {
  id: string;
  what: 'saved' | 'skipped';
  kind: HeldKind;
  title: string;
}

const REMEMBERED_OUTCOMES = 3;

/**
 * The saves agents made while "Ask before saving" was on, waiting for the user, of every chat (Harness Memory keeps them
 * as files on this computer). It listens for changes, so a save an agent holds shows up at once, and answers the three
 * things the user can do with one: save it (as it is, or edited), or skip it.
 */
export class MemoryHeldStore {
  items = $state.raw<HeldItem[]>([]);
  /** Why a save could not be made, by held id: shown in the item, until the user tries again. */
  failures = $state.raw<Record<string, string>>({});
  /** The held ids being saved or skipped now. */
  busy = $state.raw<ReadonlySet<string>>(new Set());
  private outcomes = $state.raw<Record<string, HeldOutcome[]>>({});

  /** The chats whose waiting saves are listed: the open ones. */
  private watched = new Set<string>();
  /** Only the newest read of a chat may apply its result. */
  private reads = new Map<string, number>();
  private counter = 0;

  constructor(private readonly transport: Transport) {}

  /** The waiting saves of a chat, newest first. */
  of(chatKey: string | null): HeldItem[] {
    return chatKey === null ? [] : this.items.filter((item) => item.chatKey === chatKey);
  }

  /** What was saved or skipped in a chat from its card, oldest first. */
  outcomesOf(chatKey: string | null): HeldOutcome[] {
    return chatKey === null ? [] : (this.outcomes[chatKey] ?? []);
  }

  /** Lists the waiting saves of exactly these chats: a chat that opened is read at once, one that closed is forgotten. */
  follow(chatKeys: readonly string[]): void {
    const next = new Set(chatKeys);
    const opened = [...next].filter((key) => !this.watched.has(key));
    const closed = [...this.watched].filter((key) => !next.has(key));

    this.watched = next;

    // The read counters stay (they only rise), so a read that was under way when the chat closed cannot apply after it reopened.
    if (closed.length > 0) {
      this.items = this.items.filter((item) => next.has(item.chatKey));
    }

    void this.load(opened);
  }

  /** Reads the waiting saves of these chats (the followed ones when none are named). */
  async load(chatKeys: readonly string[] = [...this.watched]): Promise<void> {
    await Promise.all(chatKeys.map((key) => this.loadChat(key)));
  }

  private async loadChat(chatKey: string): Promise<void> {
    const mine = (this.reads.get(chatKey) ?? 0) + 1;
    this.reads.set(chatKey, mine);

    try {
      const items = await this.transport.invoke('memory_held_list', { chatKey });

      if (this.reads.get(chatKey) === mine && this.watched.has(chatKey)) {
        this.items = [...this.items.filter((item) => item.chatKey !== chatKey), ...items];
      }
    } catch {
      // Harness Memory missing or too old: nothing is waiting. The Memory button is hidden then anyway.
    }
  }

  /** Starts hearing about changes (what is waiting in the followed chats is read at once). Returns the function that stops listening; the backend keeps one listener, so a later start takes over. */
  start(): () => void {
    let listening = true;

    void this.load();
    this.transport
      .invoke('memory_held_listen', {
        onEvent: () => {
          if (listening) {
            void this.load();
          }
        },
      })
      .catch(() => {});

    return () => {
      listening = false;
    };
  }

  /**
   * Saves a waiting save as the agent sent it, or with the user's `edited` draft. True when it was saved. `revision` is the version the
   * user saw (the item's own unless an editor was opened on an earlier one): the backend saves only that.
   */
  async save(item: HeldItem, edited: JsonValue | null = null, revision: string | null = item.revision): Promise<boolean> {
    return this.act(item, 'saved', () => this.transport.invoke('memory_held_save', { heldId: item.heldId, edited, revision }));
  }

  /** Drops a waiting save without saving anything, when it is still the version the user saw. True when it was dropped. */
  skip(item: HeldItem, revision: string | null = item.revision): Promise<boolean> {
    return this.act(item, 'skipped', () => this.transport.invoke('memory_held_skip', { heldId: item.heldId, revision }));
  }

  /** Says, in the item, that the agent changed it after the user saw it. */
  reportChanged(heldId: string): void {
    this.failures = { ...this.failures, [heldId]: t('memory.guards.waiting.changed') };
  }

  private async act(item: HeldItem, what: HeldOutcome['what'], run: () => Promise<unknown>): Promise<boolean> {
    this.mark(item.heldId, true);
    this.forgetFailure(item.heldId);

    try {
      await run();
      this.remember(item, what);
      this.items = this.items.filter((other) => other.heldId !== item.heldId);

      return true;
    } catch (error) {
      this.explain(item, error);

      return false;
    } finally {
      this.mark(item.heldId, false);

      if (this.watched.has(item.chatKey)) {
        void this.load([item.chatKey]);
      }
    }
  }

  /**
   * Another window or the terminal handled it first: there is nothing to say, the list is read again. A deleted note stays held, with the
   * reason. A save the agent changed after the user saw it stays too, with the newer version read in, to review again.
   */
  private explain(item: HeldItem, error: unknown): void {
    const failure = IpcFailure.from(error);

    if (failure.code === 'notFound') {
      this.items = this.items.filter((other) => other.heldId !== item.heldId);

      return;
    }

    const reason = failure.code === 'conflict' ? t('memory.guards.waiting.gone') : failure.code === 'changed' ? t('memory.guards.waiting.changed') : failure.message;

    this.failures = { ...this.failures, [item.heldId]: reason };
  }

  private remember(item: HeldItem, what: HeldOutcome['what']): void {
    this.counter += 1;
    const outcome: HeldOutcome = { id: `${item.heldId}-${this.counter}`, what, kind: item.kind, title: heldTitle(item) };

    this.outcomes = { ...this.outcomes, [item.chatKey]: [...(this.outcomes[item.chatKey] ?? []), outcome].slice(-REMEMBERED_OUTCOMES) };
  }

  private forgetFailure(heldId: string): void {
    if (heldId in this.failures) {
      const { [heldId]: _gone, ...rest } = this.failures;

      this.failures = rest;
    }
  }

  private mark(heldId: string, busy: boolean): void {
    const next = new Set(this.busy);

    if (busy) {
      next.add(heldId);
    } else {
      next.delete(heldId);
    }

    this.busy = next;
  }
}

export const memoryHeld = new MemoryHeldStore(ipc);
