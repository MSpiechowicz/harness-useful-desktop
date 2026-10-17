import { ecosystem } from '$lib/stores/ecosystem.svelte';

/**
 * Whether the latest Harness Memory release is newer than the installed one, read fresh from the installed binary and GitHub.
 * The Ecosystem page decides "update available" the same way. A source build or a development build (`1.0.1-dev`) never
 * has a newer release, and neither does a lookup that failed: with nothing to upgrade to, nothing is offered.
 */
export async function memoryUpgradeAvailable(): Promise<boolean> {
  await ecosystem.loadWithReleases(true);

  return ecosystem.state === 'ready' && ecosystem.hasUpdate('memory');
}
