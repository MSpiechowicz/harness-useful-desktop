import { t } from '$lib/i18n/i18n.svelte';
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
import { splitFileNote, toImages, withFileNote } from './attachments';
import { ComposerAttachments, type Attached } from './composer-attachments.svelte';
import { finishBash, startBash } from './bash';
import { messageAction, type SendMode } from './composer';
import { labelFromPrompt, type ChatLabel } from './naming';
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
  /** Images and files waiting in the composer for the next message. */
  readonly attached: ComposerAttachments;
  /** Thumbnails of the images this window sent, by message key; the transcript itself keeps no image data. */
  sentImages = $state.raw<Record<string, string[]>>({});
  models = $state.raw<ModelOption[] | null>(null);
  thinkingLevels = $state.raw<string[] | null>(null);
  stats = $state.raw<SessionStats | null>(null);
  catalog = $state.raw<CommandSpec[]>([]);
  /** Counts finished tool runs: each may have changed files, so git state and the saved session go stale. */
  toolRuns = $state(0);
  /** Counts finished turns, as a number that only changes when a turn ends. */
  settledCount = $derived(this.chat.settledCount);
  ui = $state<SessionUi>({ modelPicker: false, thinkingPicker: false, stats: false, approvalChooser: false, ask: null, confirmBash: null });

  approval = $derived<ApprovalPrompt | null>(pendingApproval(this.chat));
  dialog = $derived(pendingDialog(this.chat));
  commands = $derived(mergeCatalog(this.chat.commands, this.catalog));
  streaming = $derived(this.chat.phase === 'streaming');
  ended = $derived(this.chat.phase === 'exited');
  name = $derived(this.chat.session?.sessionName ?? this.chat.title ?? null);
  /** A name the user typed; it beats every generated one. */
  userNamed = $state(false);
  /** The name and description a helper agent gave the chat (the "AI chat names" setting); null until it answers. */
  aiLabel = $state.raw<ChatLabel | null>(null);
  /** What the chat was opened with, known only while the whole transcript is (not for a resumed chat's newest page). */
  firstPrompt = $derived.by(() => {
    if (this.chat.historyCursor !== null) {
      return '';
    }

    const first = this.chat.items.find((item) => item.kind === 'message' && item.role === 'user');

    return first?.kind === 'message' ? splitFileNote(first.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n')).text : '';
  });
  /** The card text: the helper's when it answered, else the opening words of the first message. */
  label = $derived(this.aiLabel ?? labelFromPrompt(this.firstPrompt));
  /** Called once, when the first user message of a new chat lands. */
  onFirstPrompt: ((session: ChatSession) => void) | null = null;

  private incoming: AgentEnvelope[] = [];
  private flushScheduled = false;
  /** Thumbnails of sent messages not yet matched to their transcript item, oldest first. */
  private unmatchedImages: string[][] = [];
  private historyLoaded = false;

  /** The session's launch info; `project` changes when the chat moves to another project. */
  info: AgentSessionInfo = $state.raw() as unknown as AgentSessionInfo;

  constructor(
    info: AgentSessionInfo,
    private readonly transport: Transport,
    private readonly notifications: ToastsStore,
  ) {
    this.info = info;
    this.attached = new ComposerAttachments(transport, notifications);
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

    let toolRuns = 0;
    for (const envelope of batch) {
      next = reduceEnvelope(next, envelope);
      if (envelope.event.type === 'toolEnd') {
        toolRuns += 1;
      }
    }

    if (next.gaps > before.gaps) {
      console.warn(`agent session ${this.key}: events arrived with a gap in their sequence`);
    }

    this.chat = next;
    this.matchSentImages(next);
    if (toolRuns > 0) {
      this.toolRuns += toolRuns;
    }
    this.afterChange(before, next);
  }

  /** Gives each new user message that carries images the thumbnails of the oldest send still waiting. */
  private matchSentImages(after: ChatState): void {
    if (this.unmatchedImages.length === 0) {
      return;
    }

    const matched: Record<string, string[]> = {};

    for (const item of after.items) {
      const hasImages = item.kind === 'message' && item.role === 'user' && item.blocks.some((block) => block.type === 'image');
      if (!hasImages || item.key in this.sentImages || this.unmatchedImages.length === 0) {
        continue;
      }

      matched[item.key] = this.unmatchedImages.shift() ?? [];
    }

    if (Object.keys(matched).length > 0) {
      this.sentImages = { ...this.sentImages, ...matched };
    }
  }

  /** Reacts to what a batch changed: ask for fresh facts once the agent is ready or settled. */
  private afterChange(before: ChatState, after: ChatState): void {
    if (after.readyCount > before.readyCount) {
      void this.send({ type: 'refreshCommands' });
    }

    if (after.settledCount > before.settledCount) {
      void this.send({ type: 'getState' });
    }

    if (before.lastUserText === null && after.lastUserText !== null) {
      const callback = this.onFirstPrompt;
      this.onFirstPrompt = null;
      callback?.(this);
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
      this.notifications.push({ title: t('chat.toast.notAccepted'), description: IpcFailure.from(error).message, tone: 'danger' });

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

  /**
   * Receives the files dragged in from the file manager while the returned function has not been
   * called: `hover` follows a drag over the window and dropped images join the composer.
   */
  listenForDrops(hover: (active: boolean) => void): () => void {
    let listening = true;

    this.transport
      .invoke('drop_listen', {
        onDrop: (event) => {
          if (!listening || this.ended) {
            return;
          }

          if (event.type === 'hover') {
            hover(event.active);
          } else {
            void this.attached.addDropped(event.images, event.files, event.skipped);
          }
        },
      })
      .catch(() => {});

    return () => {
      listening = false;
      hover(false);
    };
  }

  /** Sends the user's text with its images and files; false when it did not go out (so the composer can keep the draft). */
  async submit(text: string, mode: SendMode, attached: Attached = { images: [], files: [] }): Promise<boolean> {
    const message = withFileNote(text.trim(), attached.files);
    if (message === '') {
      return false;
    }

    // Queued before the send: the message event may land before the reply does.
    const thumbnails = attached.images.map((image) => image.url);
    if (thumbnails.length > 0) {
      this.unmatchedImages.push(thumbnails);
    }

    const accepted = (await this.send(messageAction(message, mode, toImages(attached.images)))) !== null;
    if (!accepted && thumbnails.length > 0) {
      this.unmatchedImages = this.unmatchedImages.filter((queued) => queued !== thumbnails);
    }

    return accepted;
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
      this.notifications.push({ title: t('chat.toast.noAnswer'), description: IpcFailure.from(error).message, tone: 'danger' });
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
    if (reply) {
      this.userNamed = true;
    }
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

  /** Replaces the transcript with the session's history, after the session changed under the chat (a fork). */
  async reloadHistory(): Promise<void> {
    this.historyLoaded = false;
    this.apply((state) => ({ ...state, items: [], toolCalls: {}, subagents: {}, nextOrder: 0, lastUserText: null, historyCursor: null, historyTotal: null }));
    await this.loadHistory();
  }

  get hasEarlier(): boolean {
    return this.chat.historyCursor !== null;
  }
}
