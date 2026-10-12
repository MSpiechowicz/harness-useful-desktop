import { t } from '$lib/i18n/i18n.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { ipc } from './client';
import { IpcFailure } from './transport';

/** Shows a "Reveal in file manager" request to the OS; failures surface as a toast. */
export async function revealPath(path: string): Promise<void> {
  try {
    await ipc.invoke('reveal_path', { path });
  } catch (error) {
    toasts.push({ title: t('components.action.revealFailed'), description: IpcFailure.from(error).message, tone: 'danger' });
  }
}

/**
 * Opens an https link in the default browser: a project or harness site, or any site once the user allows it in
 * Settings. Failures surface as a toast; a refused link offers that setting.
 */
export async function openExternal(url: string): Promise<void> {
  try {
    await ipc.invoke('open_external', { url });
  } catch (error) {
    const failure = IpcFailure.from(error);

    if (failure.code === 'forbidden' && prefs.value?.openAnyLink) {
      toasts.push({ title: t('components.action.linkRefused'), description: t('components.action.httpsOnly'), tone: 'warning' });
    } else if (failure.code === 'forbidden') {
      toasts.push({
        title: t('components.action.linkRefused'),
        description: t('components.action.sitesOnly'),
        tone: 'warning',
        action: { label: t('components.action.openSettings'), run: viewLinkSetting },
      });
    } else {
      toasts.push({ title: t('components.action.linkFailed'), description: failure.message, tone: 'danger' });
    }
  }
}

/** Opens Settings scrolled to Privacy & security, where the link setting lives. */
function viewLinkSetting(): void {
  viewState.settingsTarget = 'privacy';
  nav.go('settings');
}

/** Starts or stops the native launch sound; it is decoration, so a failure stays silent. */
export async function launchSound(play: boolean): Promise<void> {
  try {
    await ipc.invoke('launch_sound', { play });
  } catch {
    // No sound is fine.
  }
}
