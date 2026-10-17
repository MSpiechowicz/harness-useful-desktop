import type { Hook } from '$lib/ipc/bindings/Hook';

import { actionText } from './hook-format';

/** The parts of a hook that the review compares. */
export type ChangedField = 'name' | 'description' | 'event' | 'matcher' | 'action' | 'text' | 'timeout' | 'scope' | 'project' | 'enabled';

/** One part that differs between the version the user approved and the one waiting. `from` is null when there was no approved version, or the part was empty. */
export interface FieldChange {
  field: ChangedField;
  from: string | null;
  to: string | null;
}

/** The values of a hook as the review shows them; the empty ones are null. */
function valuesOf(hook: Hook): Record<ChangedField, string | null> {
  const action = hook.action;

  return {
    name: hook.name,
    description: hook.description || null,
    event: hook.event,
    matcher: hook.matcher || null,
    action: action.type,
    text: actionText(action),
    timeout: action.type === 'command' && action.timeoutSecs !== null ? String(action.timeoutSecs) : null,
    scope: hook.scope,
    project: hook.project,
    enabled: String(hook.enabled),
  };
}

const ORDER: readonly ChangedField[] = ['name', 'description', 'event', 'matcher', 'action', 'text', 'timeout', 'scope', 'project', 'enabled'];

/**
 * What differs between a waiting hook and the version the user approved. Without an approved version (an agent added the
 * hook) every part it sets is listed as new, so the review shows what the hook would do before it ever runs.
 */
export function changesOf(waiting: Hook, approved: Hook | null): FieldChange[] {
  const to = valuesOf(waiting);
  const from = approved ? valuesOf(approved) : null;

  return ORDER.flatMap((field) => {
    const before = from?.[field] ?? null;
    const after = to[field];

    if (from === null ? after === null : before === after) {
      return [];
    }

    return [{ field, from: before, to: after }];
  });
}
