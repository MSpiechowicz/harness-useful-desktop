import type { ScopeTarget } from '$lib/components/scope/scope-options';
import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
import type { ScriptDetail } from '$lib/ipc/bindings/ScriptDetail';
import type { ScriptDraft } from '$lib/ipc/bindings/ScriptDraft';
import type { ScriptLanguage } from '$lib/ipc/bindings/ScriptLanguage';

import type { Problem } from './editor-fields.svelte';
import { targetOfItem, targetProblems } from './scope-target';

/** The limits Harness Memory puts on a script. */
export const LIMITS = { name: 64, description: 500, bodyKiB: 64, timeoutSecs: { min: 1, max: 600 } } as const;

/** How long a script may run when the time limit is left empty. */
export const DEFAULT_TIMEOUT_SECS = 60;

/** The interpreters, the usual one first. */
export const LANGUAGES: readonly ScriptLanguage[] = ['bash', 'sh', 'python', 'node', 'bun'];

const NAME = /^[a-z0-9][a-z0-9_-]*$/;
const WHOLE_NUMBER = /^\d+$/;
const BYTES_PER_KIB = 1024;

export type ScriptScope = ScriptDraft['scope'];

/** What the editor holds: every field as typed. */
export interface ScriptForm {
  name: string;
  description: string;
  language: ScriptLanguage;
  body: string;
  /** Seconds as typed; empty means the default. */
  timeout: string;
  /** Where the script applies: Global, or the project it is kept in (the chosen one, not necessarily the open one). */
  target: ScopeTarget;
}

export type FieldKey = 'name' | 'description' | 'body' | 'timeout' | 'scope';

export interface ProblemContext {
  /** An existing script is being edited: its name and scope are kept, so neither is checked. */
  editing: boolean;
}

/** A new script, applying to `target`. */
export function blankForm(target: ScopeTarget): ScriptForm {
  return { name: '', description: '', language: 'bash', body: '', timeout: '', target };
}

/** The interpreter a script names, if this version knows it. */
function knownLanguage(language: string): ScriptLanguage | null {
  return LANGUAGES.find((known) => known === language) ?? null;
}

/** The language of a script this version cannot edit, or null when it can: such a script is kept as it is, never saved under another language. */
export function unsupportedLanguage(script: Pick<ScriptDetail, 'language'>): string | null {
  return knownLanguage(script.language) === null ? script.language : null;
}

/** The editor for a script that exists, applying where it does. A language this version does not know is shown as the usual one, and `unsupportedLanguage` keeps the script from being saved. */
export function formOf(script: ScriptDetail, projects: ProjectScope[]): ScriptForm {
  return {
    name: script.name,
    description: script.description,
    language: knownLanguage(script.language) ?? 'bash',
    body: script.body,
    timeout: String(script.timeoutSecs),
    target: targetOfItem(script, projects),
  };
}

/** The size the server limits: the bytes of the code as UTF-8. */
export function sizeOf(body: string): number {
  return new TextEncoder().encode(body).length;
}

/** The time limit typed, or the default when it was left empty; null for what is not a whole number of seconds in range. */
export function timeoutOf(text: string): number | null {
  const typed = text.trim();

  if (typed === '') {
    return DEFAULT_TIMEOUT_SECS;
  }

  const seconds = WHOLE_NUMBER.test(typed) ? Number(typed) : NaN;

  return seconds >= LIMITS.timeoutSecs.min && seconds <= LIMITS.timeoutSecs.max ? seconds : null;
}

export function problemsOf(form: ScriptForm, context: ProblemContext): Problem<FieldKey>[] {
  const problems: Problem<FieldKey>[] = [];
  const name = form.name.trim();
  const description = Array.from(form.description.trim()).length;

  if (!context.editing) {
    if (name === '') {
      problems.push({ field: 'name', message: 'memory.editor.script.problem.name.required', required: true });
    } else if (name.length > LIMITS.name) {
      problems.push({ field: 'name', message: 'memory.editor.script.problem.name.long', params: { max: LIMITS.name }, required: false });
    } else if (!NAME.test(name)) {
      problems.push({ field: 'name', message: 'memory.editor.script.problem.name.invalid', required: false });
    }
  }

  if (description === 0) {
    problems.push({ field: 'description', message: 'memory.editor.script.problem.description.required', required: true });
  } else if (description > LIMITS.description) {
    problems.push({ field: 'description', message: 'memory.editor.script.problem.description.long', params: { max: LIMITS.description }, required: false });
  }

  if (form.body.trim() === '') {
    problems.push({ field: 'body', message: 'memory.editor.script.problem.body.required', required: true });
  } else if (sizeOf(form.body) > LIMITS.bodyKiB * BYTES_PER_KIB) {
    problems.push({ field: 'body', message: 'memory.editor.script.problem.body.long', params: { max: LIMITS.bodyKiB }, required: false });
  }

  if (timeoutOf(form.timeout) === null) {
    problems.push({ field: 'timeout', message: 'memory.editor.script.problem.timeout', params: LIMITS.timeoutSecs, required: false });
  }

  problems.push(...targetProblems(form.target, context.editing));

  return problems;
}

export function draftOf(form: ScriptForm): ScriptDraft {
  return {
    name: form.name.trim(),
    description: form.description.trim(),
    language: form.language,
    body: form.body,
    timeoutSecs: timeoutOf(form.timeout) ?? DEFAULT_TIMEOUT_SECS,
    scope: form.target.kind,
  };
}

/** The code as the server compares it: line endings as LF, no trailing spaces or tabs on a line, no trailing blank lines. */
export function normalisedCode(body: string): string {
  // Split on LF only, as the backend does: a multiline `$` would also end a line at U+2028 and U+2029.
  return body
    .replace(/\r\n?/g, '\n')
    .split('\n')
    .map((line) => line.replace(/[ \t]+$/, ''))
    .join('\n')
    .replace(/\n+$/, '');
}

/** Whether `body` is other code than `opened`, the way the server counts it: a change of line endings or trailing whitespace alone is none. */
export function codeChanged(opened: string, body: string): boolean {
  return normalisedCode(opened) !== normalisedCode(body);
}
