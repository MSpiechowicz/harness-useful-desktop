import type { CommandName } from '../commands';
import { IpcFailure, type Transport } from '../transport';
import { defaultScenario, type Scenario } from './scenarios';

/**
 * Marker the CI build check greps for: it must never appear in `ui/dist`.
 * It is read at runtime so mock builds retain it through tree-shaking.
 */
export const MOCK_MARKER = '__HARNESS_MOCK_TRANSPORT__';

const LATENCY_MS = 30;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Transport backed by a scenario; deterministic apart from a small artificial latency. */
export function createMockTransport(scenario: Scenario = defaultScenario(), latencyMs: number = LATENCY_MS): Transport {
  (window as unknown as Record<string, unknown>)[MOCK_MARKER] = scenario.name;

  return {
    async invoke<K extends CommandName>(name: K, args: Parameters<Transport['invoke']>[1]) {
      await delay(latencyMs);

      const handler = scenario.handlers[name] as ((args: unknown) => unknown) | undefined;
      if (!handler) {
        throw IpcFailure.from({ code: 'internal', message: `${MOCK_MARKER}: no handler for ${name}` });
      }

      return (await handler(args)) as never;
    },
  };
}
