import type { DiffLine } from '../bindings/DiffLine';
import type { FileDiff } from '../bindings/FileDiff';
import type { GitFileStatus } from '../bindings/GitFileStatus';
import type { KnowledgeSectionDiff } from '../bindings/KnowledgeSectionDiff';

/** The lines of one changed section: unchanged lines around the removed and the added ones. */
export interface SectionChange {
  before?: string[];
  removed?: string[];
  added?: string[];
  after?: string[];
}

/** One section as the Changes view shows a file: a single hunk with the numbers of each side, the way `docs compare` prints it. */
export function diffOf(path: string, change: SectionChange, status: GitFileStatus = 'modified'): FileDiff {
  const lines: DiffLine[] = [];
  let oldNo = 1;
  let newNo = 1;

  const push = (texts: string[] | undefined, kind: DiffLine['kind']) => {
    for (const text of texts ?? []) {
      lines.push({ kind, oldNo: kind === 'add' ? null : oldNo, newNo: kind === 'remove' ? null : newNo, text });
      oldNo += kind === 'add' ? 0 : 1;
      newNo += kind === 'remove' ? 0 : 1;
    }
  };

  push(change.before, 'context');
  push(change.removed, 'remove');
  push(change.added, 'add');
  push(change.after, 'context');

  const additions = change.added?.length ?? 0;
  const deletions = change.removed?.length ?? 0;
  const context = (change.before?.length ?? 0) + (change.after?.length ?? 0);

  return {
    path,
    oldPath: null,
    status,
    staged: false,
    isBinary: false,
    truncated: false,
    additions,
    deletions,
    hunks: [{ header: `@@ -1,${context + deletions} +1,${context + additions} @@`, oldStart: 1, oldLines: context + deletions, newStart: 1, newLines: context + additions, lines }],
  };
}

/** The sections of a compare with their `occurrence`: which of the sections that share a heading path each one is, counted from 1 the way `docs compare` counts them. */
export function numbered(diffs: FileDiff[]): KnowledgeSectionDiff[] {
  const seen = new Map<string, number>();

  return diffs.map((diff) => {
    const occurrence = (seen.get(diff.path) ?? 0) + 1;

    seen.set(diff.path, occurrence);

    return { ...diff, occurrence };
  });
}

/** A new document compares with nothing: every line of every section is added. */
export function newDocumentDiffs(sections: { heading: string; lines: string[] }[]): KnowledgeSectionDiff[] {
  return numbered(sections.map((section) => diffOf(section.heading, { added: section.lines }, 'added')));
}

/** What a pending update changes in a live document: two sections edited, one added and one removed. */
export function pendingChanges(): KnowledgeSectionDiff[] {
  return numbered([
    diffOf('Install', { before: ['Run the installer from the release page.'], removed: ['Set the retry limit to 3.'], added: ['Set the retry limit to 5.', 'Back off by one hour between tries.'], after: ['Restart the app.'] }),
    diffOf('Configuration', { before: ['Settings live in the config folder.'], added: ['Set `BILLING_RETRIES` to override the limit.'] }),
    diffOf('Rollback', { added: ['Switch the feature flag off.', 'Page the on-call engineer.', 'Replay the failed charges.'] }, 'added'),
    diffOf('Troubleshooting', { removed: ['If a charge hangs, restart the worker.', 'Check the queue length first.'] }, 'deleted'),
  ]);
}
