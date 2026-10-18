import { initialChatState } from '$lib/chat/state';
import { prependHistory } from '$lib/chat/messages';
import type { ChatMessage } from '$lib/ipc/bindings/ChatMessage';
import type { SessionRef } from '$lib/ipc/bindings/SessionRef';
import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
import type { TranscriptStats } from '$lib/ipc/bindings/TranscriptStats';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import { Flight } from '$lib/utils/flight';

import { ViewLoader, type ViewStatus } from './view-loader.svelte';

/** The saved sessions of the active harness and project, newest first. */
export const sessionList = new ViewLoader<SessionSummary[]>((harness, project) => (project === null ? Promise.resolve([]) : ipc.invoke('sessions_list', { harness, project })));

const TRANSCRIPT_PAGE = 100;
/** How long a session switch keeps the previous session on screen before showing the loading state. */
const STALE_MS = 250;

function same(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

interface SessionTarget {
  ref: SessionRef;
  project: string;
}

/** Everything the Sessions view shows about the selected session: its transcript, totals and touched files. */
export class SessionDetail {
  messages = $state.raw<ChatMessage[]>([]);
  nextCursor = $state<string | null>(null);
  total = $state(0);
  transcriptStatus = $state<ViewStatus>('idle');
  transcriptError = $state.raw<IpcFailure | null>(null);
  loadingMore = $state(false);

  /** Totals of the whole active path; they arrive with the first page. */
  stats = $state.raw<TranscriptStats | null>(null);
  touched = $state.raw<string[]>([]);

  /** The active path as read-only chat state, so the chat's own message rendering shows it. */
  transcript = $derived(prependHistory(initialChatState('transcript'), this.messages, this.nextCursor, this.total));

  private target: SessionTarget | null = null;
  private generation = 0;
  /** Where the newest loaded page starts: its cursor, and how many messages come before it. */
  private tail: { cursor: string | null; offset: number } = { cursor: null, offset: 0 };
  /** Whether the first page of the session being opened has arrived. */
  private settled = false;
  private readonly flight = new Flight();

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
    this.flight.clear();
    const generation = this.generation;
    const hadSession = this.target !== null;
    this.target = ref && project ? { ref, project } : null;
    this.settled = false;

    if (!this.target || !hadSession) {
      this.clear();
    } else {
      // Switching sessions keeps the previous one on screen: a quick read swaps straight to the new one, and only a
      // slow one falls back to the loading placeholders.
      setTimeout(() => this.showLoading(generation), STALE_MS);
      this.nextCursor = null;
      this.loadingMore = false;

      // The totals and touched files belong to the previous session; they must never show under the new one.
      this.stats = null;
      this.touched = [];
    }

    if (this.target) {
      void this.load(this.target, generation);
    }
  }

  /** The loading state, if the first page has not arrived once a switch takes longer than [`STALE_MS`]. */
  private showLoading(generation: number): void {
    if (generation === this.generation && !this.settled) {
      this.transcriptStatus = 'loading';
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
        this.tail = { cursor, offset: this.messages.length };
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
  reload(): Promise<void> {
    if (!this.target) {
      return Promise.resolve();
    }

    this.generation += 1;
    this.loadingMore = false;

    return this.load(this.target, this.generation);
  }

  /**
   * Catches up with a session that is still being written, in the background: the newest loaded page is read again
   * (older pages stay as they are), with the touched files. A failure keeps what is shown. A transcript that failed to
   * load is read again from the start instead. A look never starts while the previous one is still running.
   */
  follow(): Promise<void> {
    if (this.transcriptStatus === 'error') {
      return this.flight.startUnlessRunning(() => this.reload());
    }

    return this.flight.startUnlessRunning(() => this.catchUp());
  }

  private async catchUp(): Promise<void> {
    const target = this.target;
    if (!target || this.transcriptStatus !== 'ready' || this.loadingMore) {
      return;
    }

    const mine = this.generation;
    const tail = this.tail;

    try {
      const [page, touched] = await Promise.all([
        this.transport.invoke('session_messages', { session: target.ref, project: target.project, cursor: tail.cursor, limit: this.pageSize }),
        this.transport.invoke('session_files_touched', { session: target.ref, project: target.project }),
      ]);
      // A newer page or another session arrived meanwhile; this reading is already behind.
      if (mine !== this.generation || this.tail !== tail || this.loadingMore) {
        return;
      }

      // A session nobody is writing reads the same every time; keeping what is shown means the transcript is not redrawn
      // (and its scroll position and selection are not disturbed) on every look.
      if (!same(page.messages, this.messages.slice(tail.offset)) || page.total !== this.total || page.nextCursor !== this.nextCursor) {
        this.messages = [...this.messages.slice(0, tail.offset), ...page.messages];
        this.nextCursor = page.nextCursor;
        this.total = page.total;
      }

      if (!same(touched, this.touched)) {
        this.touched = touched;
      }

      // Only the first page carries the totals.
      if (tail.cursor === null && !same(page.stats, this.stats)) {
        this.stats = page.stats;
      }
    } catch {
      // The next activity tries again.
    }
  }

  private clear(): void {
    this.messages = [];
    this.nextCursor = null;
    this.total = 0;
    this.transcriptStatus = this.target ? 'loading' : 'idle';
    this.transcriptError = null;
    this.loadingMore = false;
    this.stats = null;
    this.touched = [];
    this.tail = { cursor: null, offset: 0 };
  }

  private async load(target: SessionTarget, generation: number): Promise<void> {
    await Promise.all([this.loadFirstPage(target, generation), this.loadTouched(target, generation)]);
  }

  private async loadFirstPage({ ref, project }: SessionTarget, generation: number): Promise<void> {
    try {
      const page = await this.transport.invoke('session_messages', { session: ref, project, cursor: null, limit: this.pageSize });
      if (generation === this.generation) {
        this.settled = true;
        this.tail = { cursor: null, offset: 0 };
        this.messages = page.messages;
        this.stats = page.stats;
        this.nextCursor = page.nextCursor;
        this.total = page.total;
        this.transcriptError = null;
        this.transcriptStatus = 'ready';
      }
    } catch (error) {
      if (generation === this.generation) {
        this.settled = true;
        this.stats = null;
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
