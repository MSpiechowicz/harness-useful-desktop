import { IpcFailure } from '../transport';
import { embedderScenarioFromLocation, embedderStateOf } from './memory-embedder-handlers';
import type { MockState } from './state';
import type { Handlers } from './types';

type EmbedderCommands = 'knowledge_set_vector_search' | 'knowledge_embedder_watch' | 'knowledge_embedder_remove_model';

/**
 * The knowledge vector search commands over the mock's `knowledgeVectorSearch` pref: the embedder of the `?embedder=` scenario (shared with Harness
 * Memory's previews) while it is on, an off one otherwise.
 */
export function knowledgeEmbedderHandlers(state: MockState): Pick<Handlers, EmbedderCommands> {
  // The downloaded model was removed and vector search stayed off since: turning it on downloads it again.
  let removed = false;

  const on = (): boolean => state.knowledgeVectorSearch && state.knowledgeEnabled;
  const embedderState = (running: boolean) => embedderStateOf(embedderScenarioFromLocation(), running, removed);

  return {
    knowledge_set_vector_search: ({ enabled }) => {
      if (embedderScenarioFromLocation() === 'old') {
        throw IpcFailure.from({ code: 'unsupported', message: 'The installed Harness Knowledge has no vector search' });
      }

      removed &&= !enabled;

      return { vectorSearch: state.setKnowledgeVectorSearch(enabled).knowledgeVectorSearch, state: embedderState(enabled && state.knowledgeEnabled) };
    },
    knowledge_embedder_watch: ({ onEvent }) => {
      onEvent(embedderState(on()));

      return null;
    },
    knowledge_embedder_remove_model: () => {
      removed = true;

      return embedderState(false);
    },
  };
}
