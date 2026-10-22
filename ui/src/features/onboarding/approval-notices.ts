import { readJson, writeJson } from '$lib/utils/persisted';

/** Whose "tools run without asking" notice the approval step shows. */
export type ApprovalNotice = 'omp' | 'opencode' | 'claude';

const KEY = 'harness:approval-notices-seen';
const NOTICES: readonly ApprovalNotice[] = ['omp', 'opencode', 'claude'];

function isNoticeList(value: unknown): value is ApprovalNotice[] {
  return Array.isArray(value) && value.every((entry) => NOTICES.includes(entry as ApprovalNotice));
}

/**
 * The notices already shown, remembered per notice so seeing one never hides another. The saved preference is one
 * flag from before there was more than one notice: it means the OMP one. The others are kept in the webview's
 * storage (a per-device convenience), so a lost record only shows a notice once more.
 */
export function seenNotices(legacySeen: boolean): Set<ApprovalNotice> {
  const seen = new Set(readJson<ApprovalNotice[]>(KEY, [], isNoticeList));

  if (legacySeen) {
    seen.add('omp');
  }

  return seen;
}

export function rememberNotices(notices: Iterable<ApprovalNotice>): void {
  const seen = new Set(readJson<ApprovalNotice[]>(KEY, [], isNoticeList));

  for (const notice of notices) {
    seen.add(notice);
  }

  writeJson(KEY, [...seen]);
}
