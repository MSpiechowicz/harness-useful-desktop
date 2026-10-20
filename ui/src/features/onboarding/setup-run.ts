import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSetup } from '$lib/ipc/bindings/HarnessSetup';

import type { Companion, McpKind, SetupKey } from './setup-model';

/** What a run does for each item; the controller supplies them, so the order below is the only thing this file knows. */
export interface RunSteps {
  /** The harnesses in registry order. */
  harnesses: readonly HarnessSetup[];
  /** The items the user confirmed. */
  picked: ReadonlySet<SetupKey>;
  installCompanion: (companion: Companion) => Promise<boolean>;
  installSkills: (entry: HarnessSetup) => Promise<void>;
  registerMcp: (kind: McpKind, harness: HarnessId, companionOk: boolean) => Promise<void>;
}

/**
 * Runs the confirmed items. Everything that does not depend on something else runs at once. The two installers stay one
 * after the other: on Windows both edit the user's PATH, and two writers at a time could lose one of the entries. The
 * skills need neither installer, so they start right away. A harness's MCP entry waits only for its own server's
 * installer, and its entries go into the same file, so the memory server's waits for the dashboard's.
 *
 * A harness whose skills come from an installer script (OpenCode) is the exception to "the skills do not matter to the
 * MCP entries": the script rewrites the very file the entries are written to, so its entries wait until the script has
 * ended, however it ended.
 */
export async function runSetup(steps: RunSteps): Promise<void> {
  const { harnesses, picked } = steps;

  const dashboard = picked.has('dashboard') ? steps.installCompanion('dashboard') : Promise.resolve(true);
  const memory = dashboard.then(() => (picked.has('memory') ? steps.installCompanion('memory') : true));

  const skills = harnesses.map((entry) => ({ entry, run: picked.has(`skills:${entry.harness}`) ? steps.installSkills(entry) : null }));

  await Promise.all([
    dashboard,
    memory,
    ...skills.flatMap(({ run }) => (run ? [run] : [])),
    ...skills.map(async ({ entry, run }) => {
      if (run && entry.skills.type === 'script') {
        await run.catch(() => {});
      }

      if (picked.has(`mcp:${entry.harness}`)) {
        await steps.registerMcp('mcp', entry.harness, await dashboard);
      }
      if (picked.has(`memoryMcp:${entry.harness}`)) {
        await steps.registerMcp('memoryMcp', entry.harness, await memory);
      }
      // The entries go into the same file one after the other. The app's own needs no installer.
      if (picked.has(`desktopMcp:${entry.harness}`)) {
        await steps.registerMcp('desktopMcp', entry.harness, true);
      }
    }),
  ]);
}
