import type { ScopeTarget } from '$lib/components/scope/scope-options';
import type { HeldNoteView, HeldScriptView } from '$lib/chat/memory-held';
import type { HeldItem } from '$lib/ipc/bindings/HeldItem';
import type { NoteContent } from '$lib/ipc/bindings/NoteContent';

import { blankForm as blankNote, type NoteForm } from '../memory/note-form';
import { blankForm as blankScript, type ScriptForm } from '../memory/script-form';

/** Where the editor shows a save applying: the chat's repository for a repository save, else Global. */
function targetOf(item: HeldItem, scope: 'repo' | 'global'): ScopeTarget {
  const path = item.repoRoot ?? item.projectPath;

  return scope === 'repo' && path !== null ? { kind: 'repo', path } : { kind: 'global' };
}

/** The editor's form for a note an agent wants to save, as the agent sent it. */
export function noteFormOf(item: HeldItem, view: HeldNoteView): NoteForm {
  const { title, body, kind, tags, entities, scope } = view.note;

  return { ...blankNote(targetOf(item, scope)), title, body, kind, tags: [...tags], entities: [...entities] };
}

/** The editor's form for a script an agent wants to save, as the agent sent it. */
export function scriptFormOf(item: HeldItem, view: HeldScriptView): ScriptForm {
  const { name, description, language, body, timeoutSecs, scope } = view.script;

  return { ...blankScript(targetOf(item, scope)), name, description, language, body, timeout: String(timeoutSecs) };
}

/**
 * The note a change to a saved note would make, shaped as the editor takes a note it edits. The editor shows the scope, entities and links
 * of the note and changes none of them, so what is not known here (when it was written, how often it was read) is left empty.
 */
export function noteContentOf(item: HeldItem, view: HeldNoteView): NoteContent {
  const { title, body, kind, tags, scope } = view.note;
  const entities = view.current?.entities ?? view.note.entities;

  return {
    id: view.noteId ?? 0,
    title,
    body,
    kind,
    tags,
    entities,
    scope,
    repo: scope === 'repo' ? (item.repoRoot ?? item.projectPath) : null,
    place: 'local',
    author: { source: item.client, root: item.repoRoot },
    createdAt: item.createdAt,
    updatedAt: item.createdAt,
    revision: 0,
    hits: 0,
    readOnly: false,
    hidden: false,
  };
}
