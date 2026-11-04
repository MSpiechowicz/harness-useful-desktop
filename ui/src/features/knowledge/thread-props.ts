import type { KnowledgeComment } from '$lib/ipc/bindings/KnowledgeComment';
import type { KnowledgeId } from '$lib/ipc/knowledge-id';

/** What every review thread of a document needs, besides the comment that starts it. */
export interface ThreadProps {
  /** Every comment of the document, for the replies of a thread. */
  all: readonly KnowledgeComment[];
  /** The user may not change what is kept in this place, so nobody can answer, resolve or delete. */
  readOnly: boolean;
  /** A thread is being answered, resolved or deleted. */
  busy: boolean;
  onreply: (commentId: KnowledgeId, body: string) => Promise<boolean>;
  onresolve: (commentId: KnowledgeId, resolved: boolean) => void;
  /** Deletes a comment the user wrote. Answers whether it is gone. */
  ondelete: (commentId: KnowledgeId) => Promise<boolean>;
}
