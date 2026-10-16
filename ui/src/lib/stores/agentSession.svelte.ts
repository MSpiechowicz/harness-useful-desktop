import { applyChatDefaults } from '$lib/chat/chat-defaults';
import { activeSubagentCount } from '$lib/chat/subagents';
import { SvelteMap } from 'svelte/reactivity';

import { t } from '$lib/i18n/i18n.svelte';
import { requestAiLabel } from '$lib/chat/ai-name';
import { generatedName, generatedNameCount } from '$lib/chat/naming';
import { ChatSession } from '$lib/chat/session.svelte';
import type { AgentEnvelope } from '$lib/ipc/bindings/AgentEnvelope';
import type { AgentStartRequest } from '$lib/ipc/bindings/AgentStartRequest';
import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
import type { CommandSpec } from '$lib/ipc/bindings/CommandSpec';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { SessionRef } from '$lib/ipc/bindings/SessionRef';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { harness } from './harness.svelte';
import { prefs } from './prefs.svelte';
import { toasts, type ToastsStore } from './toasts.svelte';

/** Options a restart can change; everything else carries over from the session being replaced. */
export interface RestartOptions {
  approvalOverride?: ApprovalMode | null;
  trustOnce?: boolean;
}

/** A chat whose agent is still starting: shown as a tab at once, replaced by the session when the agent is ready. */
export interface StartingChat {
  key: string;
  harness: HarnessId;
  project: string;
}

/** A chat of the last run to reopen: its conversation, and the name it had. */
export interface RestoredChat {
  harness: HarnessId;
  project: string;
  sessionId: string;
  label: ChatSession['aiLabel'];
  userNamed: boolean;
}

/** Every open chat session (the tabs) and which one is showing. */
export class AgentSessionsStore {
  sessions = $state.raw<ChatSession[]>([]);
  /** New chats whose agent is still starting (it can take a few seconds). */
  starting = $state.raw<StartingChat[]>([]);
  activeKey = $state<string | null>(null);

  active: ChatSession | null = $derived(this.sessions.find((session) => session.key === this.activeKey) ?? null);
  activeStarting: StartingChat | null = $derived(this.starting.find((chat) => chat.key === this.activeKey) ?? null);

  private startingCount = 0;
  /** Each open chat's number, the "3" of "Chat 3" (a random free one), kept while the agent has not named it. */
  private numbers = new SvelteMap<string, number>();

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

  /**
   * A placeholder name for a new chat: a random one of the names no open chat has, so a fresh chat is not always the
   * first of the list. Once every name is taken, the lowest free number (a name with "2", "3"...) follows.
   */
  private nextNumber(): number {
    const used = new Set(this.numbers.values());
    const free = Array.from({ length: generatedNameCount() }, (_, index) => index + 1).filter((number) => !used.has(number));
    if (free.length > 0) {
      return free[Math.floor(Math.random() * free.length)];
    }

    let next = 1;
    while (used.has(next)) {
      next += 1;
    }

    return next;
  }

  private catalogs = new Map<HarnessId, Promise<CommandSpec[]>>();

  constructor(
    private readonly transport: Transport,
    private readonly notifications: ToastsStore,
  ) {}

  get(key: string): ChatSession | undefined {
    return this.sessions.find((session) => session.key === key);
  }

  /**
   * Starts an agent. Events can arrive before the command returns, so they wait in `pending` until the session exists.
   * `quiet` drops a failure without a toast (reopening last run's chats: one that can't come back just stays closed).
   */
  async start(request: AgentStartRequest, { activate = true, quiet = false } = {}): Promise<ChatSession | null> {
    // The callers gate on chat support already, this keeps a missed path from launching an agent that cannot run.
    if (!harness.canChat(request.harness)) {
      if (!quiet) {
        const name = harness.get(request.harness)?.info.name ?? request.harness;
        this.notifications.push({ title: t('chat.toast.comingSoon', { harness: name }), description: t('chat.toast.comingSoonBody'), tone: 'info' });
      }

      return null;
    }

    const pending: AgentEnvelope[] = [];
    let session: ChatSession | null = null;
    const onEvent = (envelope: AgentEnvelope): void => (session ? session.ingest(envelope) : void pending.push(envelope));

    try {
      const info = await this.transport.invoke('agent_start', { request, onEvent });
      session = new ChatSession(info, this.transport, this.notifications);
    } catch (error) {
      if (!quiet) {
        this.notifications.push({ title: t('chat.toast.startFailed'), description: IpcFailure.from(error).message, tone: 'danger' });
      }

      return null;
    }

    for (const envelope of pending) {
      session.ingest(envelope);
    }

    if (!request.utility && !request.resume) {
      session.onFirstPrompt = (named) => void this.nameWithAi(named);
    }

    this.sessions = [...this.sessions, session];
    if (!this.numbers.has(session.key)) {
      this.numbers.set(session.key, this.nextNumber());
    }
    if (activate) {
      this.activeKey = session.key;
    }
    void this.loadCatalog(session);

    return session;
  }

  /**
   * With "AI chat names" on, asks a throwaway agent for a short name and description once per chat, right after its
   * first message. Any failure leaves the name made from the first message.
   */
  private async nameWithAi(session: ChatSession): Promise<void> {
    const prompt = session.firstPrompt;
    if (!prefs.value?.aiChatNames || prompt === '' || prompt.startsWith('/') || prompt.startsWith('!')) {
      return;
    }

    const model = session.chat.session?.model ?? null;
    const label = await requestAiLabel(this.transport, {
      harness: session.info.harness,
      project: session.info.project,
      model: model ? { provider: model.provider, id: model.id } : null,
      prompt,
    });

    if (label && this.get(session.key)) {
      session.aiLabel = label;
    }
  }

  /** Starts a chat for the harness and project the user is working in, on the model and thinking level last picked. */
  /**
   * The tab appears at once as a starting chat and turns into the session when the agent is ready; the new chat is
   * shown then only if the user is still looking at its tab.
   */
  async create(harness: HarnessId, project: string): Promise<ChatSession | null> {
    this.startingCount += 1;
    const placeholder: StartingChat = { key: `starting-${this.startingCount}`, harness, project };
    this.numbers.set(placeholder.key, this.nextNumber());
    this.starting = [...this.starting, placeholder];
    this.activeKey = placeholder.key;

    const session = await this.start({ harness, project, resume: null, approvalOverride: null, trustOnce: false, utility: false }, { activate: false });
    this.starting = this.starting.filter((chat) => chat.key !== placeholder.key);
    this.handOverNumber(placeholder.key, session?.key ?? null);

    if (this.activeKey === placeholder.key) {
      this.activeKey = session?.key ?? this.sessions.at(-1)?.key ?? this.starting.at(-1)?.key ?? null;
    }

    if (session) {
      void applyChatDefaults(session);
    }

    return session;
  }

  /**
   * Reopens last run's chats together: each shows as a starting tab at once and turns into its session when its agent
   * is ready, so the chat that was showing appears as soon as it alone is up, not after every agent. Tabs keep their
   * saved order whatever order the agents finish in. Returns how many came back.
   */
  async restore(chats: RestoredChat[], wanted: number): Promise<number> {
    const slots = chats.map((chat): StartingChat => {
      this.startingCount += 1;
      const slot = { key: `starting-${this.startingCount}`, harness: chat.harness, project: chat.project };
      this.numbers.set(slot.key, this.nextNumber());

      return slot;
    });
    if (slots.length === 0) {
      return 0;
    }

    this.starting = [...this.starting, ...slots];
    this.activeKey = (slots[wanted] ?? slots[0]).key;
    const back: number[] = [];

    const sessions = await Promise.all(
      chats.map(async (chat, index) => {
        const slot = slots[index];
        const session = await this.start(
          { harness: chat.harness, project: chat.project, resume: { harness: chat.harness, id: chat.sessionId }, approvalOverride: null, trustOnce: false, utility: false },
          { activate: false, quiet: true },
        );
        this.starting = this.starting.filter((other) => other.key !== slot.key);
        this.handOverNumber(slot.key, session?.key ?? null);

        if (session) {
          session.aiLabel = chat.label;
          session.userNamed = chat.userNamed;
          back.push(index);
          this.moveTo(session.key, back.filter((other) => other < index).length);
          void session.loadHistory();
        }

        if (this.activeKey === slot.key) {
          this.activeKey = session?.key ?? this.sessions.at(0)?.key ?? this.starting.at(0)?.key ?? null;
        }

        return session;
      }),
    );

    return sessions.filter(Boolean).length;
  }

  /** Closes a tab and stops its agent. */
  async stop(key: string): Promise<void> {
    const index = this.sessions.findIndex((session) => session.key === key);
    if (index === -1) {
      return;
    }

    this.sessions = this.sessions.filter((session) => session.key !== key);
    this.numbers.delete(key);
    if (this.activeKey === key) {
      this.activeKey = this.sessions[Math.min(index, this.sessions.length - 1)]?.key ?? this.starting.at(-1)?.key ?? null;
    }

    try {
      await this.transport.invoke('agent_stop', { key });
    } catch (error) {
      this.notifications.push({ title: t('chat.toast.stopFailed'), description: IpcFailure.from(error).message, tone: 'warning' });
    }
  }

  /**
   * Replaces a session with a fresh agent that resumes the same conversation, applying new launch
   * options (a different approval mode, "trust this project once"). The tab keeps its place and draft.
   */
  async restart(key: string, options: RestartOptions): Promise<ChatSession | null> {
    const old = this.get(key);
    if (!old) {
      return null;
    }

    const resume = resumable(old);
    const draft = old.draft;
    const index = this.sessions.indexOf(old);
    const number = this.numbers.get(key);

    await this.stop(key);
    if (number !== undefined) {
      // Reserved under a temporary key so the replacement keeps the tab's number.
      this.numbers.set(`restart-${key}`, number);
    }

    const fresh = await this.start({
      harness: old.info.harness,
      project: old.info.project,
      resume,
      approvalOverride: options.approvalOverride === undefined ? old.info.approvalOverride : options.approvalOverride,
      trustOnce: options.trustOnce ?? false,
      utility: false,
    });

    this.handOverNumber(`restart-${key}`, fresh?.key ?? null);
    if (!fresh) {
      return null;
    }

    fresh.draft = draft;
    fresh.aiLabel = old.aiLabel;
    fresh.userNamed = old.userNamed;
    this.moveTo(fresh.key, index);
    if (resume) {
      void fresh.loadHistory();
    }

    return fresh;
  }

  /**
   * Restarts the open chats of a harness that are not busy, so they load what changed (an updated or removed plugin);
   * each resumes its conversation in the same tab. A chat that is answering or has helpers running is left alone.
   * Returns how many were restarted.
   */
  async reloadIdle(harnessId: HarnessId): Promise<number> {
    const idle = this.sessions.filter((session) => session.info.harness === harnessId && !session.ended && !session.streaming && activeSubagentCount(session.chat) === 0);

    for (const session of idle) {
      await this.restart(session.key, {});
    }

    return idle.length;
  }

  /**
   * Hands a chat to another harness in its own tab. The agent cannot change harness mid-conversation, so a fresh one
   * starts in the same project while the tab shows the new harness and a spinner; when it is ready it takes the tab's
   * place, number, draft and any name the user gave it. The old conversation stays in Sessions. A failed start leaves
   * the chat as it was.
   */
  async changeHarness(key: string, harnessId: HarnessId): Promise<ChatSession | null> {
    const old = this.get(key);
    if (!old || old.switchingTo || old.info.harness === harnessId) {
      return null;
    }

    old.switchingTo = harnessId;
    const fresh = await this.start({ harness: harnessId, project: old.info.project, resume: null, approvalOverride: null, trustOnce: false, utility: false }, { activate: false });
    if (!fresh) {
      old.switchingTo = null;

      return null;
    }

    fresh.draft = old.draft;
    if (old.userNamed) {
      fresh.aiLabel = old.aiLabel;
      fresh.userNamed = true;
    }
    void applyChatDefaults(fresh);
    await this.takeOver(old.key, fresh.key);

    return fresh;
  }

  /**
   * Puts session `freshKey` in the place of `oldKey` (same tab position and number, shown) and stops the old one;
   * how a resumed or forked session takes over an empty chat instead of opening another tab.
   */
  async takeOver(oldKey: string, freshKey: string): Promise<void> {
    const index = this.sessions.findIndex((session) => session.key === oldKey);
    if (index === -1 || !this.get(freshKey)) {
      return;
    }

    this.handOverNumber(oldKey, freshKey);
    this.moveTo(freshKey, index);
    this.activeKey = freshKey;
    await this.stop(oldKey);
  }

  /** Moves the number reserved under `from` to session `to` (or frees it when the start failed). */
  private handOverNumber(from: string, to: string | null): void {
    const number = this.numbers.get(from);
    this.numbers.delete(from);
    if (to !== null && number !== undefined) {
      this.numbers.set(to, number);
    }
  }

  /**
   * Moves chat `key`, conversation included, to `project` (OMP `/move`): the tab, its messages and its draft stay,
   * only the project changes. Returns false, with a notification, when the harness refuses (PI, a busy agent).
   */
  async moveProject(key: string, project: string): Promise<boolean> {
    const session = this.get(key);
    if (!session) {
      return false;
    }

    try {
      session.info = await this.transport.invoke('agent_move_project', { key, project });

      return true;
    } catch (error) {
      this.notifications.push({ title: t('chat.toast.moveFailed'), description: IpcFailure.from(error).message, tone: 'warning' });

      return false;
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

  private moveTo(key: string, index: number): void {
    const rest = this.sessions.filter((session) => session.key !== key);
    const moved = this.sessions.find((session) => session.key === key);
    if (moved) {
      this.sessions = [...rest.slice(0, index), moved, ...rest.slice(index)];
    }
  }

  /** The static slash-command catalog of a harness, fetched once and shared by its sessions. */
  private async loadCatalog(session: ChatSession): Promise<void> {
    const harness = session.info.harness;
    let catalog = this.catalogs.get(harness);

    if (!catalog) {
      catalog = this.transport.invoke('commands_catalog', { harness }).catch(() => []);
      this.catalogs.set(harness, catalog);
    }

    session.catalog = await catalog;
  }
}

export const agentSessions = new AgentSessionsStore(ipc, toasts);

/**
 * The session to resume for `session`'s conversation, or null while it has none: the agent writes its session file
 * with the first message, so an untouched chat (notices from extensions don't count) has nothing to resume.
 */
export function resumable(session: ChatSession): SessionRef | null {
  const sessionId = session.chat.session?.sessionId;
  const conversation = session.chat.items.some((item) => item.kind === 'message');

  return sessionId && conversation ? { harness: session.info.harness, id: sessionId } : null;
}
