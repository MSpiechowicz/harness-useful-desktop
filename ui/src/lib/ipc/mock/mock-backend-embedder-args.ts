import type { CommandArgs } from '../commands';

/** One valid call to each vector search, memory settings and code index command, for the mock backend test. */
export const MEMORY_EMBEDDER_ARGS = {
  code_set_enabled: { enabled: false },
  setup_install_code: { confirmed: true, onLog: () => {} },
  setup_register_code: { harness: 'omp' },
  memory_set_vector_search: { enabled: false },
  memory_embedder_watch: { onEvent: () => {} },
  memory_embedder_remove_model: undefined,
  memory_settings_get: undefined,
  memory_settings_set: { englishNotes: false },
  code_index_get: undefined,
  code_index_set: { on: false },
  code_builds_listen: { onEvent: () => {} },
  code_prepare: { project: '/home/demo/project' },
  code_auto_install_claim: undefined,
} satisfies {
  code_set_enabled: CommandArgs<'code_set_enabled'>;
  setup_install_code: CommandArgs<'setup_install_code'>;
  setup_register_code: CommandArgs<'setup_register_code'>;
  memory_set_vector_search: CommandArgs<'memory_set_vector_search'>;
  memory_embedder_watch: CommandArgs<'memory_embedder_watch'>;
  memory_embedder_remove_model: CommandArgs<'memory_embedder_remove_model'>;
  memory_settings_get: CommandArgs<'memory_settings_get'>;
  memory_settings_set: CommandArgs<'memory_settings_set'>;
  code_index_get: CommandArgs<'code_index_get'>;
  code_index_set: CommandArgs<'code_index_set'>;
  code_builds_listen: CommandArgs<'code_builds_listen'>;
  code_prepare: CommandArgs<'code_prepare'>;
  code_auto_install_claim: CommandArgs<'code_auto_install_claim'>;
};
