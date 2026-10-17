import { vi } from 'vitest';

import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

/** Starts the app on the mock backend with motion off and one project open: what the tests of a view that reads from the backend begin with. */
export async function startMockApp(path: string): Promise<void> {
  vi.stubEnv('VITE_TRANSPORT', 'mock');

  await prefs.load();
  await prefs.update({ motion: 'off' });
  await Promise.all([harness.load(), project.load()]);
  await project.open(path);
}
