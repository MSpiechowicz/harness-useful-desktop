import type { MessageKey } from '$lib/i18n/locales/en';
import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HookScope } from '$lib/ipc/bindings/HookScope';
import type { Routine } from '$lib/ipc/bindings/Routine';
import type { RoutineAction } from '$lib/ipc/bindings/RoutineAction';
import type { RoutineDraft } from '$lib/ipc/bindings/RoutineDraft';
import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
import type { RoutineWhen } from '$lib/ipc/bindings/RoutineWhen';
import type { RoutineWhenKind } from '$lib/ipc/bindings/RoutineWhenKind';
import type { ScriptScope } from '$lib/ipc/bindings/ScriptScope';

import { revealHidden } from '../memory/script-format';

import { abilitiesOf } from './routine-abilities';
import type { ActionKind } from './routine-format';

/** The limits of the routine format (`docs/routines-format.md`). */
export const LIMITS = {
  name: 80,
  description: 500,
  prompt: 8192,
  command: 2048,
  cron: 100,
  model: 200,
  everyMinutes: { min: 1, max: 720 },
  everyHours: { min: 1, max: 24 },
  maxMinutes: { min: 1, max: 240 },
  timeoutSecs: { min: 1, max: 3600 },
} as const;

/** How long an agent may work when the routine names no time, and how long a command may run. */
export const DEFAULT_MAX_MINUTES = 30;
export const DEFAULT_TIMEOUT_SECS = 300;

/** The time a new daily routine starts at. */
export const DEFAULT_AT = '09:00';

const AT = /^([01][0-9]|2[0-3]):[0-5][0-9]$/;
const SCRIPT_NAME = /^[a-z0-9][a-z0-9_-]{0,63}$/;
const WHOLE_NUMBER = /^\d+$/;

/** What the editor opens on: a routine to edit, or a new routine in a scope. */
export interface EditorTarget {
  view: RoutineView | null;
  scope: HookScope;
}

/** What the editor holds: every field as typed, so switching the schedule or the action loses nothing. */
export interface RoutineForm {
  name: string;
  description: string;
  whenKind: RoutineWhenKind;
  /** `HH:MM` of a daily or weekdays routine. */
  at: string;
  /** The number of minutes or hours as typed. */
  every: string;
  cron: string;
  kind: ActionKind;
  harness: HarnessId;
  prompt: string;
  /** The model as typed, a provider and an id; both empty for the harness's own default. */
  provider: string;
  modelId: string;
  /** OMP only: null keeps the harness's own setting. */
  approvalMode: ApprovalMode | null;
  /** PI only. */
  trustOnce: boolean;
  /** Minutes as typed; empty means the default. */
  maxMinutes: string;
  command: string;
  /** Seconds as typed; empty means the default. */
  timeout: string;
  scriptName: string;
  scriptScope: ScriptScope;
  scope: HookScope;
  enabled: boolean;
}

export type FieldKey = 'name' | 'description' | 'at' | 'every' | 'cron' | 'prompt' | 'model' | 'maxMinutes' | 'command' | 'timeout' | 'script' | 'scope';

/** Why a field is not acceptable. A `required` one is only said once the user has been at the field. */
export interface Problem {
  field: FieldKey;
  message: MessageKey;
  params?: Record<string, number>;
  required: boolean;
}

export function blankForm(scope: HookScope = 'global'): RoutineForm {
  return {
    name: '',
    description: '',
    whenKind: 'daily',
    at: DEFAULT_AT,
    every: '15',
    cron: '',
    kind: 'agent',
    harness: 'omp',
    prompt: '',
    provider: '',
    modelId: '',
    approvalMode: null,
    trustOnce: false,
    maxMinutes: '',
    command: '',
    timeout: '',
    scriptName: '',
    scriptScope: 'global',
    scope,
    enabled: true,
  };
}

/** The editor for a routine that exists. */
export function formOf(routine: Routine): RoutineForm {
  const form: RoutineForm = { ...blankForm(routine.scope), name: routine.name, description: routine.description ?? '', enabled: routine.enabled };

  return { ...form, ...whenFields(routine.when), ...actionFields(routine.action) };
}

function whenFields(when: RoutineWhen): Partial<RoutineForm> {
  switch (when.type) {
    case 'daily':
    case 'weekdays':
      return { whenKind: when.type, at: when.at };
    case 'everyMinutes':
    case 'everyHours':
      return { whenKind: when.type, every: String(when.every) };
    case 'cron':
      return { whenKind: 'cron', cron: when.expression };
    case 'appStart':
      return { whenKind: 'appStart' };
  }
}

function actionFields(action: RoutineAction): Partial<RoutineForm> {
  switch (action.type) {
    case 'agent':
      return {
        kind: 'agent',
        harness: action.harness,
        prompt: action.prompt,
        provider: action.model?.provider ?? '',
        modelId: action.model?.id ?? '',
        approvalMode: action.approvalMode,
        trustOnce: action.trustOnce,
        maxMinutes: action.maxMinutes === null ? '' : String(action.maxMinutes),
      };
    case 'command':
      return { kind: 'command', command: action.command, timeout: action.timeoutSecs === null ? '' : String(action.timeoutSecs) };
    case 'script':
      return { kind: 'script', scriptName: action.name, scriptScope: action.scope };
  }
}

/** Moves the routine to another harness. What only the other one has (an approval mode, a one-time trust) is dropped. */
export function withHarness(form: RoutineForm, harness: HarnessId): RoutineForm {
  const abilities = abilitiesOf(harness);

  return { ...form, harness, approvalMode: abilities.approvalMode ? form.approvalMode : null, trustOnce: abilities.trustOnce ? form.trustOnce : false };
}

/** Moves the routine to another scope. A global routine has no repository to look a script up in. */
export function withScope(form: RoutineForm, scope: HookScope): RoutineForm {
  return { ...form, scope, scriptScope: scope === 'global' ? 'global' : form.scriptScope };
}

function isWhole(value: string, min: number, max: number): boolean {
  const text = value.trim();

  return WHOLE_NUMBER.test(text) && Number(text) >= min && Number(text) <= max;
}

/** The schedule the form describes, or null while a field of it is not acceptable (the preview waits then). */
export function whenOf(form: RoutineForm): RoutineWhen | null {
  switch (form.whenKind) {
    case 'daily':
    case 'weekdays':
      return AT.test(form.at) ? { type: form.whenKind, at: form.at } : null;
    case 'everyMinutes':
      return isWhole(form.every, LIMITS.everyMinutes.min, LIMITS.everyMinutes.max) ? { type: 'everyMinutes', every: Number(form.every.trim()) } : null;
    case 'everyHours':
      return isWhole(form.every, LIMITS.everyHours.min, LIMITS.everyHours.max) ? { type: 'everyHours', every: Number(form.every.trim()) } : null;
    case 'cron': {
      const expression = form.cron.trim();

      return expression === '' || [...expression].length > LIMITS.cron ? null : { type: 'cron', expression };
    }
    case 'appStart':
      return { type: 'appStart' };
  }
}

/** The number of Unicode code points, which is how the format counts characters. */
export function length(text: string): number {
  return [...text].length;
}

/** Whether a prompt or command holds what the format refuses: an invisible character, or a control character other than tab and line breaks. */
export function hasHiddenCharacters(text: string): boolean {
  return revealHidden(text).count > 0;
}

export interface FormContext {
  /** A project is open, so a routine can be made for it. */
  projectOpen: boolean;
  /** The scope the routine would be in already holds the most it can. */
  scopeFull: boolean;
}

/** Everything that keeps the form from being saved, in the order of the fields. */
export function problemsOf(form: RoutineForm, context: FormContext): Problem[] {
  const found = [...nameProblems(form), ...whenProblems(form), ...actionProblems(form)];

  if (form.scope === 'project' && !context.projectOpen) {
    found.push({ field: 'scope', message: 'routines.editor.scope.noProject', required: false });
  } else if (context.scopeFull) {
    found.push({ field: 'scope', message: 'routines.editor.scope.full', params: { max: 64 }, required: false });
  }

  return found;
}

function nameProblems(form: RoutineForm): Problem[] {
  const found: Problem[] = [];
  const name = form.name.trim();

  if (name === '') {
    found.push({ field: 'name', message: 'routines.editor.name.required', required: true });
  } else if (length(name) > LIMITS.name) {
    found.push({ field: 'name', message: 'routines.editor.name.long', params: { max: LIMITS.name }, required: false });
  } else if (/\p{Cc}/u.test(name)) {
    found.push({ field: 'name', message: 'routines.editor.name.control', required: false });
  }

  if (length(form.description.trim()) > LIMITS.description) {
    found.push({ field: 'description', message: 'routines.editor.description.long', params: { max: LIMITS.description }, required: false });
  }

  return found;
}

function whenProblems(form: RoutineForm): Problem[] {
  switch (form.whenKind) {
    case 'daily':
    case 'weekdays':
      return AT.test(form.at) ? [] : [{ field: 'at', message: 'routines.editor.at.invalid', required: false }];
    case 'everyMinutes':
      return isWhole(form.every, LIMITS.everyMinutes.min, LIMITS.everyMinutes.max) ? [] : [{ field: 'every', message: 'routines.editor.every.range', params: LIMITS.everyMinutes, required: false }];
    case 'everyHours':
      return isWhole(form.every, LIMITS.everyHours.min, LIMITS.everyHours.max) ? [] : [{ field: 'every', message: 'routines.editor.every.range', params: LIMITS.everyHours, required: false }];
    case 'cron':
      return cronProblems(form.cron.trim());
    case 'appStart':
      return [];
  }
}

/** The cron grammar is checked by the backend (the preview says what is wrong with it): here only what is plain to see. */
function cronProblems(expression: string): Problem[] {
  if (expression === '') {
    return [{ field: 'cron', message: 'routines.editor.cron.required', required: true }];
  }

  return length(expression) > LIMITS.cron ? [{ field: 'cron', message: 'routines.editor.cron.long', params: { max: LIMITS.cron }, required: false }] : [];
}

function actionProblems(form: RoutineForm): Problem[] {
  switch (form.kind) {
    case 'agent':
      return agentProblems(form);
    case 'command':
      return commandProblems(form);
    case 'script':
      return scriptProblems(form);
  }
}

function agentProblems(form: RoutineForm): Problem[] {
  const found: Problem[] = [];
  const prompt = form.prompt.trim();

  if (prompt === '') {
    found.push({ field: 'prompt', message: 'routines.editor.prompt.required', required: true });
  } else if (length(prompt) > LIMITS.prompt) {
    found.push({ field: 'prompt', message: 'routines.editor.prompt.long', params: { max: LIMITS.prompt }, required: false });
  } else if (hasHiddenCharacters(prompt)) {
    found.push({ field: 'prompt', message: 'routines.editor.hidden', required: false });
  }

  found.push(...modelProblems(form));

  if (form.maxMinutes.trim() !== '' && !isWhole(form.maxMinutes, LIMITS.maxMinutes.min, LIMITS.maxMinutes.max)) {
    found.push({ field: 'maxMinutes', message: 'routines.editor.maxMinutes.range', params: LIMITS.maxMinutes, required: false });
  }

  return found;
}

function modelProblems(form: RoutineForm): Problem[] {
  const provider = form.provider.trim();
  const id = form.modelId.trim();

  if ((provider === '') !== (id === '')) {
    return [{ field: 'model', message: 'routines.editor.model.both', required: false }];
  }

  return length(provider) > LIMITS.model || length(id) > LIMITS.model ? [{ field: 'model', message: 'routines.editor.model.long', params: { max: LIMITS.model }, required: false }] : [];
}

function commandProblems(form: RoutineForm): Problem[] {
  const found: Problem[] = [];
  const command = form.command.trim();

  if (command === '') {
    found.push({ field: 'command', message: 'routines.editor.command.required', required: true });
  } else if (length(command) > LIMITS.command) {
    found.push({ field: 'command', message: 'routines.editor.command.long', params: { max: LIMITS.command }, required: false });
  } else if (hasHiddenCharacters(command)) {
    found.push({ field: 'command', message: 'routines.editor.hidden', required: false });
  }

  if (form.timeout.trim() !== '' && !isWhole(form.timeout, LIMITS.timeoutSecs.min, LIMITS.timeoutSecs.max)) {
    found.push({ field: 'timeout', message: 'routines.editor.timeout.range', params: LIMITS.timeoutSecs, required: false });
  }

  return found;
}

function scriptProblems(form: RoutineForm): Problem[] {
  const name = form.scriptName.trim();

  if (name === '') {
    return [{ field: 'script', message: 'routines.editor.script.required', required: true }];
  }

  if (!SCRIPT_NAME.test(name)) {
    return [{ field: 'script', message: 'routines.editor.script.name', required: false }];
  }

  return form.scope === 'global' && form.scriptScope === 'repo' ? [{ field: 'script', message: 'routines.editor.script.repoNeedsProject', required: false }] : [];
}

/** Whether the routine would run tools without asking: the agent is set to "yolo" or trusts the project's resources. */
export function runsUnattended(form: Pick<RoutineForm, 'kind' | 'harness' | 'approvalMode' | 'trustOnce'>): boolean {
  if (form.kind !== 'agent') {
    return false;
  }

  const abilities = abilitiesOf(form.harness);

  return (abilities.approvalMode && form.approvalMode === 'yolo') || (abilities.trustOnce && form.trustOnce);
}

function actionOf(form: RoutineForm): RoutineAction {
  if (form.kind === 'command') {
    const timeout = form.timeout.trim();

    return { type: 'command', command: form.command.trim(), timeoutSecs: timeout === '' ? null : Number(timeout) };
  }

  if (form.kind === 'script') {
    return { type: 'script', name: form.scriptName.trim(), scope: form.scope === 'global' ? 'global' : form.scriptScope };
  }

  const provider = form.provider.trim();
  const id = form.modelId.trim();
  const maxMinutes = form.maxMinutes.trim();
  const abilities = abilitiesOf(form.harness);

  return {
    type: 'agent',
    harness: form.harness,
    model: provider === '' ? null : { provider, id },
    prompt: form.prompt.trim(),
    approvalMode: abilities.approvalMode ? form.approvalMode : null,
    trustOnce: abilities.trustOnce && form.trustOnce,
    maxMinutes: maxMinutes === '' ? null : Number(maxMinutes),
  };
}

/** What `routine_save` takes, for a form without problems (see `problemsOf`). An edit names the file and the version it started from, so a change made meanwhile is a conflict, not a lost update. */
export function draftOf(form: RoutineForm, edit: Pick<RoutineView, 'id' | 'sha256'> | null, project: string | null): RoutineDraft {
  const description = form.description.trim();
  const when = whenOf(form);

  if (when === null) {
    throw new Error('the schedule is not valid');
  }

  return {
    id: edit?.id ?? null,
    expectedSha256: edit?.sha256 ?? null,
    name: form.name.trim(),
    description: description === '' ? null : description,
    scope: form.scope,
    project: form.scope === 'project' ? project : null,
    when,
    action: actionOf(form),
    enabled: form.enabled,
  };
}

/** The draft that saves a routine as it is with its schedule switched on or off: what a switch writes. */
export function toggleDraft(view: RoutineView, routine: Routine, enabled: boolean): RoutineDraft {
  return {
    id: routine.id,
    expectedSha256: view.sha256,
    name: routine.name,
    description: routine.description,
    scope: routine.scope,
    project: routine.project,
    when: routine.when,
    action: routine.action,
    enabled,
  };
}
