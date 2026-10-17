import type { MessageKey } from '$lib/i18n/locales/en';
import type { Hook } from '$lib/ipc/bindings/Hook';
import type { HookDraft } from '$lib/ipc/bindings/HookDraft';
import type { HookEvent } from '$lib/ipc/bindings/HookEvent';
import type { HookScope } from '$lib/ipc/bindings/HookScope';
import type { HookView } from '$lib/ipc/bindings/HookView';

import { type ActionKind, length, VALID_ACTIONS } from './hook-format';

/** The limits of the hook format (`docs/hooks-format.md`). */
export const LIMITS = { name: 80, description: 500, text: 4096, command: 2048, matcher: 200, timeoutMin: 1, timeoutMax: 30 } as const;

/** How long a command may run when the hook names no time. */
export const DEFAULT_TIMEOUT_SECS = 10;

/** What the editor opens on: a hook to edit, or a new hook in a scope. */
export interface EditorTarget {
  view: HookView | null;
  scope: HookScope;
  event?: HookEvent;
}

/** What the editor holds: every field as typed, so switching the action or the event loses nothing. */
export interface HookForm {
  name: string;
  description: string;
  event: HookEvent;
  kind: ActionKind;
  /** The text of an add-context or a follow-up action. */
  text: string;
  command: string;
  /** Seconds as typed; empty means the default. */
  timeout: string;
  matcher: string;
  scope: HookScope;
  enabled: boolean;
}

export type FieldKey = 'name' | 'description' | 'matcher' | 'text' | 'command' | 'timeout' | 'scope';

/** Why a field is not acceptable. A `required` one is only said once the user has been at the field. */
export interface Problem {
  field: FieldKey;
  message: MessageKey;
  params?: Record<string, number>;
  required: boolean;
}

export function blankForm(scope: HookScope = 'global'): HookForm {
  return { name: '', description: '', event: 'sessionStart', kind: 'addContext', text: '', command: '', timeout: '', matcher: '', scope, enabled: true };
}

/** The editor for a hook that exists. */
export function formOf(hook: Hook): HookForm {
  const action = hook.action;

  return {
    name: hook.name,
    description: hook.description ?? '',
    event: hook.event,
    kind: action.type,
    text: action.type === 'command' ? '' : action.text,
    command: action.type === 'command' ? action.command : '',
    timeout: action.type === 'command' && action.timeoutSecs !== null ? String(action.timeoutSecs) : '',
    matcher: hook.matcher ?? '',
    scope: hook.scope,
    enabled: hook.enabled,
  };
}

/** The ways to act at an event, in the order they are offered. */
export function validActions(event: HookEvent): readonly ActionKind[] {
  return VALID_ACTIONS[event];
}

/** Moves the hook to another event. The action stays when the event allows it, else it becomes the event's first. */
export function withEvent(form: HookForm, event: HookEvent): HookForm {
  const allowed = validActions(event);

  return { ...form, event, kind: allowed.includes(form.kind) ? form.kind : allowed[0] };
}

const TOOL_NAME = /^[A-Za-z0-9_.:-]+$/;

/** What is wrong with a tool filter, by the format's grammar: names of `[A-Za-z0-9_.:-]` joined by `|`, 200 characters at most. Empty is fine. */
export function matcherProblem(matcher: string): 'long' | 'empty' | 'chars' | null {
  const value = matcher.trim();

  if (value === '') {
    return null;
  }

  if (length(value) > LIMITS.matcher) {
    return 'long';
  }

  const names = value.split('|');

  if (names.some((name) => name === '')) {
    return 'empty';
  }

  return names.every((name) => TOOL_NAME.test(name)) ? null : 'chars';
}

function timeoutProblem(timeout: string): boolean {
  const value = timeout.trim();

  if (value === '') {
    return false;
  }

  const seconds = Number(value);

  return !/^\d+$/.test(value) || seconds < LIMITS.timeoutMin || seconds > LIMITS.timeoutMax;
}

export interface FormContext {
  /** A project is open, so a hook can be made for it. */
  projectOpen: boolean;
  /** The scope the hook would be in already holds the most it can. */
  scopeFull: boolean;
}

/** Everything that keeps the form from being saved, in the order of the fields. */
export function problemsOf(form: HookForm, context: FormContext): Problem[] {
  const found: Problem[] = [];
  const name = form.name.trim();

  if (name === '') {
    found.push({ field: 'name', message: 'hooks.editor.name.required', required: true });
  } else if (length(name) > LIMITS.name) {
    found.push({ field: 'name', message: 'hooks.editor.name.long', params: { max: LIMITS.name }, required: false });
  } else if (/\p{Cc}/u.test(name)) {
    found.push({ field: 'name', message: 'hooks.editor.name.control', required: false });
  }

  if (length(form.description.trim()) > LIMITS.description) {
    found.push({ field: 'description', message: 'hooks.editor.description.long', params: { max: LIMITS.description }, required: false });
  }

  if (form.event === 'toolEnd') {
    const matcher = matcherProblem(form.matcher);

    if (matcher) {
      found.push({ field: 'matcher', message: `hooks.editor.matcher.${matcher}`, params: { max: LIMITS.matcher }, required: false });
    }
  }

  if (form.kind === 'command') {
    found.push(...commandProblems(form));
  } else {
    const text = form.text.trim();

    if (text === '') {
      found.push({ field: 'text', message: 'hooks.editor.text.required', required: true });
    } else if (length(text) > LIMITS.text) {
      found.push({ field: 'text', message: 'hooks.editor.text.long', params: { max: LIMITS.text }, required: false });
    }
  }

  if (form.scope === 'project' && !context.projectOpen) {
    found.push({ field: 'scope', message: 'hooks.editor.scope.noProject', required: false });
  } else if (context.scopeFull) {
    found.push({ field: 'scope', message: 'hooks.editor.scope.full', params: { max: 64 }, required: false });
  }

  return found;
}

function commandProblems(form: HookForm): Problem[] {
  const found: Problem[] = [];
  const command = form.command.trim();

  if (command === '') {
    found.push({ field: 'command', message: 'hooks.editor.command.required', required: true });
  } else if (length(command) > LIMITS.command) {
    found.push({ field: 'command', message: 'hooks.editor.command.long', params: { max: LIMITS.command }, required: false });
  }

  if (timeoutProblem(form.timeout)) {
    found.push({ field: 'timeout', message: 'hooks.editor.timeout.range', params: { min: LIMITS.timeoutMin, max: LIMITS.timeoutMax }, required: false });
  }

  return found;
}

/** What `hook_save` takes. An edit names the file and the version it started from, so a change made meanwhile is a conflict, not a lost update. */
export function draftOf(form: HookForm, edit: Pick<HookView, 'id' | 'sha256'> | null, project: string | null): HookDraft {
  const trimmedTimeout = form.timeout.trim();
  const description = form.description.trim();
  const matcher = form.event === 'toolEnd' ? form.matcher.trim() : '';

  return {
    id: edit?.id ?? null,
    expectedSha256: edit?.sha256 ?? null,
    name: form.name.trim(),
    description: description === '' ? null : description,
    scope: form.scope,
    project: form.scope === 'project' ? project : null,
    event: form.event,
    matcher: matcher === '' ? null : matcher,
    action:
      form.kind === 'command'
        ? { type: 'command', command: form.command.trim(), timeoutSecs: trimmedTimeout === '' ? null : Number(trimmedTimeout) }
        : { type: form.kind, text: form.text.trim() },
    enabled: form.enabled,
  };
}
