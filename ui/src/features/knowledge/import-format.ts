import Check from '@lucide/svelte/icons/check';
import CircleAlert from '@lucide/svelte/icons/circle-alert';
import Equal from '@lucide/svelte/icons/equal';
import RefreshCw from '@lucide/svelte/icons/refresh-cw';
import SkipForward from '@lucide/svelte/icons/skip-forward';

import type { IconComponent } from '$lib/components/icon';
import { type MessageKey, t } from '$lib/i18n/i18n.svelte';
import type { KnowledgeFileStatus } from '$lib/ipc/bindings/KnowledgeFileStatus';
import type { KnowledgeSkipReason } from '$lib/ipc/bindings/KnowledgeSkipReason';

/** The results of one file, in the order the chips list them. */
export const FILE_STATUSES: readonly KnowledgeFileStatus[] = ['added', 'updated', 'unchanged', 'skipped', 'failed'];

export const STATUS_LABELS = {
  added: 'knowledge.import.status.added',
  updated: 'knowledge.import.status.updated',
  unchanged: 'knowledge.import.status.unchanged',
  skipped: 'knowledge.import.status.skipped',
  failed: 'knowledge.import.status.failed',
} as const satisfies Record<KnowledgeFileStatus, MessageKey>;

export const STATUS_TONES = { added: 'success', updated: 'accent', unchanged: 'neutral', skipped: 'warning', failed: 'danger' } as const satisfies Record<KnowledgeFileStatus, string>;

export const STATUS_ICONS: Record<KnowledgeFileStatus, IconComponent> = { added: Check, updated: RefreshCw, unchanged: Equal, skipped: SkipForward, failed: CircleAlert };

export const SKIP_REASONS = {
  missing: 'knowledge.import.skip.missing',
  symlink: 'knowledge.import.skip.symlink',
  notFileOrFolder: 'knowledge.import.skip.notFileOrFolder',
  unreadable: 'knowledge.import.skip.unreadable',
  tooMany: 'knowledge.import.skip.tooMany',
} as const satisfies Record<KnowledgeSkipReason, MessageKey>;

/** The file name of a path, whichever way the system writes it. */
export function baseName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

/** The folder a path is in, without its file name: the quiet line under a name. */
export function folderOf(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);

  return parts.slice(0, -1).join('/');
}

/** "About 40 s left", and minutes from a minute and a half on. */
export function etaText(seconds: number): string {
  if (seconds < 90) {
    return t('knowledge.import.eta.seconds', { seconds });
  }

  return t('knowledge.import.eta.minutes', { minutes: Math.round(seconds / 60) });
}
