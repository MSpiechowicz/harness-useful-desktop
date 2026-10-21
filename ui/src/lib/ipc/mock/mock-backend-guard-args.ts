import type { CommandArgs } from '../commands';
import { HELD_NOTE_SAVE_ID, HELD_REVISIONS, HELD_SCRIPT_SAVE_ID } from './memory-guards-fixtures';

type GuardCommand =
  | 'memory_guards_get'
  | 'memory_guards_update'
  | 'memory_chat_guards_set'
  | 'memory_held_list'
  | 'memory_held_save'
  | 'memory_held_skip'
  | 'memory_held_listen'
  | 'chat_memory_check_again'
  | 'dialog_pick_folder';

const CHAT_KEY = '3b2f6c1e-8a4d-4e5f-9a0b-1c2d3e4f5a6b';

/** One valid call per save-guard command (`mock-backend.test.ts` has the rest); `sessionKey` is a running chat. */
export function memoryGuardArgs(sessionKey: string): { [K in GuardCommand]: CommandArgs<K> } {
  return {
    memory_guards_get: undefined,
    memory_guards_update: { patch: { kind: 'notes', addRepo: '/home/demo/project/' } },
    memory_chat_guards_set: { key: CHAT_KEY, guards: { paused: false, notes: { on: true, ask: true }, scripts: { on: false, ask: false } }, revision: 1 },
    memory_held_list: { chatKey: CHAT_KEY },
    memory_held_save: {
      heldId: HELD_NOTE_SAVE_ID,
      edited: { title: 'Bindings come from the export test', body: 'Edited by the user.', kind: 'location', tags: [] },
      revision: HELD_REVISIONS[HELD_NOTE_SAVE_ID],
    },
    memory_held_skip: { heldId: HELD_SCRIPT_SAVE_ID, revision: HELD_REVISIONS[HELD_SCRIPT_SAVE_ID] },
    memory_held_listen: { onEvent: () => {} },
    chat_memory_check_again: { agentId: sessionKey },
    dialog_pick_folder: undefined,
  };
}
