import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
import type { KnowledgeTransition } from '$lib/ipc/bindings/KnowledgeTransition';
import { knowledgeReview } from '$lib/stores/knowledgeReview.svelte';

import type { ReviewStep, Version } from './review-model';

/** The transition each button of the review row asks for. Replacing a file is no transition: it is an import. */
const TRANSITIONS: Record<Exclude<ReviewStep, 'replace'>, KnowledgeTransition> = {
  approve: 'approve',
  requestChanges: 'requestChanges',
  ask: 'ask',
  askAgain: 'ask',
  goLive: 'goLive',
  takeBack: 'takeBack',
};

/**
 * Makes the step a button asked for on the version of `doc` that was shown. A refusal says why in a toast. A verdict is about the diff the reviewer saw, so it is given that diff's
 * `revision` (see `decisionPin`) and does not go without one.
 */
export async function takeStep(doc: Pick<KnowledgeDocument, 'title'>, version: Version, step: Exclude<ReviewStep, 'replace'>, comment: string, seen: number | null = null): Promise<void> {
  const action = TRANSITIONS[step];
  const verdict = step === 'approve' || step === 'requestChanges';

  if (verdict && seen === null) {
    return;
  }

  await knowledgeReview.transition(
    { id: version.id, revision: verdict ? (seen as number) : version.revision, title: doc.title, author: version.author },
    action,
    action === 'requestChanges' ? comment : null,
  );
}
