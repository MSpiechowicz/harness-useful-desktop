import type { RangeTarget } from '$lib/git/comment-range';
import type { CommentSide } from '$lib/ipc/bindings/CommentSide';
import type { KnowledgeLineComment } from '$lib/ipc/bindings/KnowledgeLineComment';
import type { KnowledgeVerdict } from '$lib/ipc/bindings/KnowledgeVerdict';
import { idKey, type KnowledgeId } from '$lib/ipc/knowledge-id';

/** The most line comments one review holds, and the longest comment in bytes (the contract of `docs review`). */
export const MAX_LINE_COMMENTS = 200;
export const MAX_COMMENT_BYTES = 4000;

/** The section a comment is written on: its heading path and which of the sections that share it it is, counted from 1 (what the server takes). */
export interface DraftSection {
  section: string;
  occurrence: number;
}

/** A line comment that is written but not sent yet: the section's heading path and the lines of one side of its diff. */
export interface DraftLineComment {
  id: number;
  section: string;
  /** Which of the sections of that heading it is on, counted from 1. */
  occurrence: number;
  side: CommentSide;
  startLine: number;
  endLine: number;
  body: string;
}

interface DocumentReview {
  comments: DraftLineComment[];
  note: string;
  verdict: KnowledgeVerdict;
  /** The revision of the document the line comments were written on: their lines belong to that text. Null while there are no line comments. */
  revision: number | null;
}

const EMPTY: DocumentReview = { comments: [], note: '', verdict: 'comment', revision: null };

/**
 * The review a person is writing, kept per document until it is submitted or discarded: line comments, the overall comment and the verdict. Nothing is sent to the
 * server before "Finish review", so leaving the view and coming back finds it as it was.
 */
export class KnowledgeReviewDraftStore {
  private reviews = $state.raw<Record<string, DocumentReview>>({});
  private nextId = 1;

  private of(id: KnowledgeId): DocumentReview {
    return this.reviews[idKey(id)] ?? EMPTY;
  }

  private set(id: KnowledgeId, review: DocumentReview): void {
    this.reviews = { ...this.reviews, [idKey(id)]: review };
  }

  comments(id: KnowledgeId): DraftLineComment[] {
    return this.of(id).comments;
  }

  note(id: KnowledgeId): string {
    return this.of(id).note;
  }

  verdict(id: KnowledgeId): KnowledgeVerdict {
    return this.of(id).verdict;
  }

  /** The revision the line comments were written on, null when there are none. */
  revision(id: KnowledgeId): number | null {
    return this.of(id).revision;
  }

  /** Whether the document moved on since the line comments were written: their lines may not be where the text is now. */
  stale(id: KnowledgeId, current: number): boolean {
    const revision = this.of(id).revision;

    return revision !== null && revision !== current;
  }

  /** The user looked at the new version: the line comments are now about it. */
  rebase(id: KnowledgeId, revision: number): void {
    const current = this.of(id);

    if (current.revision !== null) {
      this.set(id, { ...current, revision });
    }
  }

  /** Adds a comment on lines of a section written on `revision` of the document; false when the review is full. The first comment ties the review to its revision. */
  add(id: KnowledgeId, revision: number, at: DraftSection, target: Pick<RangeTarget, 'side' | 'startLine' | 'endLine'>, body: string): boolean {
    const current = this.of(id);

    if (current.comments.length >= MAX_LINE_COMMENTS) {
      return false;
    }

    const comment: DraftLineComment = { id: this.nextId, section: at.section, occurrence: at.occurrence, side: target.side, startLine: target.startLine, endLine: target.endLine, body };

    this.set(id, { ...current, comments: [...current.comments, comment], revision: current.revision ?? revision });
    this.nextId += 1;

    return true;
  }

  edit(id: KnowledgeId, commentId: number, body: string): void {
    const current = this.of(id);

    this.set(id, { ...current, comments: current.comments.map((comment) => (comment.id === commentId ? { ...comment, body } : comment)) });
  }

  remove(id: KnowledgeId, commentId: number): void {
    const current = this.of(id);

    const comments = current.comments.filter((comment) => comment.id !== commentId);

    this.set(id, { ...current, comments, revision: comments.length === 0 ? null : current.revision });
  }

  setNote(id: KnowledgeId, note: string): void {
    this.set(id, { ...this.of(id), note });
  }

  setVerdict(id: KnowledgeId, verdict: KnowledgeVerdict): void {
    this.set(id, { ...this.of(id), verdict });
  }

  /** What `knowledge_doc_review` takes: the comments as the server numbers lines, and the overall comment when there is one. */
  lines(id: KnowledgeId): KnowledgeLineComment[] {
    return this.of(id).comments.map((comment) => ({
      section: comment.section,
      occurrence: comment.occurrence,
      side: comment.side,
      from: comment.startLine,
      to: comment.endLine,
      body: comment.body.trim(),
    }));
  }

  /** Forgets the review of a document: it was submitted, or thrown away. */
  clear(id: KnowledgeId): void {
    const { [idKey(id)]: _gone, ...rest } = this.reviews;

    this.reviews = rest;
  }

  reset(): void {
    this.reviews = {};
  }
}

export const knowledgeReviewDraft = new KnowledgeReviewDraftStore();
