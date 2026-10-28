import type { BranchRow } from '$lib/ipc/bindings/BranchRow';
import type { GitStatus } from '$lib/ipc/bindings/GitStatus';

/** What tells a listed branch from another: the same name can be local and remote-only. */
export function rowKey(row: BranchRow): string {
  return row.remoteOnly + row.name;
}

/** What in a git status says the branch moved: the head, the branch, its upstream and counts, and the conflicted files. */
export function signatureOf(status: GitStatus): string {
  const conflicted = status.entries.filter((entry) => entry.staged === 'conflicted' || entry.unstaged === 'conflicted').map((entry) => entry.path);
  const untracked = status.entries.filter((entry) => entry.unstaged === 'untracked').length;

  return [status.head, status.branch, status.upstream, status.ahead, status.behind, status.entries.length, untracked, conflicted.join('\0')].join('|');
}
