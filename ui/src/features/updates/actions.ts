import { nav } from '$lib/stores/nav.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

/** Opens Settings scrolled to the Updates section. */
export function viewUpdates(): void {
  viewState.settingsTarget = 'updates';
  nav.go('settings');
}
