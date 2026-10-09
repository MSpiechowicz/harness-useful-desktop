import { toasts } from '$lib/stores/toasts.svelte';

import { ipc } from './client';
import { IpcFailure } from './transport';

/** Shows a "Reveal in file manager" request to the OS; failures surface as a toast. */
export async function revealPath(path: string): Promise<void> {
  try {
    await ipc.invoke('reveal_path', { path });
  } catch (error) {
    toasts.push({ title: 'Could not reveal the file', description: IpcFailure.from(error).message, tone: 'danger' });
  }
}

/** Opens an allow-listed https link in the default browser; failures surface as a toast. */
export async function openExternal(url: string): Promise<void> {
  try {
    await ipc.invoke('open_external', { url });
  } catch (error) {
    toasts.push({ title: 'Could not open the link', description: IpcFailure.from(error).message, tone: 'danger' });
  }
}
