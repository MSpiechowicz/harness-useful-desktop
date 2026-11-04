import { knowledgeChanges } from './knowledgeChanges.svelte';
import { knowledgeStats } from './knowledgeStats.svelte';

/** Whether two git names are the same person, as the server compares them: trimmed, without regard to case. */
export function sameName(left: string | null | undefined, right: string | null | undefined): boolean {
  return left != null && right != null && left.trim() !== '' && left.trim().toLowerCase() === right.trim().toLowerCase();
}

/**
 * Who the user is to Harness Knowledge: the git `user.name` it signs their writes with. The server says it (the pulse and the stats carry `me`),
 * so an author is known as the user from the first look. A server that predates that learns the name from what it refuses: `selfApproval` says the
 * author of the document is the user (when nobody else could be the one refused), and `noName` that the user has none. Until a name is known `is` answers null and the review panel offers the
 * reviewer's buttons, which the server still checks.
 */
export class KnowledgeViewerStore {
  /** A name learned from a refusal, for a server that does not say it. */
  private learned = $state<string | null>(null);
  private refusedNoName = $state(false);

  /** The user's git name: what the server says, else what a refusal taught. Null while unknown. */
  get name(): string | null {
    return knowledgeChanges.pulse?.me?.trim() || knowledgeStats.value?.me?.name?.trim() || this.learned;
  }

  /** The user has no git `user.name`: the server said so up front (it knows who the user is and has no name for them), or refused a step for it. */
  get noName(): boolean {
    if (this.name !== null) {
      return false;
    }

    const me = knowledgeStats.value?.me;

    return this.refusedNoName || (me != null && !me.name?.trim());
  }

  /** Keeps `name` as the user's own. */
  learn(name: string | null | undefined): void {
    const trimmed = name?.trim();

    if (trimmed) {
      this.learned = trimmed;
      this.refusedNoName = false;
    }
  }

  /** The server refused a step because the user has no git name. */
  learnNoName(): void {
    this.refusedNoName = true;
  }

  /** Whether `name` is the user: null while the user's name is not known, and for a document nobody signed. */
  is(name: string | null | undefined): boolean | null {
    const mine = this.name;

    if (mine === null || !name?.trim()) {
      return null;
    }

    return mine.toLowerCase() === name.trim().toLowerCase();
  }

  reset(): void {
    this.learned = null;
    this.refusedNoName = false;
  }
}

export const knowledgeViewer = new KnowledgeViewerStore();
