import { diffFields, type DocChange } from '$lib/components/docs/doc-diff';
import { t } from '$lib/i18n/i18n.svelte';
import type { Routine } from '$lib/ipc/bindings/Routine';

import { abilitiesOf } from './routine-abilities';
import { modeText } from './routine-facts';
import { ACTION_LABELS, actionText, HARNESS_NAMES, whenText } from './routine-format';
import { DEFAULT_MAX_MINUTES, DEFAULT_TIMEOUT_SECS } from './routine-form';

/** The parts of a routine that the review compares. */
export type ChangedField =
  | 'name'
  | 'description'
  | 'when'
  | 'does'
  | 'harness'
  | 'model'
  | 'approval'
  | 'maxMinutes'
  | 'text'
  | 'timeout'
  | 'scriptScope'
  | 'scope'
  | 'project'
  | 'enabled';

/** One part that differs between the version the user approved and the one waiting. `from` is null when there was no approved version, or the part was empty. */
export type FieldChange = DocChange<ChangedField>;

/** The ways to ask for approval, as the review words them. */
function approvalValue(routine: Routine): string | null {
  const action = routine.action;

  if (action.type !== 'agent') {
    return null;
  }

  const abilities = abilitiesOf(action.harness);
  const ways: string[] = [];

  if (abilities.approvalMode) {
    ways.push(modeText(action.approvalMode));
  }

  if (abilities.trustOnce) {
    ways.push(t(action.trustOnce ? 'routines.value.trustOnce' : 'routines.value.askFirst'));
  }

  return ways.length === 0 ? null : ways.join(', ');
}

/** The values of a routine as the review shows them: written for a reader, except the parts that are text, code or a path (`scopeOf` names where it applies). The empty ones are null. */
function valuesOf(routine: Routine, scopeOf: (routine: Routine) => string): Record<ChangedField, string | null> {
  const action = routine.action;
  const agent = action.type === 'agent' ? action : null;

  return {
    name: routine.name,
    description: routine.description || null,
    when: whenText(routine.when),
    does: t(ACTION_LABELS[action.type]),
    harness: agent ? HARNESS_NAMES[agent.harness] : null,
    model: agent ? (agent.model ? `${agent.model.provider}/${agent.model.id}` : t('routines.value.defaultModel')) : null,
    approval: approvalValue(routine),
    maxMinutes: agent ? t('routines.value.minutes', { count: agent.maxMinutes ?? DEFAULT_MAX_MINUTES }) : null,
    text: actionText(action),
    timeout: action.type === 'command' ? t('routines.value.seconds', { count: action.timeoutSecs ?? DEFAULT_TIMEOUT_SECS }) : null,
    scriptScope: action.type === 'script' ? t(action.scope === 'repo' ? 'routines.value.scriptRepo' : 'routines.value.scriptGlobal') : null,
    scope: scopeOf(routine),
    project: routine.project,
    enabled: t(routine.enabled ? 'routines.value.on' : 'routines.value.off'),
  };
}

const ORDER: readonly ChangedField[] = ['name', 'description', 'when', 'does', 'harness', 'model', 'approval', 'maxMinutes', 'text', 'timeout', 'scriptScope', 'scope', 'project', 'enabled'];

/**
 * What differs between a waiting routine and the version the user approved. Without an approved version (an agent added the
 * routine) every part it sets is listed as new, so the review shows what the routine would do before it ever runs. `scopeOf` names
 * where a version applies, e.g. the project it moved to.
 */
export function changesOf(waiting: Routine, approved: Routine | null, scopeOf: (routine: Routine) => string): FieldChange[] {
  return diffFields(ORDER, valuesOf(waiting, scopeOf), approved ? valuesOf(approved, scopeOf) : null);
}
