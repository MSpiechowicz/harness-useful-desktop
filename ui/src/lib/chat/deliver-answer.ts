import { t } from '$lib/i18n/i18n.svelte';
import type { UiOutcome } from '$lib/ipc/bindings/UiOutcome';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

/** Sends an answer. True when the backend refused it: the agent is still waiting, so its dialog has to come back. */
export async function deliverAnswer(
  transport: Transport,
  key: string,
  id: string,
  outcome: UiOutcome,
  notify: (toast: { title: string; description: string; tone: 'danger' }) => void,
): Promise<boolean> {
  try {
    await transport.invoke('agent_ui_respond', { key, response: { id, outcome } });

    return false;
  } catch (error) {
    const failure = IpcFailure.from(error);

    notify({ title: t('chat.toast.noAnswer'), description: failure.message, tone: 'danger' });

    return failure.code === 'invalidInput';
  }
}
