import type { MemoryEmbedderState } from '../bindings/MemoryEmbedderState';
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

function embedderState(on: boolean): MemoryEmbedderState {
  return on ? { phase: 'ready', progress: null, model: MODEL, error: null } : OFF;
}

/** The vector search and English notes commands over the mock's `vectorSearch` pref: a ready embedder while it is on, an off one otherwise. */
export function memoryEmbedderHandlers(state: MockState): Pick<Handlers, EmbedderCommands> {
  let englishNotes = true;

  return {
    memory_set_vector_search: ({ enabled }) => ({
      vectorSearch: state.setVectorSearch(enabled).vectorSearch,
      state: embedderState(enabled && state.memoryEnabled),
    }),
    memory_embedder_watch: ({ onEvent }) => {
      onEvent(embedderState(state.vectorSearch && state.memoryEnabled));

      return null;
    },
    memory_embedder_remove_model: () => OFF,
    memory_settings_get: () => ({ englishNotes }),
    memory_settings_set: (args) => {
      englishNotes = args.englishNotes;

      return { englishNotes };
    },
  };
}
