import type { MessageKey } from '$lib/i18n/locales/en';
import type { RoutineStatus } from '$lib/ipc/bindings/RoutineStatus';
import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
import type { ShareBlock } from '$lib/ipc/bindings/ShareBlock';

/** Why "Share with team" is off for a local routine: each reason has its own line, shown as the button's tooltip. */
export const SHARE_BLOCK_TEXT: Record<ShareBlock, MessageKey> = {
  seed: 'routines.share.block.seed',
  pending: 'routines.share.block.pending',
  invalid: 'routines.share.block.invalid',
  noOrigin: 'routines.share.block.noOrigin',
  onTeam: 'routines.share.block.onTeam',
};

/** The statuses a team routine cannot be copied in, and why. */
export type CopyBlocked = 'pending' | 'invalid' | 'duplicate';

export const COPY_BLOCK_TEXT: Partial<Record<CopyBlocked, MessageKey>> = {
  pending: 'routines.team.copy.pending',
  invalid: 'routines.team.copy.invalid',
  duplicate: 'routines.team.copy.duplicate',
};

/** Why "Run now" is off for a routine that cannot start: a pending one never runs, not even by hand. */
export const RUN_BLOCK_TEXT: Partial<Record<RoutineStatus, MessageKey>> = {
  pending: 'routines.run.block.pending',
  invalid: 'routines.run.block.invalid',
  duplicate: 'routines.run.block.duplicate',
  paused: 'routines.run.block.paused',
  off: 'routines.run.block.team',
};

/** The team routine is switched on for this computer and not stopped by anything but its own schedule or folder. */
export function teamSwitchedOn(view: RoutineView): boolean {
  return view.teamOn && (view.status === 'active' || view.status === 'running' || view.status === 'paused' || view.status === 'pending');
}

/** The local routine a team routine was shared from: the one with the same id. */
export function originalOf(views: readonly RoutineView[], team: RoutineView): RoutineView | null {
  return views.find((view) => view.origin === 'local' && view.id === team.id) ?? null;
}

/** A repository key as people write it: `github.com/acme/app` for `git:github.com/acme/app`. */
export function repoName(key: string): string {
  return key.replace(/^git:/, '');
}

/**
 * Why the switch of a routine is off, or null when it works. A routine that waits for approval is never turned on by a click,
 * as a write approves what it writes. A team routine's switch is this computer's own choice, which a project routine can only
 * make from the project's folder.
 */
export function switchLock(view: RoutineView, projectOpen: boolean): MessageKey | null {
  if (view.origin === 'team') {
    return teamSwitchLock(view, projectOpen);
  }

  if (view.status === 'pending') {
    return 'routines.locked.pending';
  }

  return view.routine === null ? 'routines.locked.invalid' : null;
}

function teamSwitchLock(view: RoutineView, projectOpen: boolean): MessageKey | null {
  if (view.status === 'duplicate') {
    return 'routines.locked.duplicate';
  }

  if (view.routine === null) {
    return 'routines.locked.invalid';
  }

  const turningOn = !view.teamOn;

  return turningOn && view.teamRepo !== null && !projectOpen ? 'routines.locked.teamNoProject' : null;
}
