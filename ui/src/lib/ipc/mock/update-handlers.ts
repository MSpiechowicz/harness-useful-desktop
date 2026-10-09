import type { UpdateInfo } from '../bindings/UpdateInfo';
import type { UpdateProgress } from '../bindings/UpdateProgress';
import { IpcFailure } from '../transport';
import available from './fixtures/updates/update_available.json';
import progress from './fixtures/updates/progress.json';
import type { Handlers } from './types';

type UpdateCommands = 'update_check' | 'update_install' | 'app_relaunch';

/**
 * What the mock update backend plays. The browser preview picks one with `?update=<name>`;
 * the default shows an available update, which is the most interesting state.
 */
export const UPDATE_SCENARIOS = ['available', 'current', 'unconfigured', 'network', 'forbidden'] as const;
export type UpdateScenario = (typeof UPDATE_SCENARIOS)[number];

const DEFAULT_SCENARIO: UpdateScenario = 'available';

/** Pause between two recorded download chunks, and the time "installing" takes, at full pace. */
const CHUNK_MS = 70;
const INSTALL_MS = 900;

/** Reads the scenario from the page URL (browser preview only); unknown names fall back to the default. */
export function scenarioFromLocation(search: string = window.location.search): UpdateScenario {
  const name = new URLSearchParams(search).get('update');

  return UPDATE_SCENARIOS.find((scenario) => scenario === name) ?? DEFAULT_SCENARIO;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function announced(): UpdateInfo {
  return structuredClone(available) as UpdateInfo;
}

/** The recorded progress, with the pauses of a real download. */
async function replayProgress(onProgress: (event: UpdateProgress) => void, pace: number, stopAt?: number): Promise<void> {
  const events = progress as UpdateProgress[];

  for (const event of stopAt === undefined ? events : events.slice(0, stopAt)) {
    onProgress(event);

    if (pace > 0) {
      await delay((event.phase === 'installing' ? INSTALL_MS : CHUNK_MS) * pace);
    }
  }
}

/**
 * In-memory update backend. Like the real one, an install needs an update that a check found, and
 * a failed verification happens only after the download has been going on for a while.
 */
export function updateHandlers(pace: () => number, scenario: () => UpdateScenario = scenarioFromLocation): Pick<Handlers, UpdateCommands> {
  let found = false;

  /** Tests do not wait for downloads. */
  const speed = () => (import.meta.env.MODE === 'test' ? 0 : pace());

  return {
    update_check: async () => {
      await delay(450 * speed());

      switch (scenario()) {
        case 'unconfigured':
          throw IpcFailure.from({ code: 'unsupported', message: 'Updates are not configured for this build.' });

        case 'network':
          throw IpcFailure.from({ code: 'timeout', message: 'cannot check for updates: request timed out' });

        case 'current':
          return null;

        case 'available':
        case 'forbidden':
          found = true;

          return announced();
      }
    },

    update_install: async ({ onProgress }) => {
      if (!found) {
        throw IpcFailure.from({ code: 'notFound', message: 'No update to install; check for updates first.' });
      }

      if (scenario() === 'forbidden') {
        // The signature is checked once the bytes are in, so most of the progress is seen first.
        await replayProgress(onProgress, speed(), 30);

        throw IpcFailure.from({ code: 'forbidden', message: 'cannot download the update: signature mismatch' });
      }

      await replayProgress(onProgress, speed());
      found = false;

      return null;
    },

    app_relaunch: () => null,
  };
}
