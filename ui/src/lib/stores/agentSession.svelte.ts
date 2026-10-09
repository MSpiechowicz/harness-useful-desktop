import { ChatSession } from '$lib/chat/session.svelte';
import type { AgentEnvelope } from '$lib/ipc/bindings/AgentEnvelope';
import type { AgentStartRequest } from '$lib/ipc/bindings/AgentStartRequest';
import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
import type { CommandSpec } from '$lib/ipc/bindings/CommandSpec';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { toasts, type ToastsStore } from './toasts.svelte';

/** Options a restart can change; everything else carries over from the session being replaced. */
export interface RestartOptions {
  approvalOverride?: ApprovalMode | null;
  trustOnce?: boolean;
}

/** Every open chat session (the tabs) and which one is showing. */
export class AgentSessionsStore {
  sessions = $state.raw<ChatSession[]>([]);
  activeKey = $state<string | null>(null);

  active: ChatSession | null = $derived(this.sessions.find((session) => session.key === this.activeKey) ?? null);

  private catalogs = new Map<HarnessId, Promise<CommandSpec[]>>();

  constructor(
    private readonly transport: Transport,
    private readonly notifications: ToastsStore,
  ) {}

  get(key: string): ChatSession | undefined {
    return this.sessions.find((session) => session.key === key);
  }

  /** Starts an agent. Events can arrive before the command returns, so they wait in `pending` until the session exists. */
  async start(request: AgentStartRequest): Promise<ChatSession | null> {
    const pending: AgentEnvelope[] = [];
    let session: ChatSession | null = null;
    const onEvent = (envelope: AgentEnvelope): void => (session ? session.ingest(envelope) : void pending.push(envelope));

    try {
      const info = await this.transport.invoke('agent_start', { request, onEvent });
      session = new ChatSession(info, this.transport, this.notifications);
    } catch (error) {
      this.notifications.push({ title: 'Could not start the agent', description: IpcFailure.from(error).message, tone: 'danger' });

      return null;
    }

    for (const envelope of pending) {
      session.ingest(envelope);
    }

    this.sessions = [...this.sessions, session];
    this.activeKey = session.key;
    void this.loadCatalog(session);

    return session;
  }

  /** Starts a chat for the harness and project the user is working in. */
  async create(harness: HarnessId, project: string): Promise<ChatSession | null> {
    return this.start({ harness, project, resume: null, approvalOverride: null, trustOnce: false });
  }

  /** Closes a tab and stops its agent. */
  async stop(key: string): Promise<void> {
    const index = this.sessions.findIndex((session) => session.key === key);
    if (index === -1) {
      return;
    }

    this.sessions = this.sessions.filter((session) => session.key !== key);
    if (this.activeKey === key) {
      this.activeKey = this.sessions[Math.min(index, this.sessions.length - 1)]?.key ?? null;
    }

    try {
      await this.transport.invoke('agent_stop', { key });
    } catch (error) {
      this.notifications.push({ title: 'Could not stop the agent', description: IpcFailure.from(error).message, tone: 'warning' });
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

    const sessionId = old.chat.session?.sessionId;
    const resume = sessionId ? { harness: old.info.harness, id: sessionId } : null;
    const draft = old.draft;
    const index = this.sessions.indexOf(old);

    await this.stop(key);

    const fresh = await this.start({
      harness: old.info.harness,
      project: old.info.project,
      resume,
      approvalOverride: options.approvalOverride === undefined ? old.info.approvalOverride : options.approvalOverride,
      trustOnce: options.trustOnce ?? false,
    });

    if (!fresh) {
      return null;
    }

    fresh.draft = draft;
    this.moveTo(fresh.key, index);
    if (resume) {
      void fresh.loadHistory();
    }

    return fresh;
  }

  select(key: string): void {
    if (this.get(key)) {
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
