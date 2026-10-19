import type { CommandArgs, CommandName } from '../commands';

/** One valid call per command of the notes (`mock-backend.test.ts` has the rest). The copy ones name the review the first comparison makes. */
export const MEMORY_NOTE_ARGS: { [K in Extract<CommandName, `memory_note${string}`>]: CommandArgs<K> } = {
  memory_notes_list: { project: '/home/demo/project', offset: 0 },
  memory_note_get: { id: 3, project: '/home/demo/project' },
  memory_note_delete: { id: 3, project: '/home/demo/project' },
  memory_note_save: { project: '/home/demo/project', draft: { title: 'A note from the app', body: 'Written in the app.', kind: 'decision', tags: [], entities: [], scope: 'repo' } },
  memory_notes_delete_all: { project: null },
  memory_note_hide: { id: 3, project: '/home/demo/project', hidden: true },
  memory_notes_discard_local: { ids: 'all' },
  memory_notes_copy_preview: { kind: 'notes' },
  memory_notes_copy_rows: { kind: 'notes', previewId: 'copy-1', filter: 'conflicts', offset: 0, limit: 50 },
  memory_notes_copy_apply: { kind: 'notes', previewId: 'copy-1', overwrite: 'none' },
  memory_notes_import_preview: { project: '/home/demo/project' },
  memory_notes_import_template: { format: 'json' },
  memory_notes_import_apply: { planId: 'notes-import-1', rows: [0, 2], defaultScope: 'repo', replace: false, project: '/home/demo/project', onProgress: () => {} },
};
