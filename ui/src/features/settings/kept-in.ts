import type { StripOption } from '$lib/components/strip';
import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';

/** The rows of "Kept in". Each has its own hints; hooks and routines keep what other people share as a second place next to this computer. */
export type KeptInItem = 'notes' | 'scripts' | 'hooks' | 'routines';

/** The three places, in the order every row shows them. */
const PLACES = ['remote', 'local', 'remoteLocal'] as const satisfies readonly MemoryNotesRouting[];

const LABELS = {
  remote: 'settings.memoryRemote.target.remote',
  local: 'settings.memoryRemote.target.local',
  remoteLocal: 'settings.memoryRemote.target.remoteLocal',
} as const satisfies Record<MemoryNotesRouting, MessageKey>;

/**
 * What a place cannot do for a row: a reason to show on hover, or null to grey it out without one (the notice under the row says
 * what to do). Every row gets the same three options with the same labels, so the controls line up.
 */
export type Unavailable = Partial<Record<MemoryNotesRouting, string | null>>;

export function keptInOptions(unavailable: Unavailable): StripOption<MemoryNotesRouting>[] {
  return PLACES.map((value) => {
    const reason = unavailable[value];

    return { value, label: t(LABELS[value]), disabled: reason !== undefined, ...(reason ? { reason } : {}) };
  });
}

/** The line under a row's control, per place. */
export function keptInHints(item: KeptInItem): Record<MemoryNotesRouting, string> {
  return {
    remote: t(`settings.memoryRemote.hint.${item}.remote`),
    local: t(`settings.memoryRemote.hint.${item}.local`),
    remoteLocal: t(`settings.memoryRemote.hint.${item}.remoteLocal`),
  };
}
