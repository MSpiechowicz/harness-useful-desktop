import type { ChatSession } from '$lib/chat/session.svelte';
import type { ComposedReview } from '$lib/ipc/bindings/ComposedReview';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { ReviewComment } from '$lib/ipc/bindings/ReviewComment';
import type { ReviewRequest } from '$lib/ipc/bindings/ReviewRequest';
import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';

import { agentSessions, type AgentSessionsStore } from './agentSession.svelte';
import { nav } from './nav.svelte';
import { toasts, type ToastsStore } from './toasts.svelte';

/** Limits from the review contract. */
export const MAX_COMMENTS = 100;
export const MAX_TEXT_BYTES = 8 * 1024;

/** Where a review goes: an open chat's key, or a brand-new chat. */
export type SendTarget = string | 'new';

export interface DraftComment extends ReviewComment {
  id: number;
}

interface ProjectReview {
  comments: DraftComment[];
  note: string;
}

const EMPTY: ProjectReview = { comments: [], note: '' };

export function byteLength(text: string): number {
  return new TextEncoder().encode(text).length;
}

/** Where drafts are kept between launches (the webview's storage, on this device only). */
const STORAGE_KEY = 'harness:review-drafts';

function isComment(value: unknown): value is DraftComment {
  const comment = value as DraftComment | null;

  return (
    typeof comment?.id === 'number' &&
    typeof comment.path === 'string' &&
    (comment.side === 'old' || comment.side === 'new') &&
    typeof comment.startLine === 'number' &&
    typeof comment.endLine === 'number' &&
    typeof comment.body === 'string' &&
    Array.isArray(comment.snippet)
  );
}

/** The saved drafts, keeping only well-formed entries; an unreadable record is an empty one. */
function loadDrafts(): Record<string, ProjectReview> {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}') as Record<string, Partial<ProjectReview>>;

    return Object.fromEntries(
      Object.entries(saved).map(([project, review]) => [
        project,
        { comments: (Array.isArray(review?.comments) ? review.comments : []).filter(isComment).slice(0, MAX_COMMENTS), note: typeof review?.note === 'string' ? review.note : '' },
      ]),
    );
  } catch {
    return {};
  }
}

/**
 * Pending review comments per project. They survive switching views and projects and closing the app (saved on this
 * device), and disappear when sent or discarded.
 */
export class ReviewStore {
  private reviews = $state.raw<Record<string, ProjectReview>>(loadDrafts());
  private nextId = Math.max(0, ...Object.values(this.reviews).flatMap((review) => review.comments.map((comment) => comment.id))) + 1;

  constructor(
    private readonly transport: Transport,
    private readonly agents: AgentSessionsStore,
    private readonly notifications: ToastsStore,
  ) {}

  private persist(): void {
    try {
      const kept = Object.fromEntries(Object.entries(this.reviews).filter(([, review]) => review.comments.length > 0 || review.note.trim() !== ''));
      localStorage.setItem(STORAGE_KEY, JSON.stringify(kept));
    } catch {
      // Storage can be unavailable; the drafts then last until the app closes.
    }
  }

  commentsFor(project: string | null): DraftComment[] {
    return project === null ? EMPTY.comments : this.of(project).comments;
  }

  noteFor(project: string | null): string {
    return project === null ? '' : this.of(project).note;
  }

  /** Adds a comment; false when the review already holds the maximum. */
  add(project: string, comment: ReviewComment): boolean {
    const current = this.of(project);
    if (current.comments.length >= MAX_COMMENTS) {
      this.notifications.push({ title: `A review holds at most ${MAX_COMMENTS} comments`, description: 'Send or discard the current ones first.', tone: 'warning' });

      return false;
    }

    this.set(project, { ...current, comments: [...current.comments, { ...comment, id: this.nextId }] });
    this.nextId += 1;

    return true;
  }

  edit(project: string, id: number, body: string): void {
    const current = this.of(project);
    this.set(project, { ...current, comments: current.comments.map((comment) => (comment.id === id ? { ...comment, body } : comment)) });
  }

  remove(project: string, id: number): void {
    const current = this.of(project);
    this.set(project, { ...current, comments: current.comments.filter((comment) => comment.id !== id) });
  }

  setNote(project: string, note: string): void {
    this.set(project, { ...this.of(project), note });
  }

  /** Forgets every comment and the note of a project. */
  discard(project: string): void {
    this.reviews = Object.fromEntries(Object.entries(this.reviews).filter(([key]) => key !== project));
    this.persist();
  }

  /** The request the backend composes: comments grouped by file in a stable order. */
  requestFor(project: string): ReviewRequest {
    const { comments, note } = this.of(project);
    const ordered = [...comments].sort((a, b) => a.path.localeCompare(b.path) || a.startLine - b.startLine || a.endLine - b.endLine);

    return {
      comments: ordered.map(({ path, side, startLine, endLine, body, snippet }) => ({ path, side, startLine, endLine, body, snippet })),
      note: note.trim() === '' ? null : note.trim(),
    };
  }

  /** The exact prompt that would be sent. Throws the backend's refusal (limits). */
  async compose(project: string): Promise<ComposedReview> {
    return this.transport.invoke('review_compose', { request: this.requestFor(project) });
  }

  /**
   * Delivers the composed prompt to a chat: as a follow-up when that chat is busy, as a prompt when
   * idle, or as the first message of a new chat. On success the comments are cleared and the chat opens.
   */
  async send(project: string, harness: HarnessId, target: SendTarget, prompt: string): Promise<boolean> {
    const session = await this.sessionFor(project, harness, target);
    if (!session) {
      return false;
    }

    const delivered = await session.submit(prompt, session.streaming ? 'followUp' : 'send');
    if (!delivered) {
      return false;
    }

    this.agents.select(session.key);
    this.discard(project);
    nav.go('chat');

    return true;
  }

  private async sessionFor(project: string, harness: HarnessId, target: SendTarget): Promise<ChatSession | null> {
    if (target === 'new') {
      return this.agents.create(harness, project);
    }

    const open = this.agents.get(target);
    if (!open) {
      this.notifications.push({ title: 'That chat is no longer open', description: 'Pick another chat, or start a new one.', tone: 'warning' });
    }

    return open ?? null;
  }

  private of(project: string): ProjectReview {
    return this.reviews[project] ?? EMPTY;
  }

  private set(project: string, review: ProjectReview): void {
    this.reviews = { ...this.reviews, [project]: review };
    this.persist();
  }
}

export const review = new ReviewStore(ipc, agentSessions, toasts);
