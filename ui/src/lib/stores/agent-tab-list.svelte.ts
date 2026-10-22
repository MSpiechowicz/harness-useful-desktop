import { SvelteMap } from 'svelte/reactivity';

import { generatedName } from '$lib/chat/naming';
import type { ChatSession } from '$lib/chat/session.svelte';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';

import { nextNumber, type StartingChat } from './agent-tabs';

/** The chat tabs: the open sessions, the chats still starting, which one is showing and each one's name. */
export class ChatTabs {
  sessions = $state.raw<ChatSession[]>([]);
  /** New chats whose agent is still starting (it can take a few seconds). */
  starting = $state.raw<StartingChat[]>([]);
  activeKey = $state<string | null>(null);

  active: ChatSession | null = $derived(this.sessions.find((session) => session.key === this.activeKey) ?? null);
  activeStarting: StartingChat | null = $derived(this.starting.find((chat) => chat.key === this.activeKey) ?? null);

  private startingCount = 0;
  /** Each open chat's number, the "3" of "Chat 3" (a random free one), kept while the agent has not named it. */
  protected numbers = new SvelteMap<string, number>();

  /**
   * A chat's title: the name the user typed, else the helper's, else the agent's own, else the opening words of the
   * first message, else a generated creature name.
   */
  title(key: string): string {
    const session = this.get(key);
    const generated = generatedName(this.numbers.get(key) ?? 1);
    if (!session) {
      return generated;
    }

    return (session.userNamed ? session.name : null) ?? session.aiLabel?.name ?? session.name ?? session.label?.name ?? generated;
  }

  /**
   * What a chat's tooltip says: a name cut short by the row shows the message it came from in full rather than the
   * same words twice; otherwise the name, and below it what the chat is about.
   */
  tip(key: string): { text: string; detail: string } {
    const title = this.title(key);
    const description = this.description(key);
    const stem = title.replace(/…$/, '');

    return description.startsWith(stem) ? { text: description, detail: '' } : { text: title, detail: description };
  }

  /** The line under a chat's title on its card; empty when there is nothing to add to the title. */
  description(key: string): string {
    return this.get(key)?.label?.description ?? '';
  }

  get(key: string): ChatSession | undefined {
    return this.sessions.find((session) => session.key === key);
  }

  /** A new starting tab for a chat of `harness` in `project`, with a number for its placeholder name (not shown yet). */
  protected slot(harness: HarnessId, project: string): StartingChat {
    this.startingCount += 1;
    const slot = { key: `starting-${this.startingCount}`, harness, project };
    this.numbers.set(slot.key, nextNumber(this.numbers.values()));

    return slot;
  }

  /** Moves the number reserved under `from` to session `to` (or frees it when the start failed). */
  protected handOverNumber(from: string, to: string | null): void {
    const number = this.numbers.get(from);
    this.numbers.delete(from);
    if (to !== null && number !== undefined) {
      this.numbers.set(to, number);
    }
  }

  /** Shows the next (`1`) or previous (`-1`) tab, wrapping around; starting chats count as tabs. */
  cycle(step: 1 | -1): void {
    const keys = [...this.sessions.map((session) => session.key), ...this.starting.map((chat) => chat.key)];
    if (keys.length === 0) {
      return;
    }

    const current = this.activeKey === null ? -1 : keys.indexOf(this.activeKey);
    this.activeKey = keys[(current + step + keys.length) % keys.length];
  }

  select(key: string): void {
    if (this.get(key) || this.starting.some((chat) => chat.key === key)) {
      this.activeKey = key;
    }
  }

  protected moveTo(key: string, index: number): void {
    const rest = this.sessions.filter((session) => session.key !== key);
    const moved = this.sessions.find((session) => session.key === key);
    if (moved) {
      this.sessions = [...rest.slice(0, index), moved, ...rest.slice(index)];
    }
  }
}
