import type { AgentAction } from '$lib/ipc/bindings/AgentAction';
import type { AgentEnvelope } from '$lib/ipc/bindings/AgentEnvelope';
import type { AgentReply } from '$lib/ipc/bindings/AgentReply';
import type { AgentSessionInfo } from '$lib/ipc/bindings/AgentSessionInfo';
import type { CommandSpec } from '$lib/ipc/bindings/CommandSpec';
import type { ModelOption } from '$lib/ipc/bindings/ModelOption';
import type { SessionStats } from '$lib/ipc/bindings/SessionStats';
import type { UiOutcome } from '$lib/ipc/bindings/UiOutcome';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import type { ToastsStore } from '$lib/stores/toasts.svelte';

import { pendingApproval, pendingDialog, type ApprovalPrompt } from './approval';
import { finishBash, startBash } from './bash';
import { messageAction, type SendMode } from './composer';
import { prependHistory } from './messages';
import { applyEvent, dropRequest, reduceEnvelope } from './reducer';
import { mergeCatalog } from './slash';
import { initialChatState } from './state';
import type { ChatState } from './types';

const HISTORY_PAGE = 50;
const FLUSH_FALLBACK_MS = 100;

/** Small pieces of view state that slash commands and other surfaces can open from outside the header. */
export interface SessionUi {
  modelPicker: boolean;
  thinkingPicker: boolean;
  stats: boolean;
  approvalChooser: boolean;
  /** A line of text the user is being asked for (compact instructions, a session name). */
  ask: 'compact' | 'name' | null;
  /** A `!command` waiting for the user's confirmation. */
  confirmBash: string | null;
}

/**
 * One running agent session: its transcript (reduced from the event stream), the composer
 * draft, and every request the UI can make of it. Events are applied once per frame, so a
 * burst of streamed deltas costs one render.
 */
export class ChatSession {
  readonly key: string;

  chat = $state.raw<ChatState>(initialChatState(''));
  draft = $state('');
  models = $state.raw<ModelOption[] | null>(null);
  thinkingLevels = $state.raw<string[] | null>(null);
  stats = $state.raw<SessionStats | null>(null);
  catalog = $state.raw<CommandSpec[]>([]);
  ui = $state<SessionUi>({ modelPicker: false, thinkingPicker: false, stats: false, approvalChooser: false, ask: null, confirmBash: null });

  approval = $derived<ApprovalPrompt | null>(pendingApproval(this.chat));
  dialog = $derived(pendingDialog(this.chat));
  commands = $derived(mergeCatalog(this.chat.commands, this.catalog));
  streaming = $derived(this.chat.phase === 'streaming');
  ended = $derived(this.chat.phase === 'exited');
  name = $derived(this.chat.session?.sessionName ?? this.chat.title ?? null);

  private incoming: AgentEnvelope[] = [];
  private flushScheduled = false;
  private historyLoaded = false;

  /** The session's launch info; `project` changes when the chat moves to another project. */
  info: AgentSessionInfo = $state.raw() as unknown as AgentSessionInfo;

  constructor(
    info: AgentSessionInfo,
    private readonly transport: Transport,
    private readonly notifications: ToastsStore,
  ) {
    this.info = info;
    this.key = info.key;
    this.chat = initialChatState(info.key);
  }

  /** Queues an event; the transcript updates on the next frame. */
  ingest(envelope: AgentEnvelope): void {
    this.incoming.push(envelope);

    if (!this.flushScheduled) {
      this.flushScheduled = true;
      requestAnimationFrame(() => this.flush());
      setTimeout(() => this.flush(), FLUSH_FALLBACK_MS);
    }
  }

  /** Applies everything queued so far, in order. */
  flush(): void {
    this.flushScheduled = false;
    if (this.incoming.length === 0) {
      return;
    }

    const batch = this.incoming;
    this.incoming = [];
    const before = this.chat;
    let next = before;

    for (const envelope of batch) {
      next = reduceEnvelope(next, envelope);
    }

    if (next.gaps > before.gaps) {
      console.warn(`agent session ${this.key}: events arrived with a gap in their sequence`);
    }

    this.chat = next;
    this.afterChange(before, next);
  }

  /** Reacts to what a batch changed: ask for fresh facts once the agent is ready or settled. */
  private afterChange(before: ChatState, after: ChatState): void {
    if (after.readyCount > before.readyCount) {
      void this.send({ type: 'refreshCommands' });
    }

    if (after.settledCount > before.settledCount) {
      void this.send({ type: 'getState' });
    }
  }

  /** Applies a change the UI already knows about (an answered request, a reply that carries state). */
  apply(update: (state: ChatState) => ChatState): void {
    this.chat = update(this.chat);
  }

  /** Sends an action; failures become a toast and `null`. */
  async send(action: AgentAction): Promise<AgentReply | null> {
    try {
      const reply = await this.transport.invoke('agent_send', { key: this.key, action });
      this.absorb(reply);

      return reply;
    } catch (error) {
      this.notifications.push({ title: 'The agent did not accept that', description: IpcFailure.from(error).message, tone: 'danger' });

      return null;
    }
  }

  private absorb(reply: AgentReply): void {
    if (reply.type === 'state') {
      this.apply((state) => applyEvent(state, { type: 'state', state: reply.state }));
    } else if (reply.type === 'commands') {
      this.apply((state) => applyEvent(state, { type: 'commandsUpdated', commands: reply.commands }));
    }
  }

  /** Sends the user's text; false when it did not go out (so the composer can keep the draft). */
  async submit(text: string, mode: SendMode): Promise<boolean> {
    const message = text.trim();
    if (message === '') {
      return false;
    }

    return (await this.send(messageAction(message, mode))) !== null;
  }

  async abort(): Promise<void> {
    await this.send({ type: 'abort' });
  }

  /** Sends the last thing the user said again, after a failed turn. */
  async retry(): Promise<void> {
    const text = this.chat.lastUserText;
    if (text === null) {
      return;
    }

    this.apply((state) => ({ ...state, promptError: null }));
    await this.submit(text, 'send');
  }

  /** Answers an agent UI request; the dialog closes immediately and the answer follows. */
  async respond(requestId: string, outcome: UiOutcome): Promise<void> {
    this.apply((state) => dropRequest(state, requestId));

    try {
      await this.transport.invoke('agent_ui_respond', { key: this.key, response: { id: requestId, outcome } });
    } catch (error) {
      this.notifications.push({ title: 'The agent did not get your answer', description: IpcFailure.from(error).message, tone: 'danger' });
    }
  }

  /** Runs a confirmed `!command` and shows its output in the transcript. */
  async runBash(command: string): Promise<void> {
    const started = startBash(this.chat, command);
    this.chat = started.state;

    try {
      const output = await this.transport.invoke('agent_bash', { key: this.key, command, confirmed: true });
      this.apply((state) => finishBash(state, started.key, output));
    } catch (error) {
      this.apply((state) => finishBash(state, started.key, { error: IpcFailure.from(error).message }));
    }
  }

  async loadModels(): Promise<void> {
    if (this.models !== null) {
      return;
    }

    const reply = await this.send({ type: 'getAvailableModels' });
    if (reply?.type === 'models') {
      this.models = reply.models;
    }
  }

  async loadThinkingLevels(): Promise<void> {
    if (this.thinkingLevels !== null) {
      return;
    }

    const reply = await this.send({ type: 'getAvailableThinkingLevels' });
    if (reply?.type === 'thinkingLevels') {
      this.thinkingLevels = reply.levels;
    }
  }

  async loadStats(): Promise<void> {
    const reply = await this.send({ type: 'getSessionStats' });
    if (reply?.type === 'stats') {
      this.stats = reply.stats;
    }
  }

  async chooseModel(model: ModelOption): Promise<void> {
    const reply = await this.send({ type: 'setModel', provider: model.provider, modelId: model.id });
    if (reply) {
      this.apply((state) => applyEvent(state, { type: 'modelChanged', model }));
    }
  }

  async chooseThinking(level: string): Promise<void> {
    const reply = await this.send({ type: 'setThinkingLevel', level });
    if (reply) {
      this.apply((state) => applyEvent(state, { type: 'thinkingChanged', level }));
    }
  }

  async rename(name: string): Promise<void> {
    const trimmed = name.trim();
    const session = this.chat.session;
    if (trimmed === '' || trimmed === this.name) {
      return;
    }

    const reply = await this.send({ type: 'setSessionName', name: trimmed });
    if (reply && session) {
      this.apply((state) => applyEvent(state, { type: 'state', state: { ...(state.session ?? session), sessionName: trimmed } }));
    }
  }

  /** Older messages of a resumed session: the first call loads the newest page. */
  async loadHistory(): Promise<void> {
    if (this.historyLoaded && this.chat.historyCursor === null) {
      return;
    }

    const reply = await this.send({ type: 'getHistory', cursor: this.historyLoaded ? this.chat.historyCursor : null, limit: HISTORY_PAGE });
    if (reply?.type === 'history') {
      this.historyLoaded = true;
      this.apply((state) => prependHistory(state, reply.messages, reply.nextCursor, reply.total));
    }
  }

  get hasEarlier(): boolean {
    return this.chat.historyCursor !== null;
  }
}
