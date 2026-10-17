import { type FactRow, momentFact } from '$lib/components/facts';
import { t } from '$lib/i18n/i18n.svelte';
import type { Hook } from '$lib/ipc/bindings/Hook';
import type { HookApproval } from '$lib/ipc/bindings/HookApproval';

import { spelledOut } from '../memory/script-format';

import { ACTION_LABELS, EVENT_LABELS, folderName, SCOPE_LABELS } from './hook-format';
import { DEFAULT_TIMEOUT_SECS } from './hook-form';

/**
 * Who made the hook: the user, an agent, or the Harness family catalog. A family hook is the catalog's only while the
 * version on disk is one the user approved. Once an agent rewrote it and it waits, it is the agent's.
 */
function sourceOf(hook: Hook, approval: HookApproval): string {
  if (hook.seed && approval === 'approved') {
    return t('hooks.source.family');
  }

  const byAgent = hook.createdBy === 'agent' || (hook.seed !== null && hook.updatedBy === 'agent');

  return t(byAgent ? 'hooks.source.agent' : 'hooks.source.you');
}

/** The Details card of a hook: the same facts in the same order for every hook, with the ones that do not apply left out. */
export function hookFacts(hook: Hook, approval: HookApproval): FactRow[] {
  const command = hook.action.type === 'command' ? hook.action : null;
  const project = hook.scope === 'project' ? hook.project : null;
  const scope = project ? `${t(SCOPE_LABELS.project)} · ${spelledOut(folderName(project))}` : t(SCOPE_LABELS[hook.scope]);

  return [
    { label: t('hooks.field.when'), value: t(EVENT_LABELS[hook.event]) },
    ...(hook.event === 'toolEnd' ? [{ label: t('hooks.field.toolFilter'), value: hook.matcher ?? t('hooks.value.everyTool'), mono: hook.matcher !== null }] : []),
    { label: t('hooks.field.does'), value: t(ACTION_LABELS[hook.action.type]) },
    ...(command ? [{ label: t('hooks.field.timeout'), value: t('hooks.value.seconds', { count: command.timeoutSecs ?? DEFAULT_TIMEOUT_SECS }) }] : []),
    { label: t('hooks.field.appliesTo'), value: scope },
    // The whole path, since a short name does not tell two folders apart, and an agent can move a hook to another one.
    ...(project ? [{ label: t('hooks.field.project'), value: spelledOut(project), mono: true, cutStart: true, title: spelledOut(project) }] : []),
    { label: t('hooks.field.source'), value: sourceOf(hook, approval) },
    momentFact(t('hooks.field.changed'), hook.updatedAt),
  ];
}
