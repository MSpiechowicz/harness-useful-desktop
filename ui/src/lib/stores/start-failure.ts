import { t, tn } from '$lib/i18n/i18n.svelte';
import type { AgentStartRequest } from '$lib/ipc/bindings/AgentStartRequest';
import { IpcFailure } from '$lib/ipc/transport';

import type { RestoredChat } from './agent-tabs';
import { harness } from './harness.svelte';
import type { ToastInput } from './toasts.svelte';

/** A start failure longer than this many characters is a sentence to read and act on, not a label. */
const LONG_START_FAILURE = 80;

/**
 * True for a refusal because the harness does not trust the folder yet, also when its settings file (which holds its
 * trust answers) cannot be read: either way a start that trusts the folder once may follow.
 */
export function isUntrusted(error: unknown): boolean {
  const { code } = IpcFailure.from(error);

  return code === 'untrusted' || code === 'untrustedUnreadable';
}

/** The last segment of a folder path, as the toasts name it. */
function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

/**
 * The toast of a chat that could not start. A folder the harness does not trust yet (Claude Code) offers to start
 * again trusting it once, which `trustOnce` runs (the caller's own flow again): the app never trusts a folder unless the
 * user picks that. That toast names the folder and goes away when another project becomes the active one.
 */
export function startFailureToast(error: unknown, request: AgentStartRequest, trustOnce: () => void): ToastInput {
  const failure = IpcFailure.from(error);

  if (isUntrusted(failure) && !request.trustOnce) {
    const info = harness.get(request.harness)?.info;
    const name = info?.name ?? request.harness;
    const binary = info?.binaryName ?? request.harness;

    return {
      title: t('chat.toast.untrusted', { harness: name, folder: folderName(request.project) }),
      description: failure.code === 'untrustedUnreadable' ? t('chat.toast.untrustedUnreadableBody', { harness: name }) : t('chat.toast.untrustedBody', { binary }),
      tone: 'warning',
      duration: 0,
      project: request.project,
      action: { label: t('chat.toast.trustOnce'), run: trustOnce },
    };
  }

  // A long reason names a fix to carry out (a folder to chmod): it stays until the user dismisses it.
  return { title: t('chat.toast.startFailed'), description: failure.message, tone: 'danger', duration: failure.message.length > LONG_START_FAILURE ? 0 : undefined };
}

/**
 * The one toast for last run's chats that stayed closed because their harness does not trust their folders yet: it
 * names the folders, and "Trust once" runs `trustOnce`, which reopens them trusting each folder once.
 */
export function untrustedReopenToast(chats: RestoredChat[], trustOnce: () => void): ToastInput {
  const harnesses = [...new Set(chats.map((chat) => harness.get(chat.harness)?.info.name ?? chat.harness))];
  const folders = [...new Set(chats.map((chat) => folderName(chat.project)))];

  return {
    title: tn('chat.toast.untrustedReopen', chats.length),
    description: t('chat.toast.untrustedReopenBody', { harness: harnesses.join(', '), folders: folders.join(', ') }),
    tone: 'warning',
    duration: 0,
    action: { label: t('chat.toast.trustOnce'), run: trustOnce },
  };
}
