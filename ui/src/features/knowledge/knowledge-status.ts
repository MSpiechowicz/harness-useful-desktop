import CircleCheck from '@lucide/svelte/icons/circle-check';
import Clock from '@lucide/svelte/icons/clock';
import Eye from '@lucide/svelte/icons/eye';
import FilePenLine from '@lucide/svelte/icons/file-pen-line';
import GitPullRequestArrow from '@lucide/svelte/icons/git-pull-request-arrow';
import MessageSquareWarning from '@lucide/svelte/icons/message-square-warning';

import type { IconComponent } from '$lib/components/icon';
import { type MessageKey, t } from '$lib/i18n/i18n.svelte';
import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
import type { KnowledgeStatusFilter } from '$lib/ipc/bindings/KnowledgeStatusFilter';
import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';

import { type IsViewer, mineOf } from './review-model';

/** What a document is to people: a live one, a draft, one that waits for approval, one a reviewer wants changed, or a live one with an update that is none of these yet. */
export type DocState = 'live' | 'draft' | 'waiting' | 'changesRequested' | 'pending';

/** How a state's badge looks: the label, the tone, the icon and whether it is only outlined. */
export interface StatusLook {
  label: string;
  tone: 'neutral' | 'success' | 'warning' | 'danger' | 'info';
  icon: IconComponent;
  outline: boolean;
}

interface StateLook {
  label: MessageKey;
  tone: StatusLook['tone'];
  icon: IconComponent;
}

const LOOKS: Record<DocState, StateLook> = {
  live: { label: 'knowledge.status.live', tone: 'success', icon: CircleCheck },
  draft: { label: 'knowledge.status.draft', tone: 'neutral', icon: FilePenLine },
  waiting: { label: 'knowledge.status.waiting', tone: 'warning', icon: Clock },
  changesRequested: { label: 'knowledge.status.changesRequested', tone: 'danger', icon: MessageSquareWarning },
  pending: { label: 'knowledge.status.pending', tone: 'info', icon: GitPullRequestArrow },
};

/** The state a row shows. A live document with a pending version shows that version's trouble (changes were asked for) or says it has an update. */
export function stateOf(doc: Pick<KnowledgeDocument, 'status' | 'pending'>): DocState {
  if (doc.pending) {
    return doc.pending.status === 'changesRequested' ? 'changesRequested' : 'pending';
  }

  return doc.status;
}

/**
 * The badge of a row. A document that waits for approval says what it is to the viewer: any teammate who is
 * not among its authors may approve it ("Ready to review"), one of the authors waits for somebody else ("Waiting for approval", outlined), and a viewer whose name is
 * not known, or a row that names no authors, gets the plain waiting badge.
 */
export function lookOf(doc: Pick<KnowledgeDocument, 'status' | 'pending' | 'authors'>, is: IsViewer = (name) => knowledgeViewer.is(name)): StatusLook {
  const look = LOOKS[stateOf(doc)];
  const badge: StatusLook = { label: t(look.label), tone: look.tone, icon: look.icon, outline: false };

  if (doc.status !== 'waiting' || doc.pending || !doc.authors || doc.authors.length === 0) {
    return badge;
  }

  const mine = mineOf({ author: null, authors: doc.authors, status: 'waiting' }, [], is);

  if (mine === true) {
    return { ...badge, outline: true };
  }

  return mine === false ? { ...badge, label: t('knowledge.status.readyToReview'), icon: Eye } : badge;
}

/** The filter choices of the State filter, in the order of the menu: the first is "All". */
export const STATUS_CHOICES: readonly (KnowledgeStatusFilter | 'all')[] = ['all', 'live', 'draft', 'waiting', 'changesRequested', 'pending'];

export const STATUS_CHOICE_LABELS = {
  all: 'memory.filter.all',
  live: 'knowledge.status.live',
  draft: 'knowledge.filter.status.drafts',
  waiting: 'knowledge.status.waiting',
  changesRequested: 'knowledge.status.changesRequested',
  pending: 'knowledge.status.pending',
} as const satisfies Record<KnowledgeStatusFilter | 'all', MessageKey>;
