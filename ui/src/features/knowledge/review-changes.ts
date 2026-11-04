import type { ChangeItem, ChangeSection } from '$lib/components/change-list';
import { statusLetter, statusName, statusTone } from '$lib/git/entries';
import { t } from '$lib/i18n/i18n.svelte';
import type { KnowledgeComment } from '$lib/ipc/bindings/KnowledgeComment';
import type { KnowledgeSectionDiff } from '$lib/ipc/bindings/KnowledgeSectionDiff';

/** How much a pending version changes: sections, added and removed lines. */
export function totalsOf(sections: readonly KnowledgeSectionDiff[]): { sections: number; additions: number; deletions: number } {
  return { sections: sections.length, additions: sections.reduce((sum, diff) => sum + diff.additions, 0), deletions: sections.reduce((sum, diff) => sum + diff.deletions, 0) };
}

/** A heading path as a row names it: the headings of the outline in order, without a "/" the list would read as a folder. */
export function headingLabel(heading: string): string {
  return heading.trim() === '' ? t('knowledge.sections.intro') : heading.replaceAll('/', '∕');
}

/** What names a section in a compare: its heading path and which of the sections that share it it is (counted from 1). */
export interface SectionRef {
  section: string;
  occurrence: number;
}

/** The server keeps a comment only on a section whose number and heading it can hold: it refuses a comment on one beyond these. */
export const MAX_COMMENT_OCCURRENCE = 1_000_000;
export const MAX_COMMENT_HEADING = 300;

/** Why no comment can be written on a section (its number or its heading is beyond what the server keeps a comment on), or null when one can. */
export function sectionCommentBlock(diff: Pick<KnowledgeSectionDiff, 'path' | 'occurrence'>): string | null {
  return diff.occurrence > MAX_COMMENT_OCCURRENCE || [...diff.path].length > MAX_COMMENT_HEADING ? t('knowledge.drafts.review.sectionNoComments') : null;
}

/** The key of a section: its number first, so that no heading (not even one that reads "Install#2") can be taken for another section of the same document. */
export function sectionKey(path: string, occurrence: number): string {
  return `${occurrence}:${path}`;
}

/** A key for each section, in order. */
export function sectionKeys(sections: readonly KnowledgeSectionDiff[]): string[] {
  return sections.map((diff) => sectionKey(diff.path, diff.occurrence));
}

/** The name a section goes by in a list: its heading, with its number after it for a heading that repeats. */
export function sectionLabel(path: string, occurrence: number): string {
  return occurrence > 1 ? `${headingLabel(path)} #${occurrence}` : headingLabel(path);
}

/** The row of a changed section: its status letter and tone as for a changed file, its counts and the comments left on it. */
export function changeItem(diff: KnowledgeSectionDiff, comments: number): ChangeItem {
  return {
    id: sectionKey(diff.path, diff.occurrence),
    label: sectionLabel(diff.path, diff.occurrence),
    letter: statusLetter(diff.status),
    letterLabel: statusName(diff.status),
    tone: statusTone(diff.status),
    additions: diff.additions,
    deletions: diff.deletions,
    comments,
  };
}

/** The section a saved comment is on. A comment an older server stored has no number: it is on the first section of its heading. */
export function savedSectionRef(comment: Pick<KnowledgeComment, 'section' | 'occurrence'>): SectionRef | null {
  return comment.section == null ? null : { section: comment.section, occurrence: comment.occurrence ?? 1 };
}

/** How many comments each section has, by section key: the saved threads that start there (replies belong to their thread) and the ones being written. */
export function commentCounts(saved: readonly KnowledgeComment[], drafts: readonly SectionRef[]): Map<string, number> {
  const counts = new Map<string, number>();
  const refs = [...saved.filter((comment) => comment.parentId == null).map(savedSectionRef), ...drafts];

  for (const ref of refs) {
    if (ref === null) {
      continue;
    }

    const key = sectionKey(ref.section, ref.occurrence);

    counts.set(key, (counts.get(key) ?? 0) + 1);
  }

  return counts;
}

/** The one heading of the list: every changed section. */
export function changeSections(sections: readonly KnowledgeSectionDiff[], counts: ReadonlyMap<string, number>): ChangeSection[] {
  return [{ id: 'changed', title: t('knowledge.drafts.changes.sections'), items: sections.map((diff) => changeItem(diff, counts.get(sectionKey(diff.path, diff.occurrence)) ?? 0)) }];
}

/**
 * The revision at which a reviewer last looked at the version, to compare with what it was then: the newest revision a comment of theirs was written on. The reviewer is the
 * user when the user's name is known, else any other than the author. Null when they wrote nothing yet.
 */
export function sinceRevisionOf(comments: readonly KnowledgeComment[], author: string | null, viewer: string | null): number | null {
  const same = (left: string | null | undefined, right: string | null | undefined): boolean => left != null && right != null && left.trim().toLowerCase() === right.trim().toLowerCase();
  const theirs = comments.filter((comment) => (viewer !== null ? same(comment.by, viewer) : comment.by != null && !same(comment.by, author)));

  return theirs.length === 0 ? null : Math.max(...theirs.map((comment) => comment.revision));
}
