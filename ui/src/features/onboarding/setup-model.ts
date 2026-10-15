import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
import type { DashboardSetup } from '$lib/ipc/bindings/DashboardSetup';
import type { ExtensionAction } from '$lib/ipc/bindings/ExtensionAction';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSetup } from '$lib/ipc/bindings/HarnessSetup';
import type { McpRefusal } from '$lib/ipc/bindings/McpRefusal';
import type { McpState } from '$lib/ipc/bindings/McpState';

/** What one item of the step is doing: `idle` until a run reaches it. */
export type ItemPhase = 'idle' | 'running' | 'done' | 'failed' | 'skipped';

/** The things the step can set up: the dashboard, and per harness its skills and its MCP entry. */
export type SetupKey = 'dashboard' | `skills:${HarnessId}` | `mcp:${HarnessId}`;

export interface Chip {
  tone: 'success' | 'warning' | 'neutral' | 'danger' | 'accent';
  label: string;
  /** A spinner instead of the dot: the item is being set up right now. */
  busy?: boolean;
  /** Nothing to say about the item (its harness is not installed): a quiet dash instead of a badge; `label` is its text for assistive technology. */
  none?: boolean;
}

/** The JSON a person adds to an MCP file when the app leaves it alone, one key per line so it never wraps mid-token. */
export const MCP_SNIPPET = `{
  "mcpServers": {
    "harness-dashboard": {
      "command": "harness-dashboard",
      "args": ["mcp"]
    }
  }
}`;

/** The harness CLI line an action stands for, as shown in the confirmation. */
export function commandOf(harness: HarnessId, action: ExtensionAction): string {
  switch (action.type) {
    case 'pluginInstall':
      return `${harness} plugin install ${action.target} --scope ${action.scope}`;
    case 'marketplaceAdd':
      return `${harness} plugin marketplace add ${action.source}`;
    case 'packageInstall':
      return `${harness} install ${action.source}`;
    default:
      return `${harness} ${action.type}`;
  }
}

/** A refusal with no file means the harness has no MCP file the app could edit. */
export function isUnsupported(mcp: McpState): boolean {
  return mcp.type === 'refused' && mcp.file === '';
}

const REFUSAL_KEYS: Record<McpRefusal, MessageKey> = {
  unparsable: 'onboarding.setup.refused.unparsable',
  notAnObject: 'onboarding.setup.refused.notAnObject',
  serversNotAnObject: 'onboarding.setup.refused.serversNotAnObject',
  symlink: 'onboarding.setup.refused.symlink',
  notRegularFile: 'onboarding.setup.refused.notRegularFile',
  tooLarge: 'onboarding.setup.refused.tooLarge',
  unreadable: 'onboarding.setup.refused.unreadable',
  writeFailed: 'onboarding.setup.refused.writeFailed',
};

/** Why the app leaves an MCP file alone, in a sentence. */
export function refusalText(reason: McpRefusal): string {
  return t(REFUSAL_KEYS[reason]);
}

/** The chip of a running or finished item, or null while it shows the state read from the machine. */
function runChip(phase: ItemPhase, waiting: boolean): Chip | null {
  switch (phase) {
    case 'running':
      return { tone: 'accent', label: t('onboarding.setup.chip.running'), busy: true };
    case 'done':
      return { tone: 'success', label: t('onboarding.setup.chip.done') };
    case 'failed':
      return { tone: 'danger', label: t('onboarding.setup.chip.failed') };
    case 'skipped':
      return { tone: 'neutral', label: t('onboarding.setup.chip.skipped') };
    case 'idle':
      return waiting ? { tone: 'neutral', label: t('onboarding.setup.chip.queued') } : null;
  }
}

/** A harness row: a running or finished item first, then "not installed", then the state read from the machine. */
function rowChip(setup: HarnessSetup, phase: ItemPhase, waiting: boolean, state: () => Chip): Chip {
  return runChip(phase, waiting) ?? (setup.installed ? state() : { tone: 'neutral', label: t('onboarding.setup.chip.notInstalled'), none: true });
}

export function skillsChip(setup: HarnessSetup, phase: ItemPhase, waiting: boolean): Chip {
  return rowChip(setup, phase, waiting, () => {
    const skills = setup.skills;

    switch (skills.type) {
      case 'installed':
        return { tone: 'success', label: t('onboarding.setup.chip.installed') };
      case 'install':
        return { tone: 'warning', label: t('onboarding.setup.chip.missing') };
      case 'manual':
        return { tone: 'neutral', label: t('onboarding.setup.chip.manual') };
      case 'unavailable':
        return { tone: 'neutral', label: t('onboarding.setup.chip.unavailable') };
    }
  });
}

export function mcpChip(setup: HarnessSetup, phase: ItemPhase, waiting: boolean): Chip {
  return rowChip(setup, phase, waiting, () => {
    const mcp = setup.mcp;

    switch (mcp.type) {
      case 'configured':
        return { tone: 'success', label: t('onboarding.setup.chip.installed') };
      case 'missing':
        return { tone: 'warning', label: t('onboarding.setup.chip.missing') };
      case 'disabled':
        return { tone: 'warning', label: t('onboarding.setup.chip.disabled') };
      case 'refused':
        return isUnsupported(mcp) ? { tone: 'neutral', label: t('onboarding.setup.chip.unsupported') } : { tone: 'danger', label: t('onboarding.setup.chip.refused') };
    }
  });
}

/** The state of the dashboard binary, or what a run is doing to it. */
export function dashboardChip(dashboard: DashboardSetup, phase: ItemPhase, waiting: boolean): Chip {
  const running = runChip(phase, waiting);
  if (running) {
    return running;
  }

  if (dashboard.installed) {
    return { tone: 'success', label: t('onboarding.setup.chip.installed') };
  }

  if (!dashboard.supportedOnThisOs) {
    return { tone: 'neutral', label: t('onboarding.setup.chip.unavailable') };
  }

  return { tone: 'warning', label: t('onboarding.setup.chip.notInstalled') };
}
