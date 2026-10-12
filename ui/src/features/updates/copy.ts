import type { UpdateInfo } from '$lib/ipc/bindings/UpdateInfo';
import type { UpdateState, UpdateFailureKind } from '$lib/stores/updates.svelte';
import { t, tn } from '$lib/i18n/i18n.svelte';
import { formatClock, formatLongDate, perLocale } from '$lib/utils/format';

/** Windows runs the installer, which quits the app and starts the new version itself. */
export function installerClosesApp(): string {
  return t('updates.installerCloses');
}

export function restartStopsAgents(): string {
  return t('updates.restartStopsAgents');
}

type During = 'check' | 'install';

/** What a restart costs while chats are working. */
export function restartWarning(chats: number): string {
  return tn('updates.restartWarning', chats);
}

/**
 * The release notes without a first heading that only repeats the announced version
 * ("## 0.2.0 (2026-10-08)"); the section above them already says it.
 */
export function releaseNotes(info: UpdateInfo): string {
  const notes = (info.notes ?? '').trim();
  const [first, ...rest] = notes.split('\n');
  const heading = /^ {0,3}#{1,6}\s+(.*)$/.exec(first);

  if (heading && heading[1].includes(info.version)) {
    return rest.join('\n').trim();
  }

  return notes;
}

const MEGABYTES = perLocale((locale) => new Intl.NumberFormat(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 }));
const KILOBYTES = perLocale((locale) => new Intl.NumberFormat(locale));

/** "1.4 MB"; sizes here are installer downloads, so MB is the unit that reads best. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) {
    return `${KILOBYTES().format(Math.max(1, Math.round(bytes / 1024)))} ${t('updates.unit.kb')}`;
  }

  return `${MEGABYTES().format(bytes / (1024 * 1024))} ${t('updates.unit.mb')}`;
}

/** The release date as the manifest gave it (RFC 3339), or `null` when absent or unreadable. */
export function formatReleaseDate(date: string | null): string | null {
  const parsed = date ? new Date(date) : null;

  return parsed && !Number.isNaN(parsed.getTime()) ? formatLongDate(parsed) : null;
}

/**
 * What went wrong, in plain words. A signature failure never explains itself further: the details
 * are not for the screen, and the outcome (nothing was installed) is what matters.
 */
export function failureMessage(reason: UpdateFailureKind, during: During): string {
  switch (reason) {
    case 'network':
      return during === 'check' ? t('updates.failure.network.check') : t('updates.failure.network.install');

    case 'verification':
      return t('updates.failure.verification');

    case 'manifest':
      return t('updates.failure.manifest');

    case 'installer':
      return t('updates.failure.installer');

    case 'busy':
      return t('updates.failure.busy');

    case 'stale':
      return t('updates.failure.stale');

    case 'unknown':
      return t('updates.failure.unknown');
  }
}

/** The one-line state of updates, for Settings. */
export function statusLine(state: UpdateState): string {
  switch (state.kind) {
    case 'idle':
      return t('updates.status.idle');

    case 'checking':
      return t('updates.status.checking');

    case 'upToDate':
      return t('updates.status.upToDate', { time: formatClock(state.checkedAt) });

    case 'notConfigured':
      return t('updates.status.notConfigured');

    case 'available':
      return t('updates.status.available', { version: state.info.version });

    case 'downloading':
      return t('updates.status.downloading', { version: state.info.version });

    case 'installing':
      return t('updates.status.installing', { version: state.info.version });

    case 'readyToRelaunch':
      return t('updates.status.readyToRelaunch', { version: state.info.version });

    case 'failed':
      return state.during === 'check' ? t('updates.status.failedCheck') : t('updates.status.failedInstall');
  }
}

/** How far a download is, for the progress label ("1.2 of 2.0 MB"); empty until bytes arrive. */
export function progressLabel(state: UpdateState): string {
  if (state.kind !== 'downloading' || !state.progress) {
    return '';
  }

  const { downloaded, total } = state.progress;

  return total ? t('updates.progress.of', { downloaded: formatBytes(downloaded), total: formatBytes(total) }) : formatBytes(downloaded);
}

/** The short label of the status-bar indicator; empty when there is nothing to show. */
export function indicatorLabel(state: UpdateState, fraction: number | null): string {
  switch (state.kind) {
    case 'available':
      return t('updates.indicator.available', { version: state.info.version });

    case 'downloading':
      return fraction === null ? t('updates.indicator.updating') : t('updates.indicator.updatingPercent', { percent: Math.round(fraction * 100) });

    case 'installing':
      return t('updates.indicator.installing');

    case 'readyToRelaunch':
      return t('updates.indicator.restart');

    case 'failed':
      return state.info ? t('updates.indicator.failed') : '';

    default:
      return '';
  }
}
