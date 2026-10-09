import { initialChatState } from '$lib/chat/state';
import { prependHistory } from '$lib/chat/messages';
import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';
import type { SessionRef } from '$lib/ipc/bindings/SessionRef';
import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
import type { SessionTree } from '$lib/ipc/bindings/SessionTree';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { ViewLoader, type ViewStatus } from './view-loader.svelte';

/** The saved sessions of the active harness and project, newest first. */
export const sessionList = new ViewLoader<SessionSummary[]>((harness, project) => (project === null ? Promise.resolve([]) : ipc.invoke('sessions_list', { harness, project })));

const TRANSCRIPT_PAGE = 100;

interface SessionTarget {
  ref: SessionRef;
  project: string;
}

/** Everything the Sessions view shows about the selected session: its tree, transcript and touched files. */
export class SessionDetail {
  tree = $state.raw<SessionTree | null>(null);
  treeStatus = $state<ViewStatus>('idle');
  treeError = $state.raw<IpcFailure | null>(null);

  messages = $state.raw<ChatMessage[]>([]);
  nextCursor = $state<string | null>(null);
  total = $state(0);
  transcriptStatus = $state<ViewStatus>('idle');
  transcriptError = $state.raw<IpcFailure | null>(null);
  loadingMore = $state(false);

  touched = $state.raw<string[]>([]);

  /** The active path as read-only chat state, so the chat's own message rendering shows it. */
  transcript = $derived(prependHistory(initialChatState('transcript'), this.messages, this.nextCursor, this.total));

  private target: SessionTarget | null = null;
  private generation = 0;

  constructor(
    private readonly transport: Transport,
    private readonly pageSize: number = TRANSCRIPT_PAGE,
  ) {}

  get ref(): SessionRef | null {
    return this.target?.ref ?? null;
  }

  /** Loads a session; selecting the one already shown does nothing. */
  open(ref: SessionRef | null, project: string | null): void {
    const same = this.target?.ref.id === ref?.id && this.target?.ref.harness === ref?.harness && this.target?.project === project;
    if (same) {
      return;
    }

    this.generation += 1;
    this.target = ref && project ? { ref, project } : null;
    this.clear();

    if (this.target) {
      void this.load(this.target, this.generation);
    }
  }

  /** Reads the next page of the transcript and appends it. */
  async loadMore(): Promise<void> {
    const target = this.target;
    const cursor = this.nextCursor;
    if (!target || cursor === null || this.loadingMore) {
      return;
    }

    const mine = this.generation;
    this.loadingMore = true;

    try {
      const page = await this.transport.invoke('session_messages', { session: target.ref, project: target.project, cursor, limit: this.pageSize });
      if (mine === this.generation) {
        this.messages = [...this.messages, ...page.messages];
        this.nextCursor = page.nextCursor;
        this.total = page.total;
      }
    } catch (error) {
      if (mine === this.generation) {
        this.transcriptError = IpcFailure.from(error);
      }
    } finally {
      if (mine === this.generation) {
        this.loadingMore = false;
      }
    }
  }

  /** Re-reads the selected session from disk. */
  reload(): void {
    if (this.target) {
      this.generation += 1;
      void this.load(this.target, this.generation);
    }
  }

  private clear(): void {
    this.tree = null;
    this.treeStatus = this.target ? 'loading' : 'idle';
    this.treeError = null;
    this.messages = [];
    this.nextCursor = null;
    this.total = 0;
    this.transcriptStatus = this.target ? 'loading' : 'idle';
    this.transcriptError = null;
    this.loadingMore = false;
    this.touched = [];
  }

  private async load(target: SessionTarget, generation: number): Promise<void> {
    await Promise.all([this.loadTree(target, generation), this.loadFirstPage(target, generation), this.loadTouched(target, generation)]);
  }

  private async loadTree({ ref, project }: SessionTarget, generation: number): Promise<void> {
    try {
      const tree = await this.transport.invoke('session_tree', { session: ref, project });
      if (generation === this.generation) {
        this.tree = tree;
        this.treeError = null;
        this.treeStatus = 'ready';
      }
    } catch (error) {
      if (generation === this.generation) {
        this.treeError = IpcFailure.from(error);
        this.treeStatus = 'error';
      }
    }
  }

  private async loadFirstPage({ ref, project }: SessionTarget, generation: number): Promise<void> {
    try {
      const page = await this.transport.invoke('session_messages', { session: ref, project, cursor: null, limit: this.pageSize });
      if (generation === this.generation) {
        this.messages = page.messages;
        this.nextCursor = page.nextCursor;
        this.total = page.total;
        this.transcriptError = null;
        this.transcriptStatus = 'ready';
      }
    } catch (error) {
      if (generation === this.generation) {
        this.transcriptError = IpcFailure.from(error);
        this.transcriptStatus = 'error';
      }
    }
  }

  /** Touched files are a nicety: a failure leaves the list empty rather than breaking the view. */
  private async loadTouched({ ref, project }: SessionTarget, generation: number): Promise<void> {
    try {
      const paths = await this.transport.invoke('session_files_touched', { session: ref, project });
      if (generation === this.generation) {
        this.touched = paths;
      }
    } catch {
      if (generation === this.generation) {
        this.touched = [];
      }
    }
  }
}

export const sessionDetail = new SessionDetail(ipc);
