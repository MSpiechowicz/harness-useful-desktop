import { type ChipRules, chipsOf, mergeChips } from '$lib/components/docs/chips';
import type { ScopeTarget } from '$lib/components/scope/scope-options';
import type { NoteContent } from '$lib/ipc/bindings/NoteContent';
import type { NoteDraft } from '$lib/ipc/bindings/NoteDraft';
import type { NoteKind } from '$lib/ipc/bindings/NoteKind';
import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';

import type { Problem } from './editor-fields.svelte';
import { targetOfItem, targetProblems } from './scope-target';

/** The limits Harness Memory puts on a note. */
export const LIMITS = { title: 120, body: 4000, tags: 8, tag: 32, entities: 10, entity: 80 } as const;

const TAG = /^[a-z0-9_-]+$/;

/** Tags end at a comma or a space and are kept lowercase. */
export const TAG_RULES: ChipRules = { split: /[\s,]+/, normalize: (piece) => piece.toLowerCase() };

/** Entities end at a comma or a line break, may hold spaces, and are kept on one line. */
export const ENTITY_RULES: ChipRules = { split: /[,\n\r]+/, normalize: (piece) => piece.replace(/\s+/g, ' ').trim() };

export type NoteScope = NoteDraft['scope'];

/** What the editor holds: every field as typed. */
export interface NoteForm {
  title: string;
  body: string;
  kind: NoteKind;
  /** The tags that are chips already. */
  tags: string[];
  /** The text typed after the last tag chip. */
  tagsDraft: string;
  /** The entities that are chips already. A note that exists keeps its own whatever the editor holds. */
  entities: string[];
  /** The text typed after the last entity chip. */
  entitiesDraft: string;
  /** Where the note applies: Global, or the project it is kept in (the chosen one, not necessarily the open one). */
  target: ScopeTarget;
}

export type FieldKey = 'title' | 'body' | 'tags' | 'entities' | 'scope';

export interface ProblemContext {
  /** An existing note is being edited: its scope is not changed, so it needs no project. */
  editing: boolean;
}

/** A new note, applying to `target`. */
export function blankForm(target: ScopeTarget): NoteForm {
  return { title: '', body: '', kind: 'note', tags: [], tagsDraft: '', entities: [], entitiesDraft: '', target };
}

/** The editor for a note that exists, applying where it does. */
export function formOf(note: NoteContent, projects: ProjectScope[]): NoteForm {
  return { title: note.title, body: note.body, kind: note.kind, tags: note.tags, tagsDraft: '', entities: note.entities, entitiesDraft: '', target: targetOfItem(note, projects) };
}

/** The tags of the form, as the server keeps them: the chips and what is typed after them, lowercase, each once. */
export function tagsOf(form: NoteForm): string[] {
  return mergeChips(form.tags, chipsOf(form.tagsDraft, TAG_RULES));
}

/** The entities of the form: the chips and what is typed after them, each once whatever its case. */
export function entitiesOf(form: NoteForm): string[] {
  return mergeChips(form.entities, chipsOf(form.entitiesDraft, ENTITY_RULES));
}

/** A tag the server would refuse, shown in red in the field. */
export function tagFlagged(tag: string): boolean {
  return length(tag) > LIMITS.tag || !TAG.test(tag);
}

/** An entity the server would refuse, shown in red in the field. */
export function entityFlagged(entity: string): boolean {
  return length(entity) > LIMITS.entity;
}

/** The length the server counts, in characters rather than UTF-16 units. */
export function length(text: string): number {
  return Array.from(text).length;
}

export function problemsOf(form: NoteForm, context: ProblemContext): Problem<FieldKey>[] {
  const problems: Problem<FieldKey>[] = [];
  const title = length(form.title.trim());
  const body = length(form.body.trim());
  const tags = tagsOf(form);
  const entities = entitiesOf(form);

  if (title === 0) {
    problems.push({ field: 'title', message: 'memory.editor.note.problem.title.required', required: true });
  } else if (title > LIMITS.title) {
    problems.push({ field: 'title', message: 'memory.editor.note.problem.title.long', params: { max: LIMITS.title }, required: false });
  }

  if (body === 0) {
    problems.push({ field: 'body', message: 'memory.editor.note.problem.body.required', required: true });
  } else if (body > LIMITS.body) {
    problems.push({ field: 'body', message: 'memory.editor.note.problem.body.long', params: { max: LIMITS.body }, required: false });
  }

  if (tags.length > LIMITS.tags) {
    problems.push({ field: 'tags', message: 'memory.editor.note.problem.tags.many', params: { max: LIMITS.tags }, required: false });
  } else if (tags.some((tag) => length(tag) > LIMITS.tag)) {
    problems.push({ field: 'tags', message: 'memory.editor.note.problem.tags.long', params: { max: LIMITS.tag }, required: false });
  } else if (tags.some((tag) => !TAG.test(tag))) {
    problems.push({ field: 'tags', message: 'memory.editor.note.problem.tags.invalid', required: false });
  }

  // An edit cannot change them, so there is nothing to check.
  if (!context.editing && entities.length > LIMITS.entities) {
    problems.push({ field: 'entities', message: 'memory.editor.note.problem.entities.many', params: { max: LIMITS.entities }, required: false });
  } else if (!context.editing && entities.some((entity) => length(entity) > LIMITS.entity)) {
    problems.push({ field: 'entities', message: 'memory.editor.note.problem.entities.long', params: { max: LIMITS.entity }, required: false });
  }

  problems.push(...targetProblems(form.target, context.editing));

  return problems;
}

export function draftOf(form: NoteForm): NoteDraft {
  return { title: form.title.trim(), body: form.body.trim(), kind: form.kind, tags: tagsOf(form), entities: entitiesOf(form), scope: form.target.kind };
}
