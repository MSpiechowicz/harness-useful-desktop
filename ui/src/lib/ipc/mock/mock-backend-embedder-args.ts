import type { CommandArgs } from '../commands';

/** One valid call to each vector search, memory settings and code index command, for the mock backend test. */
export const MEMORY_EMBEDDER_ARGS = {
  memory_set_vector_search: { enabled: false },
  memory_embedder_watch: { onEvent: () => {} },
  memory_embedder_remove_model: undefined,
  memory_settings_get: undefined,
  memory_settings_set: { englishNotes: false },
  memory_code_index_get: undefined,
  memory_code_index_set: { on: false },
  memory_code_builds_listen: { onEvent: () => {} },
  memory_code_prepare: { project: '/home/demo/project' },
} satisfies {
  memory_set_vector_search: CommandArgs<'memory_set_vector_search'>;
  memory_embedder_watch: CommandArgs<'memory_embedder_watch'>;
  memory_embedder_remove_model: CommandArgs<'memory_embedder_remove_model'>;
  memory_settings_get: CommandArgs<'memory_settings_get'>;
  memory_settings_set: CommandArgs<'memory_settings_set'>;
  memory_code_index_get: CommandArgs<'memory_code_index_get'>;
  memory_code_index_set: CommandArgs<'memory_code_index_set'>;
  memory_code_builds_listen: CommandArgs<'memory_code_builds_listen'>;
  memory_code_prepare: CommandArgs<'memory_code_prepare'>;
};
