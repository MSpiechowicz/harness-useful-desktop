import { type MessageKey, t } from '$lib/i18n/i18n.svelte';
import type { KnowledgeTeamApproval } from '$lib/ipc/bindings/KnowledgeTeamApproval';
import { relativeTime } from '$lib/utils/format';

/** Why the team approval switch cannot be used: each is said in a grey line under the row. */
export type ApprovalBlock = 'noRemote' | 'unsupported' | 'unreachable' | 'readOnly' | 'noName';

/** What decides whether the switch may be used. `supported` and `writable` are null while they are not known, and a switch is not blocked by what is unknown. */
export interface ApprovalState {
  /** A team database is saved. */
  remoteSaved: boolean;
  /** The installed Harness Knowledge has team approval. */
  supported: boolean | null;
  /** The team database cannot be reached now. */
  unreachable: boolean;
  /** The role may change the setting. */
  writable: boolean | null;
  /** The user has no git `user.name`, which the team database needs to say who switched it. */
  noName?: boolean;
}

const WHY: Record<ApprovalBlock, MessageKey> = {
  noRemote: 'settings.knowledgeApproval.why.noRemote',
  unsupported: 'settings.knowledgeApproval.why.unsupported',
  unreachable: 'settings.knowledgeApproval.why.unreachable',
  readOnly: 'settings.knowledgeApproval.why.readOnly',
  noName: 'settings.knowledgeApproval.why.noName',
};

/** The words of the line that says why the switch is greyed out. */
export function approvalWhy(block: ApprovalBlock): MessageKey {
  return WHY[block];
}

/** The first thing that keeps the switch from working, in the order the user can put it right: connect, update, wait for the database, ask for a role that writes, then set a git name. */
export function approvalBlock(state: ApprovalState): ApprovalBlock | null {
  if (!state.remoteSaved) {
    return 'noRemote';
  }

  if (state.supported === false) {
    return 'unsupported';
  }

  if (state.unreachable) {
    return 'unreachable';
  }

  if (state.writable === false) {
    return 'readOnly';
  }

  if (state.noName) {
    return 'noName';
  }

  return null;
}

/**
 * Who last switched team approval and when, as one grey line ("Turned on by Grace Hopper, 2 days ago"): null while the team database does not say who.
 * `you` is whether that person is the user, who reads "by you".
 */
export function changedLine(value: Pick<KnowledgeTeamApproval, 'approval' | 'changedBy' | 'changedAt'> | null, you: (name: string) => boolean | null): string | null {
  const name = value?.changedBy?.trim();

  if (!value || !name) {
    return null;
  }

  const when = value.changedAt ? relativeTime(value.changedAt) : t('settings.knowledgeApproval.changed.earlier');

  if (you(name) === true) {
    return t(value.approval ? 'settings.knowledgeApproval.changed.on.you' : 'settings.knowledgeApproval.changed.off.you', { when });
  }

  return t(value.approval ? 'settings.knowledgeApproval.changed.on' : 'settings.knowledgeApproval.changed.off', { name, when });
}
