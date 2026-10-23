import type { CommandArgs } from '../commands';

/** One valid call to each vector search and memory settings command, for the mock backend test. */
export const MEMORY_EMBEDDER_ARGS = {
  memory_set_vector_search: { enabled: false },
  memory_embedder_watch: { onEvent: () => {} },
  memory_embedder_remove_model: undefined,
  memory_settings_get: undefined,
  memory_settings_set: { englishNotes: false },
} satisfies {
  memory_set_vector_search: CommandArgs<'memory_set_vector_search'>;
  memory_embedder_watch: CommandArgs<'memory_embedder_watch'>;
  memory_embedder_remove_model: CommandArgs<'memory_embedder_remove_model'>;
  memory_settings_get: CommandArgs<'memory_settings_get'>;
  memory_settings_set: CommandArgs<'memory_settings_set'>;
};
