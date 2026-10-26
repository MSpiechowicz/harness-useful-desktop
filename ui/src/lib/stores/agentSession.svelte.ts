import { applyChatDefaults } from '$lib/chat/chat-defaults';
import { activeSubagentCount } from '$lib/chat/subagents';

import { t } from '$lib/i18n/i18n.svelte';
import { requestAiLabel } from '$lib/chat/ai-name';
import { ChatSession } from '$lib/chat/session.svelte';
import type { AgentStartRequest } from '$lib/ipc/bindings/AgentStartRequest';
import type { CommandSpec } from '$lib/ipc/bindings/CommandSpec';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { SessionRef } from '$lib/ipc/bindings/SessionRef';
import type { SessionRoutineTag } from '$lib/ipc/bindings/SessionRoutineTag';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { ChatTabs } from './agent-tab-list.svelte';
import { buffered, nextNumber, type RestartOptions, type RestoredChat } from './agent-tabs';
import { harness } from './harness.svelte';
import { type MemoryIdentities, withMemory } from './agent-memory';
import { type ChatMemory, memoryChat } from './memoryChat.svelte';
import { prefs } from './prefs.svelte';
import { openRunWindow } from './routine-window';
import { isUntrusted, startFailureToast, untrustedReopenToast } from './start-failure';
import { toasts, type ToastsStore } from './toasts.svelte';

export type { RestartOptions, RestoredChat, StartingChat } from './agent-tabs';

/** Every open chat session (the tabs, see `ChatTabs`) and the agents behind them. */
export class AgentSessionsStore extends ChatTabs {
  /** The routine runs being attached to, by run id, so that a second ask waits for the same tab. */
  private attaching = new Map<string, Promise<ChatSession | null>>();

  private catalogs = new Map<HarnessId, Promise<CommandSpec[]>>();

  constructor(
    private readonly transport: Transport,
    private readonly notifications: ToastsStore,
    readonly memory: MemoryIdentities = memoryChat,
  ) {
    super();
  }

  /**
   * Starts an agent. Events can arrive before the command returns, so they wait until the session exists.
   * `quiet` drops a failure without a toast (reopening last run's chats: one that can't come back just stays closed).
   * A chat gets its memory identity here (see `withMemory`): `keep` is the identity of the chat being replaced or brought back.
   * `retry` is the caller's whole flow again with the folder trusted once: the "Trust once" action of the toast for a
   * folder the harness does not trust yet. Without one the action starts the same request again, trusting it once.
   */
  async start(
    request: AgentStartRequest,
    { activate = true, quiet = false, keep = null, retry }: { activate?: boolean; quiet?: boolean; keep?: ChatMemory | null; retry?: () => void } = {},
  ): Promise<ChatSession | null> {
    const launched = await this.launch(request, activate, keep);

    if (launched.kind === 'started') {
      return launched.session;
    }

    if (quiet) {
      return null;
    }

    if (launched.kind === 'unsupported') {
      const name = harness.get(request.harness)?.info.name ?? request.harness;
      this.notifications.push({ title: t('chat.toast.comingSoon', { harness: name }), description: t('chat.toast.comingSoonBody'), tone: 'info' });

      return null;
    }

    const trustOnce = retry ?? (() => void this.start({ ...request, trustOnce: true }, { activate, keep }));
    this.notifications.push(startFailureToast(launched.error, request, trustOnce));

    return null;
  }

  /** Starts an agent (see `start`) and says how it went, without telling the user. */
  private async launch(request: AgentStartRequest, activate: boolean, keep: ChatMemory | null): Promise<Launch> {
    // The callers gate on chat support already, this keeps a missed path from launching an agent that cannot run.
    if (!harness.canChat(request.harness)) {
      return { kind: 'unsupported' };
    }

    const feed = buffered();
    let session: ChatSession;
    const launched = await withMemory(this.memory, request, keep);

    try {
      const info = await this.transport.invoke('agent_start', { request: launched, onEvent: feed.onEvent });
      session = new ChatSession(info, this.transport, this.notifications);
    } catch (error) {
      const untrusted = isUntrusted(error) && !request.trustOnce;

      return { kind: untrusted ? 'untrusted' : 'failed', error };
    }

    feed.bind(session);

    this.memory.attach(session.key, launched.memoryChat, request.resume);
    if (!request.utility && !request.resume) {
      session.onFirstPrompt = (named) => void this.nameWithAi(named);
    }

    this.show(session, activate);

    return { kind: 'started', session };
  }

  /** Adds a session to the tabs, with a number for its placeholder name, and shows it when asked. */
  private show(session: ChatSession, activate: boolean): void {
    this.sessions = [...this.sessions, session];
    if (!this.numbers.has(session.key)) {
      this.numbers.set(session.key, nextNumber(this.numbers.values()));
    }
    if (activate) {
      this.activeKey = session.key;
    }
    void this.loadCatalog(session);
  }

  /**
   * Opens the live window of a routine run as a chat tab: the backend replays the run's conversation and then sends
   * what happens next, and the tab answers its questions and approvals like any chat. A run that is open already
   * is shown instead (the backend lets one window look at a run). Closing the tab stops the agent, as for any chat.
   * `quiet` drops a failure without a toast (the run has ended, its session can be read instead).
   */
  async attachRoutine(routine: SessionRoutineTag, { activate = true, quiet = false } = {}): Promise<ChatSession | null> {
    // A run that is being attached to (a toast action and a notification click at once) is waited for, not attached twice.
    let attaching = this.attaching.get(routine.runId);
    if (!attaching && !this.sessions.some((session) => session.routine?.runId === routine.runId)) {
      attaching = this.attachNew(routine, quiet).finally(() => this.attaching.delete(routine.runId));
      this.attaching.set(routine.runId, attaching);
    }

    const session = (await attaching) ?? this.sessions.find((open) => open.routine?.runId === routine.runId) ?? null;
    if (session && activate) {
      this.activeKey = session.key;
    }

    return session;
  }

  private async attachNew(routine: SessionRoutineTag, quiet: boolean): Promise<ChatSession | null> {
    const session = await openRunWindow(this.transport, this.notifications, routine, quiet);

    if (session) {
      this.show(session, false);
    }

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
  async create(harness: HarnessId, project: string, { trustOnce = false }: { trustOnce?: boolean } = {}): Promise<ChatSession | null> {
    const placeholder = this.slot(harness, project);
    this.starting = [...this.starting, placeholder];
    this.activeKey = placeholder.key;

    const request: AgentStartRequest = { harness, project, resume: null, approvalOverride: null, trustOnce, utility: false, memoryChat: null };
    const session = await this.start(request, { activate: false, retry: () => void this.create(harness, project, { trustOnce: true }) });
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
   * saved order whatever order the agents finish in. Chats whose folder the harness does not trust yet stay closed,
   * and one toast lists them with a "Trust once" action that reopens them in their places. Returns how many came back.
   */
  async restore(chats: RestoredChat[], wanted: number): Promise<number> {
    return this.reopenAll(chats, wanted, chats.map((_, index) => index), [], false);
  }

  /**
   * Reopens `chats[index]` for each of `indexes` (see `restore`). `back` holds the indexes that are open already, so
   * each tab lands in its saved place among them. `trustOnce` trusts each folder once.
   */
  private async reopenAll(chats: RestoredChat[], wanted: number, indexes: number[], back: number[], trustOnce: boolean): Promise<number> {
    const slots = indexes.map((index) => this.slot(chats[index].harness, chats[index].project));
    if (slots.length === 0) {
      return 0;
    }

    this.starting = [...this.starting, ...slots];
    const shown = indexes.indexOf(wanted);
    if (shown !== -1 || !trustOnce) {
      this.activeKey = (slots[shown] ?? slots[0]).key;
    }

    const untrusted: number[] = [];

    const sessions = await Promise.all(
      indexes.map(async (index, at) => {
        const chat = chats[index];
        const slot = slots[at];
        const reopened = await this.reopen(chat, trustOnce);
        const session = reopened === 'untrusted' ? null : reopened;
        this.starting = this.starting.filter((other) => other.key !== slot.key);
        this.handOverNumber(slot.key, session?.key ?? null);

        if (reopened === 'untrusted') {
          untrusted.push(index);
        }

        if (session) {
          session.aiLabel = chat.label;
          session.userNamed = chat.userNamed;
          back.push(index);
          this.moveTo(session.key, back.filter((other) => other < index).length);
          // A run's window is replayed by the backend, a resumed chat reads its conversation.
          if (!session.routine) {
            void session.loadHistory();
          }
        }

        if (this.activeKey === slot.key) {
          this.activeKey = session?.key ?? this.sessions.at(0)?.key ?? this.starting.at(0)?.key ?? null;
        }

        return session;
      }),
    );

    if (untrusted.length > 0) {
      const closed = untrusted.sort((a, b) => a - b);
      this.notifications.push(untrustedReopenToast(closed.map((index) => chats[index]), () => void this.reopenAll(chats, wanted, closed, back, true)));
    }

    return sessions.filter(Boolean).length;
  }

  /**
   * Brings one chat of the last run back: the window of a routine run that is still going on, else the saved
   * conversation (a run that ended meanwhile leaves its session to read). `untrusted` when the harness does not trust
   * its folder yet (and `trustOnce` did not trust it).
   */
  private async reopen(chat: RestoredChat, trustOnce: boolean): Promise<ChatSession | 'untrusted' | null> {
    if (chat.routine) {
      const attached = await this.attachRoutine(chat.routine, { activate: false, quiet: true });
      if (attached || chat.sessionId === '') {
        return attached;
      }
    }

    const request: AgentStartRequest = { harness: chat.harness, project: chat.project, resume: { harness: chat.harness, id: chat.sessionId }, approvalOverride: null, trustOnce, utility: false, memoryChat: null };
    const launched = await this.launch(request, false, chat.memory ?? null);

    if (launched.kind === 'untrusted') {
      return 'untrusted';
    }

    return launched.kind === 'started' ? launched.session : null;
  }

  /** Closes a tab and stops its agent. */
  async stop(key: string): Promise<void> {
    const index = this.sessions.findIndex((session) => session.key === key);
    if (index === -1) {
      return;
    }

    this.sessions = this.sessions.filter((session) => session.key !== key);
    this.numbers.delete(key);
    this.memory.release(key);
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

    const approvalOverride = options.approvalOverride === undefined ? old.info.approvalOverride : options.approvalOverride;
    const replacement: Replacement = {
      request: { harness: old.info.harness, project: old.info.project, resume: resumable(old), approvalOverride, trustOnce: options.trustOnce ?? false, utility: false, memoryChat: null },
      slot: `restart-${key}`,
      draft: old.draft,
      index: this.sessions.indexOf(old),
      number: this.numbers.get(key),
      // The replacement is the same chat for Harness Memory: its key, with the guards as they are now.
      keep: this.memory.get(key),
      aiLabel: old.aiLabel,
      userNamed: old.userNamed,
    };

    await this.stop(key);

    return this.replace(replacement);
  }

  /**
   * Starts the replacement of a stopped chat in its tab (see `restart`). Trusting the folder once from the toast of a
   * refused start runs this again, so the chat still comes back in its place with its draft and history.
   */
  private async replace(chat: Replacement): Promise<ChatSession | null> {
    // Reserved under a temporary key so the replacement keeps the tab's number (unless another chat took it meanwhile).
    if (chat.number !== undefined && ![...this.numbers.values()].includes(chat.number)) {
      this.numbers.set(chat.slot, chat.number);
    }

    const retry = () => void this.replace({ ...chat, request: { ...chat.request, trustOnce: true } });
    const fresh = await this.start(chat.request, { keep: chat.keep, retry });

    this.handOverNumber(chat.slot, fresh?.key ?? null);
    if (!fresh) {
      return null;
    }

    fresh.draft = chat.draft;
    fresh.aiLabel = chat.aiLabel;
    fresh.userNamed = chat.userNamed;
    this.moveTo(fresh.key, chat.index);
    if (chat.request.resume) {
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
  async changeHarness(key: string, harnessId: HarnessId, { trustOnce = false }: { trustOnce?: boolean } = {}): Promise<ChatSession | null> {
    const old = this.get(key);
    if (!old || old.switchingTo || old.info.harness === harnessId) {
      return null;
    }

    old.switchingTo = harnessId;
    const request: AgentStartRequest = { harness: harnessId, project: old.info.project, resume: null, approvalOverride: null, trustOnce, utility: false, memoryChat: null };
    const fresh = await this.start(request, { activate: false, retry: () => void this.changeHarness(key, harnessId, { trustOnce: true }) });
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

/** How a start went (see `AgentSessionsStore.launch`): `untrusted` when the harness does not trust the folder yet and the start did not trust it once. */
type Launch = { kind: 'started'; session: ChatSession } | { kind: 'unsupported' } | { kind: 'untrusted' | 'failed'; error: unknown };

/** A stopped chat to start again in its tab (see `AgentSessionsStore.restart`). */
interface Replacement {
  request: AgentStartRequest;
  /** The temporary key its number waits under. */
  slot: string;
  draft: string;
  index: number;
  number: number | undefined;
  keep: ChatMemory | null;
  aiLabel: ChatSession['aiLabel'];
  userNamed: boolean;
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
