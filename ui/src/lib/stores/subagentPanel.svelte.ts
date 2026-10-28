import { defaultPanelTarget } from '$lib/chat/subagents';
import type { Subagent } from '$lib/ipc/bindings/Subagent';

export type PanelTab = 'activity' | 'brief' | 'output';

interface PanelView {
  id: string;
  tab: PanelTab;
}

/** What the chat marks for a moment after "Jump to it": the spawning tool card and the subagent's row. */
export interface Flash {
  key: string;
  toolCallId: string;
  subagentId: string;
}

/** A request to scroll the chat to a tool call. `nonce` changes on every request, so asking twice for the same card still scrolls. */
export interface JumpRequest {
  key: string;
  toolCallId: string;
  nonce: number;
}

/** How long the flash stays; `--flash-duration` in motion.css is the same length. */
export const FLASH_MS = 1600;

/**
 * Which subagent each chat's panel shows, and on which tab. The panel is a view of the chat, not part of the transcript,
 * so it lives beside the sessions, keyed by session key: each chat remembers its own while it is open.
 */
export class SubagentPanelStore {
  private views = $state<Record<string, PanelView | undefined>>({});
  private nonce = 0;
  private flashTimer: ReturnType<typeof setTimeout> | undefined;

  flash = $state<Flash | null>(null);
  jump = $state<JumpRequest | null>(null);

  /** The subagent the chat's panel shows, or null when it is closed. */
  selectedId(key: string): string | null {
    return this.views[key]?.id ?? null;
  }

  tab(key: string): PanelTab {
    return this.views[key]?.tab ?? 'activity';
  }

  /** Shows a subagent; another one than the panel showed starts on its Activity tab. */
  open(key: string, id: string): void {
    const view = this.views[key];

    this.views = { ...this.views, [key]: { id, tab: view?.id === id ? view.tab : 'activity' } };
  }

  close(key: string): void {
    this.views = { ...this.views, [key]: undefined };
  }

  setTab(key: string, tab: PanelTab): void {
    const view = this.views[key];

    if (view) {
      this.views = { ...this.views, [key]: { ...view, tab } };
    }
  }

  /** The shortcut: closes the panel, or opens it on the first running subagent, else the newest. Nothing to show does nothing. */
  toggle(key: string, subagents: Record<string, Subagent>): void {
    const target = defaultPanelTarget(subagents);

    if (this.selectedId(key) !== null) {
      this.close(key);
    } else if (target !== null) {
      this.open(key, target);
    }
  }

  /** Scrolls the chat to the call that spawned a subagent and marks the card and the row for a moment. */
  jumpTo(key: string, toolCallId: string, subagentId: string): void {
    this.nonce += 1;
    this.jump = { key, toolCallId, nonce: this.nonce };
    this.flash = { key, toolCallId, subagentId };

    clearTimeout(this.flashTimer);
    this.flashTimer = setTimeout(() => (this.flash = null), FLASH_MS);
  }

  /** Whether the card of this call is being flashed in this chat. */
  flashesCall(key: string, toolCallId: string): boolean {
    return this.flash?.key === key && this.flash.toolCallId === toolCallId;
  }

  flashesRow(key: string, subagentId: string): boolean {
    return this.flash?.key === key && this.flash.subagentId === subagentId;
  }
}

export const subagentPanel = new SubagentPanelStore();
