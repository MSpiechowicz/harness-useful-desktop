import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import { harness } from '$lib/stores/harness.svelte';

/** What an agent routine can ask of a harness before it runs: how it asks for approval, and whether it trusts the project's own resources. */
export interface RoutineAbilities {
  /** The harness has approval modes (OMP, OpenCode), so the routine may name one. */
  approvalMode: boolean;
  /** The harness can run without asking, so the routine may name Yolo (Claude Code has approval modes but no Yolo). */
  yoloMode: boolean;
  /** The harness judges which tool calls are safe itself, so the routine may name Auto (Claude Code only). */
  autoMode: boolean;
  /** The harness gates project-local resources behind a trust (PI), so the routine may trust them once. */
  trustOnce: boolean;
}

/**
 * Read from the harness's capabilities, so a harness that gains them shows the fields without a change here. A harness the
 * list does not know yet is not asked about: it keeps whatever the routine holds, so a routine opened before the list arrives
 * loses nothing on save.
 */
export function abilitiesOf(id: HarnessId): RoutineAbilities {
  const capabilities = harness.get(id)?.capabilities;

  return {
    approvalMode: capabilities?.approvalModes ?? true,
    yoloMode: (capabilities?.approvalModes ?? true) && (capabilities?.yoloMode ?? true),
    autoMode: (capabilities?.approvalModes ?? true) && (capabilities?.autoMode ?? true),
    trustOnce: capabilities?.projectTrust ?? true,
  };
}

/** The approval mode a routine keeps on this harness: none without approval modes, and never Yolo or Auto where it is not offered. */
export function modeFor(abilities: RoutineAbilities, mode: ApprovalMode | null): ApprovalMode | null {
  if (!abilities.approvalMode || !offers(abilities, mode)) {
    return null;
  }

  return mode;
}

/** Whether the harness offers `mode`: Yolo and Auto only where its abilities say so, the harness setting and the others always. */
export function offers(abilities: RoutineAbilities, mode: ApprovalMode | 'harness' | null): boolean {
  if (mode === 'yolo') {
    return abilities.yoloMode;
  }

  if (mode === 'auto') {
    return abilities.autoMode;
  }

  return true;
}

/** The harnesses a routine can start a chat with, in the order the harness list has them, and the one the routine names already. */
export function routineHarnesses(current: HarnessId): HarnessId[] {
  const chatty = harness.summaries.filter((summary) => summary.capabilities.chat).map((summary) => summary.info.id);

  return chatty.includes(current) ? chatty : [current, ...chatty];
}
