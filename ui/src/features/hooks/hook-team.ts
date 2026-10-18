import type { MessageKey } from '$lib/i18n/locales/en';
import type { HookView } from '$lib/ipc/bindings/HookView';
import type { ShareBlock } from '$lib/ipc/bindings/ShareBlock';

/** Why "Share with team" is off for a local hook: each reason has its own line, shown as the button's tooltip. */
export const SHARE_BLOCK_TEXT: Record<ShareBlock, MessageKey> = {
  seed: 'hooks.share.block.seed',
  pending: 'hooks.share.block.pending',
  invalid: 'hooks.share.block.invalid',
  noOrigin: 'hooks.share.block.noOrigin',
  onTeam: 'hooks.share.block.onTeam',
};

/** The statuses a team hook cannot be copied in, and why. */
export type CopyBlocked = 'pending' | 'invalid' | 'duplicate';

export const COPY_BLOCK_TEXT: Partial<Record<CopyBlocked, MessageKey>> = {
  pending: 'hooks.team.copy.pending',
  invalid: 'hooks.team.copy.invalid',
  duplicate: 'hooks.team.copy.duplicate',
};

export function isTeam(view: HookView): boolean {
  return view.origin === 'team';
}

/** The team hook is switched on for this computer: the team did not ship it off and the local switch is not off either. */
export function teamSwitchedOn(view: HookView): boolean {
  return view.hook !== null && view.hook.enabled && !view.teamDisabled && view.status !== 'pending' && view.status !== 'invalid' && view.status !== 'duplicate';
}

/** Why the switch of a hook is off, or null when it works. A hook that waits for approval or cannot run is never turned on by a click. */
export function switchLock(view: HookView): MessageKey | null {
  if (view.origin === 'team') {
    if (view.status === 'duplicate') {
      return 'hooks.locked.duplicate';
    }

    if (view.status === 'pending') {
      return 'hooks.locked.teamPending';
    }

    if (view.hook === null) {
      return 'hooks.locked.invalid';
    }

    return view.hook.enabled ? null : 'hooks.locked.teamOff';
  }

  if (view.status === 'pending') {
    return 'hooks.locked.pending';
  }

  return view.hook === null ? 'hooks.locked.invalid' : null;
}

/** The local hook a team hook was shared from: the one with the same id. */
export function originalOf(views: readonly HookView[], team: HookView): HookView | null {
  return views.find((view) => view.origin === 'local' && view.id === team.id) ?? null;
}

/** A repository key as people write it: `github.com/acme/app` for `git:github.com/acme/app`. */
export function repoName(key: string): string {
  return key.replace(/^git:/, '');
}
