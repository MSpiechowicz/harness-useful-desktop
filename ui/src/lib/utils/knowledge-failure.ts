import { t } from '$lib/i18n/i18n.svelte';
import { IpcFailure } from '$lib/ipc/transport';

/** What a failed read of a document says: the remote database being unreachable in the app's own words, anything else as the server said it. */
export function readFailureText(error: unknown): string {
  const failure = IpcFailure.from(error);

  return failure.code === 'unreachable' ? t('knowledge.offline.document') : failure.message;
}
