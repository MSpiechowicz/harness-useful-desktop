import { prefs } from '$lib/stores/prefs.svelte';

/** Turns the setting "Keep running in the tray when the window is closed" on. */
export async function turnOnTray(): Promise<void> {
  await prefs.update({ keepRunningInTray: true });
}
