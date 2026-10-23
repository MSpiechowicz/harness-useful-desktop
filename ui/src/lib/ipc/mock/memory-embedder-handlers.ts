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

const OFF: MemoryEmbedderState = { phase: 'off', progress: null, model: null, error: null };

/**
 * Browser-preview switch `?embedder=`: what the embedder does while vector search is on. `downloading`, `backfilling` and `updating` stop part
 * way, `sweep` downloads without knowing the size, `verifying`, `loading` and `starting` are the other busy phases, `failed` and `elsewhere` are
 * the stopped ones, and `old` is a Harness Memory without `embedder` and `settings`. Anything else is a ready embedder.
 */
export const EMBEDDER_SCENARIOS = ['ready', 'starting', 'downloading', 'sweep', 'verifying', 'loading', 'backfilling', 'updating', 'failed', 'elsewhere', 'old'] as const;
export type EmbedderScenario = (typeof EMBEDDER_SCENARIOS)[number];

export function embedderScenarioFromLocation(search: string = globalThis.location?.search ?? ''): EmbedderScenario {
  const name = new URLSearchParams(search).get('embedder');

  return EMBEDDER_SCENARIOS.find((scenario) => scenario === name) ?? 'ready';
}

const RUNNING: Record<EmbedderScenario, MemoryEmbedderState> = {
  ready: { phase: 'ready', progress: null, model: MODEL, error: null },
  starting: { phase: 'starting', progress: null, model: null, error: null },
  downloading: { phase: 'downloading', progress: 0.42, model: null, error: null },
  sweep: { phase: 'downloading', progress: null, model: null, error: null },
  verifying: { phase: 'verifying', progress: null, model: null, error: null },
  loading: { phase: 'loading', progress: null, model: null, error: null },
  backfilling: { phase: 'backfilling', progress: 0.63, model: MODEL, error: null },
  updating: { phase: 'updating', progress: 0.3, model: MODEL, error: null },
  failed: { phase: 'failed', progress: null, model: null, error: 'The model could not be downloaded' },
  elsewhere: { phase: 'elsewhere', progress: null, model: null, error: null },
  old: OFF,
};

function embedderState(on: boolean, scenario: EmbedderScenario): MemoryEmbedderState {
  return on ? RUNNING[scenario] : OFF;
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

  const on = (): boolean => state.vectorSearch && state.memoryEnabled;

  return {
    memory_set_vector_search: ({ enabled }) => {
      if (scenario() === 'old') {
        throw unsupported('vector search');
      }

      return { vectorSearch: state.setVectorSearch(enabled).vectorSearch, state: embedderState(enabled && state.memoryEnabled, scenario()) };
    },
    memory_embedder_watch: ({ onEvent }) => {
      onEvent(embedderState(on(), scenario()));

      return null;
    },
    memory_embedder_remove_model: () => OFF,
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
