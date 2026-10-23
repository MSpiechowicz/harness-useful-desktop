import type { EmbedderPhase } from '../bindings/EmbedderPhase';
import type { MemoryEmbedderState } from '../bindings/MemoryEmbedderState';
import { IpcFailure } from '../transport';
import type { MockState } from './state';
import type { Handlers } from './types';

type EmbedderCommands =
  | 'memory_set_vector_search'
  | 'memory_embedder_watch'
  | 'memory_embedder_remove_model'
  | 'memory_settings_get'
  | 'memory_settings_set';

/** The model the preview's embedder serves. */
const MODEL = 'paraphrase-multilingual-MiniLM-L12-v2';

/**
 * Browser-preview switch `?embedder=`: what the embedder does while vector search is on. `downloading`, `backfilling` and `updating` stop part
 * way, `sweep` downloads without knowing the size, `verifying`, `loading` and `starting` are the other busy phases, `failed` and `elsewhere` are
 * the stopped ones, and `old` is a Harness Memory without `embedder` and `settings` (its feature lines are not listed, see `memory-remote-handlers.ts`).
 * Anything else is a ready embedder.
 */
export const EMBEDDER_SCENARIOS = ['ready', 'starting', 'downloading', 'sweep', 'verifying', 'loading', 'backfilling', 'updating', 'failed', 'elsewhere', 'old'] as const;
export type EmbedderScenario = (typeof EMBEDDER_SCENARIOS)[number];

export function embedderScenarioFromLocation(search: string = globalThis.location?.search ?? ''): EmbedderScenario {
  const name = new URLSearchParams(search).get('embedder');

  return EMBEDDER_SCENARIOS.find((scenario) => scenario === name) ?? 'ready';
}

function running(phase: EmbedderPhase, progress: number | null, model: string | null, modelPresent: boolean | null, error: string | null = null): MemoryEmbedderState {
  return { phase, progress, model, error, modelPresent };
}

const RUNNING: Record<EmbedderScenario, MemoryEmbedderState> = {
  ready: running('ready', null, MODEL, true),
  starting: running('starting', null, null, null),
  downloading: running('downloading', 0.42, null, false),
  sweep: running('downloading', null, null, false),
  verifying: running('verifying', null, null, false),
  loading: running('loading', null, null, null),
  backfilling: running('backfilling', 0.63, MODEL, true),
  updating: running('updating', 0.3, MODEL, true),
  failed: running('failed', null, null, false, 'The model could not be downloaded'),
  elsewhere: running('elsewhere', null, null, null),
  old: running('off', null, null, null),
};

/** The stopped embedder: whether a model is downloaded is known by the scenarios that got that far, until it is removed. */
function stopped(scenario: EmbedderScenario, removed: boolean): MemoryEmbedderState {
  const known = RUNNING[scenario].modelPresent;

  return running('off', null, null, known === null ? null : known && !removed);
}

function unsupported(feature: string): IpcFailure {
  return IpcFailure.from({ code: 'unsupported', message: `The installed Harness Memory has no ${feature}` });
}

/**
 * The vector search and English notes commands over the mock's `vectorSearch` pref: the embedder of the `?embedder=` scenario while it is on,
 * an off one otherwise.
 */
export function memoryEmbedderHandlers(state: MockState, scenario: () => EmbedderScenario = embedderScenarioFromLocation): Pick<Handlers, EmbedderCommands> {
  let englishNotes = true;
  // The downloaded model was removed and vector search stayed off since: turning it on downloads it again.
  let removed = false;

  const on = (): boolean => state.vectorSearch && state.memoryEnabled;
  const embedderState = (running: boolean): MemoryEmbedderState => (running ? RUNNING[scenario()] : stopped(scenario(), removed));

  return {
    memory_set_vector_search: ({ enabled }) => {
      if (scenario() === 'old') {
        throw unsupported('vector search');
      }

      removed &&= !enabled;

      return { vectorSearch: state.setVectorSearch(enabled).vectorSearch, state: embedderState(enabled && state.memoryEnabled) };
    },
    memory_embedder_watch: ({ onEvent }) => {
      onEvent(embedderState(on()));

      return null;
    },
    memory_embedder_remove_model: () => {
      removed = true;

      return embedderState(false);
    },
    memory_settings_get: () => {
      if (scenario() === 'old') {
        throw unsupported('settings');
      }

      return { englishNotes };
    },
    memory_settings_set: (args) => {
      if (scenario() === 'old') {
        throw unsupported('settings');
      }

      englishNotes = args.englishNotes;

      return { englishNotes };
    },
  };
}
