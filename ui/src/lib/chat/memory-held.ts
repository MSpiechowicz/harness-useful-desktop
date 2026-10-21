import type { HeldItem } from '$lib/ipc/bindings/HeldItem';
import type { NoteDraft } from '$lib/ipc/bindings/NoteDraft';
import type { NoteKind } from '$lib/ipc/bindings/NoteKind';
import type { ScriptDraft } from '$lib/ipc/bindings/ScriptDraft';
import type { ScriptLanguage } from '$lib/ipc/bindings/ScriptLanguage';
import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';

type Fields = Record<string, JsonValue>;

const NOTE_KINDS: readonly NoteKind[] = ['decision', 'fix', 'problem', 'outcome', 'optimisation', 'location', 'note'];
const LANGUAGES: readonly ScriptLanguage[] = ['bash', 'sh', 'python', 'node', 'bun'];
const DEFAULT_TIMEOUT_SECS = 60;

/** A note a held save writes, or the note as saved now. */
export interface HeldNoteFields {
  title: string;
  body: string;
  kind: NoteKind;
  tags: string[];
  entities: string[];
  scope: 'repo' | 'global';
}

/** A script a held save writes, or the script it replaces as saved now. */
export interface HeldScriptFields {
  name: string;
  description: string;
  language: ScriptLanguage;
  body: string;
  timeoutSecs: number;
  scope: 'repo' | 'global';
}

export interface HeldNoteView {
  type: 'note';
  heldId: string;
  /** A change to the note `noteId` instead of a new note. */
  noteId: number | null;
  note: HeldNoteFields;
  /** The note as saved now, for an update. */
  current: HeldNoteFields | null;
}

export interface HeldScriptView {
  type: 'script';
  heldId: string;
  script: HeldScriptFields;
  /** The script of that name as saved now, when the save replaces one. */
  current: HeldScriptFields | null;
}

export type HeldView = HeldNoteView | HeldScriptView;

function fieldsOf(value: JsonValue): Fields {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? value : {};
}

function text(value: JsonValue | undefined): string {
  return typeof value === 'string' ? value : '';
}

function words(value: JsonValue | undefined): string[] {
  return Array.isArray(value) ? value.filter((entry): entry is string => typeof entry === 'string') : [];
}

/** A word as the server compares it: trimmed and lowercase. The server stores it that way already, so this only guards a display. */
function normal(value: JsonValue | undefined): string | undefined {
  return typeof value === 'string' ? value.trim().toLowerCase() : undefined;
}

function pick<T extends string>(value: JsonValue | undefined, allowed: readonly T[], fallback: T): T {
  const word = normal(value);

  return allowed.find((candidate) => candidate === word) ?? fallback;
}

function scopeOf(value: JsonValue | undefined, fallback: 'repo' | 'global'): 'repo' | 'global' {
  const word = normal(value);

  return word === 'global' || word === 'repo' ? word : fallback;
}

/** An appended text as the server keeps it before joining: line breaks as `\n`, trimmed. */
function appended(value: JsonValue | undefined): string {
  return typeof value === 'string' ? value.replace(/\r\n|[\r\v\f\u0085\u2028\u2029]/g, '\n').trim() : '';
}

function whole(value: JsonValue | undefined): number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? Math.round(value) : DEFAULT_TIMEOUT_SECS;
}

function noteOf(fields: Fields, fallback: HeldNoteFields): HeldNoteFields {
  return {
    title: 'title' in fields ? text(fields.title) : fallback.title,
    body: 'body' in fields ? text(fields.body) : fallback.body,
    kind: 'kind' in fields ? pick(fields.kind, NOTE_KINDS, fallback.kind) : fallback.kind,
    tags: 'tags' in fields ? words(fields.tags) : fallback.tags,
    entities: 'entities' in fields ? words(fields.entities) : fallback.entities,
    scope: scopeOf(fields.scope, fallback.scope),
  };
}

function scriptOf(fields: Fields, fallback: HeldScriptFields): HeldScriptFields {
  // The agent's arguments are snake case, the script as saved now is camel case.
  const timeout = 'timeout_secs' in fields ? fields.timeout_secs : fields.timeoutSecs;

  return {
    name: 'name' in fields ? text(fields.name) : fallback.name,
    description: 'description' in fields ? text(fields.description) : fallback.description,
    language: 'language' in fields ? pick(fields.language, LANGUAGES, fallback.language) : fallback.language,
    body: 'body' in fields ? text(fields.body) : fallback.body,
    timeoutSecs: timeout === undefined ? fallback.timeoutSecs : whole(timeout),
    scope: scopeOf(fields.scope, fallback.scope),
  };
}

/**
 * The note an update leaves: the body is the one the agent sent, else the saved one, with `append` added after a line break, as the
 * server joins them. Showing the sum keeps an append from hiding in a card that would otherwise show the saved body unchanged.
 */
function updatedOf(fields: Fields, saved: HeldNoteFields): HeldNoteFields {
  const note = noteOf(fields, saved);
  const append = appended(fields.append);

  return append === '' ? note : { ...note, body: `${note.body}\n${append}` };
}

/** What a held save would save, read from the arguments the agent sent (and the saved version it changes) as far as they are well formed. */
export function heldView(item: HeldItem): HeldView {
  const args = fieldsOf(item.arguments);
  const current = fieldsOf(item.current);
  const inRepo = item.repoRoot !== null || item.projectPath !== null;

  if (item.kind === 'script') {
    const blank: HeldScriptFields = { name: '', description: '', language: 'bash', body: '', timeoutSecs: DEFAULT_TIMEOUT_SECS, scope: inRepo ? 'repo' : 'global' };
    const replaced = Object.keys(current).length > 0 ? scriptOf(current, blank) : null;

    return { type: 'script', heldId: item.heldId, script: scriptOf(args, replaced ?? blank), current: replaced };
  }

  const blank: HeldNoteFields = { title: '', body: '', kind: 'note', tags: [], entities: [], scope: inRepo ? 'repo' : 'global' };
  const before = item.tool === 'note_update' && Object.keys(current).length > 0 ? noteOf(current, blank) : null;
  const id = item.tool === 'note_update' ? (typeof args.id === 'number' ? args.id : typeof current.id === 'number' ? current.id : null) : null;

  const note = before ? updatedOf(args, before) : noteOf(args, blank);

  return { type: 'note', heldId: item.heldId, noteId: id, note, current: before };
}

/** The first line of the card: how many of each the agent wants to save, as counts for the plural messages. */
export function heldCounts(items: readonly HeldItem[]): { notes: number; scripts: number } {
  const scripts = items.filter((item) => item.kind === 'script').length;

  return { notes: items.length - scripts, scripts };
}

/** What a held item is called in a short line: a note's title or a script's name. */
export function heldTitle(item: HeldItem): string {
  const view = heldView(item);

  return view.type === 'note' ? view.note.title : view.script.name;
}

/** The `edited` draft for a held save: only the fields the tool takes, as the user left them. */
export function editedNote(item: HeldItem, draft: NoteDraft): JsonValue {
  const { title, body, kind, tags } = draft;

  return item.tool === 'note_update' ? { title, body, kind, tags } : { title, body, kind, tags, entities: draft.entities, scope: draft.scope };
}

export function editedScript(draft: ScriptDraft): JsonValue {
  return { name: draft.name, description: draft.description, language: draft.language, body: draft.body, timeoutSecs: draft.timeoutSecs, scope: draft.scope };
}

const LONG_LINES = 10;
const LONG_CHARS = 700;

/** Whether a body is long enough to be cut short with "Show all" in the card. */
export function isLong(text: string): boolean {
  return text.length > LONG_CHARS || text.split('\n').length > LONG_LINES;
}
