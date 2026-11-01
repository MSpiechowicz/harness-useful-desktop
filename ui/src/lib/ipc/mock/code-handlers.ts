import type { CodeBuildEvent } from '../bindings/CodeBuildEvent';
import type { CodeIndexState } from '../bindings/CodeIndexState';
import { IpcFailure } from '../transport';
import type { Handlers } from './types';

type CodeCommands = 'code_index_get' | 'code_index_set' | 'code_builds_listen' | 'code_prepare';

/**
 * Browser-preview switch `?code=`: `old` is a Harness Code without the `code-index` feature, `build` runs a first index of a repository once
 * a page listens (a toast for 'building', then 'ready'), `bulk` a large change that ends in an error, `cancelled` and `stale` a build that
 * stops without a result. Anything else is a current server with no build going on.
 */
export const CODE_SCENARIOS = ['ready', 'old', 'build', 'bulk', 'cancelled', 'stale'] as const;
export type CodeScenario = (typeof CODE_SCENARIOS)[number];

export function codeScenarioFromLocation(search: string = globalThis.location?.search ?? ''): CodeScenario {
  const name = new URLSearchParams(search).get('code');

  return CODE_SCENARIOS.find((scenario) => scenario === name) ?? 'ready';
}

/** How long the preview's build waits between two of its steps. */
const STEP_MS = 700;

const FILES = 240;

function build(phase: CodeBuildEvent['phase'], filesDone: number, kind: string, error: string | null = null): CodeBuildEvent {
  return { buildId: 1, root: '/home/demo/project', repoName: 'project', kind, phase, filesTotal: FILES, filesDone, error };
}

/** The events a scenario sends after the page listens, in order. */
function steps(scenario: CodeScenario): CodeBuildEvent[] {
  const progress = (kind: string): CodeBuildEvent[] => [0, 24, 72, 144, 216].map((done) => build('building', done, kind));

  switch (scenario) {
    case 'build':
      return [...progress('full'), build('ready', FILES, 'full')];
    case 'bulk':
      return [...progress('bulk').slice(0, 3), build('error', 72, 'bulk', 'The index could not be written: disk full')];
    case 'cancelled':
      return [...progress('full').slice(0, 2), build('cancelled', 24, 'full')];
    case 'stale':
      return [...progress('full').slice(0, 2), build('stale', 24, 'full')];
    default:
      return [];
  }
}

function notInstalled(): IpcFailure {
  return IpcFailure.from({ code: 'notFound', message: 'Harness Code is not installed' });
}

function unsupported(): IpcFailure {
  return IpcFailure.from({ code: 'unsupported', message: 'The installed Harness Code has no code index' });
}

/**
 * The code index switch (on until set otherwise) and the builds of the `?code=` scenario. Without Harness Code installed (the setup
 * mock decides) the switch and the builds on open answer `notFound`, like the backend.
 */
export function codeHandlers(
  scenario: () => CodeScenario = codeScenarioFromLocation,
  stepMs: number = STEP_MS,
  installed: () => boolean = () => true,
): Pick<Handlers, CodeCommands> {
  let enabled = true;
  let saved = false;

  const state = (): CodeIndexState => ({ enabled, source: saved ? 'file' : 'default' });
  const timers = new Set<ReturnType<typeof setTimeout>>();

  return {
    code_index_get: () => {
      if (!installed()) {
        throw notInstalled();
      }

      if (scenario() === 'old') {
        throw unsupported();
      }

      return state();
    },
    code_index_set: ({ on }) => {
      if (!installed()) {
        throw notInstalled();
      }

      if (scenario() === 'old') {
        throw unsupported();
      }

      enabled = on;
      saved = true;

      return state();
    },
    code_builds_listen: ({ onEvent }) => {
      timers.forEach(clearTimeout);
      timers.clear();

      steps(scenario()).forEach((event, index) => {
        timers.add(setTimeout(() => onEvent(event), (index + 1) * stepMs));
      });

      return null;
    },
    code_prepare: () => {
      if (!installed()) {
        throw notInstalled();
      }

      if (scenario() === 'old') {
        throw unsupported();
      }

      return null;
    },
  };
}
