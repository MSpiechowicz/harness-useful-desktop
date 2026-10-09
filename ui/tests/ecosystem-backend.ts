import { vi, type MockInstance } from 'vitest';

import type { CommandArgs, CommandName } from '$lib/ipc/commands';
import { ipc } from '$lib/ipc/client';
import { ecosystemHandlers, type EcosystemSwitches } from '$lib/ipc/mock/ecosystem-handlers';
import { defaultScenario } from '$lib/ipc/mock/scenarios';
import { createMockTransport } from '$lib/ipc/mock/transport';
import { dashboard } from '$lib/stores/dashboard.svelte';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { extensionsView } from '$lib/stores/views.svelte';

export interface Backend {
  /** Arguments of every call to one command, in order. */
  calls: <K extends CommandName>(name: K) => CommandArgs<K>[];
  spy: MockInstance;
}

/**
 * Routes the app's shared `ipc` to a fresh in-memory backend (so extension changes of one test
 * never reach the next) and forgets what the ecosystem stores learned from the previous one.
 */
export function freshBackend(switches: Partial<EcosystemSwitches> = {}): Backend {
  const scenario = defaultScenario();
  scenario.handlers = { ...scenario.handlers, ...ecosystemHandlers({ dashboard: 'ok', ecosystem: 'ok', ...switches }) };

  const transport = createMockTransport(scenario, 0);
  const log: [CommandName, unknown][] = [];

  const spy = vi.spyOn(ipc, 'invoke').mockImplementation((name, args) => {
    log.push([name, args]);

    return transport.invoke(name, args);
  });

  dashboard.reset();
  ecosystem.reset();
  extensionsView.sync(null, null);

  return { spy, calls: (name) => log.filter(([called]) => called === name).map(([, args]) => args as never) };
}
