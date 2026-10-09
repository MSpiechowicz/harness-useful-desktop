import type { UpdateInfo } from '$lib/ipc/bindings/UpdateInfo';
import type { UpdateState, UpdateFailureKind } from '$lib/stores/updates.svelte';
import { formatClock, formatLongDate } from '$lib/utils/format';

/** Windows runs the installer, which quits the app and starts the new version itself. */
export const INSTALLER_CLOSES_APP = 'The installer will close Harness Desktop and reopen it when it is done.';

export const RESTART_STOPS_AGENTS = 'Running agents stop when the app restarts.';

type During = 'check' | 'install';

/** What a restart costs while chats are working. */
export function restartWarning(chats: number): string {
  return `${chats} running ${chats === 1 ? 'chat' : 'chats'} will be stopped, and any answer in progress is lost.`;
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

/** "1.4 MB"; sizes here are installer downloads, so MB is the unit that reads best. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) {
    return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  }

  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
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
      return during === 'check'
        ? 'Could not reach the update server. Check your connection and try again.'
        : 'The download was interrupted. Nothing was changed; try again.';

    case 'verification':
      return 'The update could not be verified and was not installed.';

    case 'manifest':
      return 'The update information was incomplete or has nothing for this system. Try again later.';

    case 'installer':
      return 'The installer did not finish, or the permission prompt was cancelled. Nothing was changed.';

    case 'busy':
      return 'An update is already being installed.';

    case 'stale':
      return 'That update is no longer available. Check again.';

    case 'unknown':
      return 'Something went wrong while updating. Nothing was changed.';
  }
}

/** The one-line state of updates, for Settings. */
export function statusLine(state: UpdateState): string {
  switch (state.kind) {
    case 'idle':
      return 'Not checked yet this session.';

    case 'checking':
      return 'Checking for updates…';

    case 'upToDate':
      return `You are up to date. Checked at ${formatClock(state.checkedAt)}.`;

    case 'notConfigured':
      return 'This build cannot check for updates.';

    case 'available':
      return `Version ${state.info.version} is available.`;

    case 'downloading':
      return `Downloading version ${state.info.version}…`;

    case 'installing':
      return `Installing version ${state.info.version}…`;

    case 'readyToRelaunch':
      return `Version ${state.info.version} is installed. Restart to finish.`;

    case 'failed':
      return state.during === 'check' ? 'Could not check for updates.' : 'The update was not installed.';
  }
}

/** How far a download is, for the progress label ("1.2 of 2.0 MB"); empty until bytes arrive. */
export function progressLabel(state: UpdateState): string {
  if (state.kind !== 'downloading' || !state.progress) {
    return '';
  }

  const { downloaded, total } = state.progress;

  return total ? `${formatBytes(downloaded)} of ${formatBytes(total)}` : formatBytes(downloaded);
}

/** The short label of the status-bar indicator; empty when there is nothing to show. */
export function indicatorLabel(state: UpdateState, fraction: number | null): string {
  switch (state.kind) {
    case 'available':
      return `Update ${state.info.version}`;

    case 'downloading':
      return fraction === null ? 'Updating…' : `Updating ${Math.round(fraction * 100)}%`;

    case 'installing':
      return 'Installing…';

    case 'readyToRelaunch':
      return 'Restart to update';

    case 'failed':
      return state.info ? 'Update failed' : '';

    default:
      return '';
  }
}
