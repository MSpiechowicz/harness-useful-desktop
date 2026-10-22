import { t } from '$lib/i18n/i18n.svelte';
import type { AgentStartRequest } from '$lib/ipc/bindings/AgentStartRequest';
import { IpcFailure } from '$lib/ipc/transport';

import { harness } from './harness.svelte';
import type { ToastInput } from './toasts.svelte';

/** A start failure longer than this many characters is a sentence to read and act on, not a label. */
const LONG_START_FAILURE = 80;

/**
 * The toast of a chat that could not start. A folder the harness does not trust yet (Claude Code) offers to start
 * again trusting it once, which `trustOnce` runs: the app never trusts a folder unless the user picks that.
 */
export function startFailureToast(error: unknown, request: AgentStartRequest, trustOnce: () => void): ToastInput {
  const failure = IpcFailure.from(error);

  if (failure.code === 'untrusted' && !request.trustOnce) {
    const info = harness.get(request.harness)?.info;
    const name = info?.name ?? request.harness;
    const binary = info?.binaryName ?? request.harness;

    return {
      title: t('chat.toast.untrusted', { harness: name }),
      description: t('chat.toast.untrustedBody', { binary }),
      tone: 'warning',
      duration: 0,
      action: { label: t('chat.toast.trustOnce'), run: trustOnce },
    };
  }

  // A long reason names a fix to carry out (a folder to chmod): it stays until the user dismisses it.
  return { title: t('chat.toast.startFailed'), description: failure.message, tone: 'danger', duration: failure.message.length > LONG_START_FAILURE ? 0 : undefined };
}
