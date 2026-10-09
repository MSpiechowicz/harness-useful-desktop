import type { DiffScope } from '$lib/ipc/bindings/DiffScope';
import type { GitFileStatus } from '$lib/ipc/bindings/GitFileStatus';
import type { StatusEntry } from '$lib/ipc/bindings/StatusEntry';

export const SCOPES: readonly DiffScope[] = ['all', 'unstaged', 'staged', 'untracked'];

export const SCOPE_LABELS: Record<DiffScope, string> = { all: 'All', unstaged: 'Unstaged', staged: 'Staged', untracked: 'Untracked' };

/** Whether a changed path belongs under a scope tab. */
export function inScope(entry: StatusEntry, scope: DiffScope): boolean {
  switch (scope) {
    case 'all':
      return true;

    case 'staged':
      return entry.staged !== null;

    case 'unstaged':
      return entry.unstaged !== null && entry.unstaged !== 'untracked';

    case 'untracked':
      return entry.unstaged === 'untracked';
  }
}

/** Paths compare equal across `./` prefixes and Windows separators. */
export function normalizePath(path: string): string {
  return path.replaceAll('\\', '/').replace(/^\.\//, '');
}

/** True when the entry is one of the paths (a renamed file also matches its old name). */
export function matchesPaths(entry: StatusEntry, paths: ReadonlySet<string>): boolean {
  return paths.has(normalizePath(entry.path)) || (entry.origPath !== null && paths.has(normalizePath(entry.origPath)));
}

const LETTERS: Record<GitFileStatus, string> = {
  modified: 'M',
  added: 'A',
  deleted: 'D',
  renamed: 'R',
  copied: 'C',
  typeChanged: 'T',
  untracked: 'U',
  conflicted: '!',
  ignored: 'I',
};

const NAMES: Record<GitFileStatus, string> = {
  modified: 'Modified',
  added: 'Added',
  deleted: 'Deleted',
  renamed: 'Renamed',
  copied: 'Copied',
  typeChanged: 'Type changed',
  untracked: 'Untracked',
  conflicted: 'Conflicted',
  ignored: 'Ignored',
};

export function statusLetter(status: GitFileStatus): string {
  return LETTERS[status];
}

export function statusName(status: GitFileStatus): string {
  return NAMES[status];
}

export type StatusTone = 'success' | 'warning' | 'danger' | 'info' | 'neutral';

const TONES: Record<GitFileStatus, StatusTone> = {
  modified: 'warning',
  added: 'success',
  deleted: 'danger',
  renamed: 'info',
  copied: 'info',
  typeChanged: 'warning',
  untracked: 'success',
  conflicted: 'danger',
  ignored: 'neutral',
};

export function statusTone(status: GitFileStatus): StatusTone {
  return TONES[status];
}

/** The status a row shows for the scope in view; `all` prefers the working-tree change. */
export function displayStatus(entry: StatusEntry, scope: DiffScope): GitFileStatus {
  const staged = entry.staged;
  const unstaged = entry.unstaged;

  if (scope === 'staged' && staged) {
    return staged;
  }

  return unstaged ?? staged ?? 'modified';
}

/** How many changed paths each scope tab holds. */
export function scopeCounts(entries: readonly StatusEntry[]): Record<DiffScope, number> {
  return {
    all: entries.length,
    unstaged: entries.filter((entry) => inScope(entry, 'unstaged')).length,
    staged: entries.filter((entry) => inScope(entry, 'staged')).length,
    untracked: entries.filter((entry) => inScope(entry, 'untracked')).length,
  };
}
