import { heldSummary, heldTitle } from '$lib/chat/memory-held';
import { t } from '$lib/i18n/i18n.svelte';
import type { MessageKey } from '$lib/i18n/locales/en';
import type { HeldItem } from '$lib/ipc/bindings/HeldItem';
import type { HeldSaved } from '$lib/ipc/bindings/HeldSaved';
import type { MemorySavePlace } from '$lib/ipc/bindings/MemorySavePlace';
import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { toasts, type ToastsStore } from './toasts.svelte';

/** A waiting save that was saved, whether the remote was unreachable so the change waits on this computer, and where a new entry went when the answer says. */
interface SavedItem {
  item: HeldItem;
  queued: boolean;
  place: MemorySavePlace | null;
}

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

  /** The chats whose waiting saves are listed: the open ones. */
  private watched = new Set<string>();
  /** Only the newest read of a chat may apply its result. */
  private reads = new Map<string, number>();

  constructor(
    private readonly transport: Transport,
    private readonly notifications: Pick<ToastsStore, 'push'>,
  ) {}

  /** The waiting saves of a chat, newest first. */
  of(chatKey: string | null): HeldItem[] {
    return chatKey === null ? [] : this.items.filter((item) => item.chatKey === chatKey);
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
   * user saw (the item's own unless an editor was opened on an earlier one): the backend saves only that. `place` is sent only while the card
   * shows its Save to field (a new entry of a scope kept in "Remote + this computer"): left out, the server decides.
   */
  async save(item: HeldItem, edited: JsonValue | null = null, revision: string | null = item.revision, place?: MemorySavePlace): Promise<boolean> {
    const saved = await this.write(item, edited, revision, place);

    if (saved) {
      this.announceSaved([saved]);
    }

    return saved !== null;
  }

  /**
   * Saves these one after the other, so a failure on one does not hide the rest, and says so with one toast for what was saved. `placeOf` says
   * where each one goes: the choice of its own card, or nothing where its card shows no choice.
   */
  async saveAll(items: readonly HeldItem[], placeOf: (item: HeldItem) => MemorySavePlace | undefined = () => undefined): Promise<void> {
    const saved: SavedItem[] = [];

    for (const item of items) {
      const written = await this.write(item, null, item.revision, placeOf(item));

      if (written) {
        saved.push(written);
      }
    }

    this.announceSaved(saved);
  }

  /** Drops a waiting save without saving anything, when it is still the version the user saw. True when it was dropped. */
  async skip(item: HeldItem, revision: string | null = item.revision): Promise<boolean> {
    const skipped = (await this.act(item, () => this.transport.invoke('memory_held_skip', { heldId: item.heldId, revision }))) !== null;

    if (skipped) {
      this.notifications.push({ title: t(item.kind === 'note' ? 'memory.guards.toast.skippedNote' : 'memory.guards.toast.skippedScript'), description: heldTitle(item) });
    }

    return skipped;
  }

  /** Says, in the item, that the agent changed it after the user saw it. */
  reportChanged(heldId: string): void {
    this.failures = { ...this.failures, [heldId]: t('memory.guards.waiting.changed') };
  }

  /** What was saved, or null when it was not. */
  private async write(item: HeldItem, edited: JsonValue | null, revision: string | null, place?: MemorySavePlace): Promise<SavedItem | null> {
    const saved = await this.act(item, () => this.transport.invoke('memory_held_save', { heldId: item.heldId, edited, revision, ...(place === undefined ? {} : { place }) }));

    return saved === null ? null : { item, queued: queuedBy(saved.value), place: placeBy(saved.value) };
  }

  /** Runs what the user did with a waiting save: what it answered, or null when it failed (the reason is kept for the item). */
  private async act<T>(item: HeldItem, run: () => Promise<T>): Promise<{ value: T } | null> {
    this.mark(item.heldId, true);
    this.forgetFailure(item.heldId);

    try {
      const value = await run();
      this.items = this.items.filter((other) => other.heldId !== item.heldId);

      return { value };
    } catch (error) {
      this.explain(item, error);

      return null;
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

  /**
   * One toast for what was saved at once: one item by its title, more by how many. A change kept on this computer while the remote is unreachable
   * is not saved yet: it has its own toast, which says it is applied when the remote is back.
   */
  private announceSaved(saved: readonly SavedItem[]): void {
    const queued = saved.filter((entry) => entry.queued).map((entry) => entry.item);
    const done = saved.filter((entry) => !entry.queued).map((entry) => entry.item);

    if (done.length === 1) {
      const [item] = done;

      this.notifications.push({ title: t(savedTitle(item, saved.find((entry) => entry.item === item)?.place ?? null)), description: heldTitle(item), tone: 'success' });
    } else if (done.length > 1) {
      this.notifications.push({ title: t('memory.guards.toast.savedAll'), description: heldSummary(done), tone: 'success' });
    }

    if (queued.length > 0) {
      this.notifications.push({
        title: t('memory.sync.queued.title'),
        description: queued.length === 1 ? t('memory.sync.queued.body', { title: heldTitle(queued[0]) }) : t('memory.sync.queued.bodyMany', { items: heldSummary(queued) }),
        tone: 'warning',
      });
    }
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

/** Where the answer says a new entry went. Null for an answer that does not say, or an update. */
function placeBy(saved: HeldSaved | null | undefined): MemorySavePlace | null {
  if (!saved || !('place' in saved)) {
    return null;
  }

  return saved.place === 'remote' || saved.place === 'local' ? saved.place : null;
}

/** The toast title of a saved item: it says where it went when the answer tells. */
function savedTitle(item: HeldItem, place: MemorySavePlace | null): MessageKey {
  const note = item.kind === 'note';

  if (place === null) {
    return note ? 'memory.guards.toast.savedNote' : 'memory.guards.toast.savedScript';
  }

  if (note) {
    return place === 'remote' ? 'memory.guards.toast.savedNote.remote' : 'memory.guards.toast.savedNote.local';
  }

  return place === 'remote' ? 'memory.guards.toast.savedScript.remote' : 'memory.guards.toast.savedScript.local';
}

/** Whether the answer says the change to a team note was kept on this computer, to be applied when the remote is back. A script is never queued. */
function queuedBy(saved: HeldSaved | null | undefined): boolean {
  return saved?.kind === 'note' && saved.queued === true;
}

export const memoryHeld = new MemoryHeldStore(ipc, toasts);
