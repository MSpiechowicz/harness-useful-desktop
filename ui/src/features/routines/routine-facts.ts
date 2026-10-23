import { type FactRow, momentFact } from '$lib/components/facts';
import { t } from '$lib/i18n/i18n.svelte';
import type { MessageKey } from '$lib/i18n/locales/en';
import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
import type { HookApproval } from '$lib/ipc/bindings/HookApproval';
import type { Routine } from '$lib/ipc/bindings/Routine';
import type { RoutineAction } from '$lib/ipc/bindings/RoutineAction';
import type { RoutineView } from '$lib/ipc/bindings/RoutineView';

import { formatDateTime } from '$lib/utils/format';

import { spelledOut } from '../memory/script-format';

import { abilitiesOf } from './routine-abilities';
import { ACTION_LABELS, HARNESS_NAMES, untilTime, whenText } from './routine-format';
import { DEFAULT_MAX_MINUTES, DEFAULT_TIMEOUT_SECS } from './routine-form';
import { repoName } from './routine-team';

const MODE_LABELS: Record<ApprovalMode, MessageKey> = {
  alwaysAsk: 'routines.mode.alwaysAsk',
  write: 'routines.mode.write',
  yolo: 'routines.mode.yolo',
  auto: 'routines.mode.auto',
};

/** The words for the approval mode of an agent action, and for an unset one. */
export function modeText(mode: ApprovalMode | null): string {
  return t(mode === null ? 'routines.mode.harness' : MODE_LABELS[mode]);
}

/**
 * Who made the routine: the user or an agent. A routine an agent rewrote and that waits is the agent's, whoever created it.
 */
function sourceOf(routine: Routine, approval: HookApproval): string {
  const byAgent = routine.createdBy === 'agent' || (approval !== 'approved' && routine.updatedBy === 'agent');

  return t(byAgent ? 'routines.source.agent' : 'routines.source.you');
}

/** How an agent routine asks for approval, in the rows its harness has: an approval mode, a one-time trust of the project, or both. */
function approvalFacts(action: Extract<RoutineAction, { type: 'agent' }>): FactRow[] {
  const abilities = abilitiesOf(action.harness);
  const rows: FactRow[] = [];

  if (abilities.approvalMode) {
    rows.push({ label: t('routines.field.approvalMode'), value: modeText(action.approvalMode), tone: action.approvalMode === 'yolo' || action.approvalMode === 'auto' ? 'warning' : undefined });
  }

  if (abilities.trustOnce) {
    rows.push({ label: t('routines.field.trust'), value: t(action.trustOnce ? 'routines.value.trustOnce' : 'routines.value.askFirst'), tone: action.trustOnce ? 'warning' : undefined });
  }

  return rows;
}

/** What an agent routine does besides its prompt: the harness, the model and its thinking level, how it asks for approval and how long it may work. */
function agentFacts(action: Extract<RoutineAction, { type: 'agent' }>): FactRow[] {
  const thinking = action.model?.thinking;

  return [
    { label: t('routines.field.harness'), value: HARNESS_NAMES[action.harness] },
    { label: t('routines.field.model'), value: action.model ? `${action.model.provider}/${action.model.id}` : t('routines.value.defaultModel'), mono: action.model !== null },
    // Only a routine that sets a level says so: the others leave it to the model.
    ...(thinking ? [{ label: t('routines.field.thinking'), value: thinking }] : []),
    ...approvalFacts(action),
    { label: t('routines.field.maxMinutes'), value: t('routines.value.minutes', { count: action.maxMinutes ?? DEFAULT_MAX_MINUTES }) },
  ];
}

function actionFacts(action: RoutineAction): FactRow[] {
  switch (action.type) {
    case 'agent':
      return agentFacts(action);
    case 'command':
      return [{ label: t('routines.field.timeout'), value: t('routines.value.seconds', { count: action.timeoutSecs ?? DEFAULT_TIMEOUT_SECS }) }];
    case 'script':
      return [{ label: t('routines.field.script'), value: spelledOut(action.name), mono: true }, { label: t('routines.field.scriptScope'), value: t(action.scope === 'repo' ? 'routines.value.scriptRepo' : 'routines.value.scriptGlobal') }];
  }
}

/**
 * The Details card of a routine: the same facts in the same order for every routine, with the ones that do not apply left out.
 * A team routine (`team` is its view) names its repository instead of a folder, and who shared it and when instead of who wrote it. `scope` says where it applies (see `docScope`).
 */
export function routineFacts(routine: Routine, approval: HookApproval, view: RoutineView, scope: string, now: number = Date.now()): FactRow[] {
  const team = view.origin === 'team';
  const project = routine.scope === 'project' ? routine.project : null;
  // Where a project routine applies: a team routine names its repository (`acme/app`), a local one its folder.
  const where = project ? (team ? repoName(view.teamRepo ?? project) : project) : null;

  return [
    { label: t('routines.field.when'), value: whenText(routine.when), mono: routine.when.type === 'cron' },
    ...(view.nextRun ? [{ label: t('routines.field.nextRun'), value: untilTime(view.nextRun, now), title: formatDateTime(view.nextRun) }] : []),
    { label: t('routines.field.does'), value: t(ACTION_LABELS[routine.action.type]) },
    ...actionFacts(routine.action),
    { label: t('routines.field.appliesTo'), value: spelledOut(scope) },
    // The whole path, since a short name does not tell two folders apart, and an agent can move a routine to another one.
    ...(where ? [{ label: t(team ? 'routines.field.repository' : 'routines.field.project'), value: spelledOut(where), mono: true, cutStart: true, title: spelledOut(where) }] : []),
    ...(team && view.teamFolder ? [{ label: t('routines.field.runsIn'), value: spelledOut(view.teamFolder), mono: true, cutStart: true, title: spelledOut(view.teamFolder) }] : []),
    { label: t('routines.field.source'), value: team ? t('routines.source.team') : sourceOf(routine, approval) },
    ...(team ? teamMoments(view) : [momentFact(t('routines.field.changed'), routine.updatedAt)]),
  ];
}

/** Who shared a team routine and when its team copy last changed. */
function teamMoments(team: RoutineView): FactRow[] {
  return [
    ...(team.teamAuthor ? [{ label: t('routines.field.sharedBy'), value: spelledOut(team.teamAuthor) }] : []),
    ...(team.teamUpdatedAt ? [momentFact(t('routines.field.teamUpdated'), team.teamUpdatedAt)] : []),
    ...(team.teamChanged ? [{ label: t('routines.field.yourRoutine'), value: t('routines.value.changedSinceShared'), tone: 'warning' as const }] : []),
  ];
}
