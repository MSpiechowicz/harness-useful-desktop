import type { Handlers } from './types';
import type { MockHooks } from './hooks-state';

type HookCommands = 'hooks_list' | 'hook_save' | 'hook_delete' | 'hook_approve' | 'hook_reject' | 'hooks_family' | 'hooks_family_apply' | 'hooks_delete_all' | 'hooks_listen' | 'hooks_share' | 'hooks_team_remove' | 'hooks_team_refresh' | 'hooks_team_copy' | 'hooks_team_enable';

/** The hooks folder of the mock backend: `hooks_list` and the commands that change it, like the real ones, without a folder. */
export function hooksHandlers(hooks: MockHooks): Pick<Handlers, HookCommands> {
  return {
    hooks_list: ({ project }) => hooks.list(project),
    hook_save: ({ draft }) => hooks.save(draft),
    hook_delete: ({ id, sha256 }) => hooks.delete(id, sha256),
    hook_approve: ({ id, sha256 }) => hooks.approve(id, sha256),
    hook_reject: ({ id, sha256 }) => hooks.reject(id, sha256),
    hooks_family: () => hooks.family(),
    hooks_family_apply: ({ seeds }) => hooks.applyFamily(seeds),
    hooks_delete_all: ({ project }) => hooks.deleteAll(project ?? null),
    hooks_listen: ({ onEvent }) => hooks.listen(() => onEvent({ kind: 'changed' })),
    hooks_share: ({ id, sha256 }) => hooks.share(id, sha256),
    hooks_team_remove: ({ id, sha256 }) => hooks.teamRemove(id, sha256),
    hooks_team_refresh: ({ project }) => hooks.teamRefresh(project ?? null),
    hooks_team_copy: ({ id, sha256, project }) => hooks.teamCopy(id, sha256, project ?? null),
    hooks_team_enable: ({ id, enabled }) => hooks.teamEnable(id, enabled),
  };
}
