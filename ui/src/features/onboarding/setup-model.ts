import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
import type { DashboardSetup } from '$lib/ipc/bindings/DashboardSetup';
import type { ExtensionAction } from '$lib/ipc/bindings/ExtensionAction';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSetup } from '$lib/ipc/bindings/HarnessSetup';
import type { McpRefusal } from '$lib/ipc/bindings/McpRefusal';
import type { McpState } from '$lib/ipc/bindings/McpState';

/** What one item of the step is doing: `idle` until a run reaches it. */
export type ItemPhase = 'idle' | 'running' | 'done' | 'failed' | 'skipped';

/** The companions the step installs from their own installers. */
export type Companion = 'dashboard' | 'memory';

/** The two MCP servers the step registers per harness: the dashboard's (`mcp`) and the memory server's (`memoryMcp`). */
export type McpKind = 'mcp' | 'memoryMcp';

/** The things the step can set up: the two companions, and per harness its skills and its two MCP entries. */
export type SetupKey = Companion | `skills:${HarnessId}` | `${McpKind}:${HarnessId}`;

/** The companion each MCP entry runs, which has to be installed before the entry can be added. */
export const MCP_COMPANION: Record<McpKind, Companion> = { mcp: 'dashboard', memoryMcp: 'memory' };

/** One line of what the confirmation lists. */
export interface PlannedCommand {
  /** Whose command it is: a companion or a harness. */
  owner: Companion | HarnessId;
  command: string;
}

/** A file the run will touch, whether it is created, and how many entries it gets. */
export interface PlannedFile {
  file: string;
  creates: boolean;
  entries: number;
}

/** What one companion's installer changes besides the binary, one sentence each. */
export interface PlannedEffects {
  owner: Companion;
  effects: string[];
}

/** Everything a run would do, shown before anything runs. */
export interface Plan {
  commands: PlannedCommand[];
  files: PlannedFile[];
  effects: PlannedEffects[];
}

/**
 * What a companion's installer changes: the backend's list, and for the memory server the entry it adds to the
 * harnesses' MCP config once installed (the backend does that after the installer, so its list does not say it).
 */
export function companionEffects(companion: Companion, server: DashboardSetup): string[] {
  return companion === 'memory' ? [...server.sideEffects, t('onboarding.setup.memory.registers')] : server.sideEffects;
}

/** The install of one companion as a plan: its installer command and what it changes. */
export function companionPlan(companion: Companion, server: DashboardSetup): Plan {
  return {
    commands: [{ owner: companion, command: server.command }],
    files: [],
    effects: [{ owner: companion, effects: companionEffects(companion, server) }],
  };
}

export interface Chip {
  tone: 'success' | 'warning' | 'neutral' | 'danger' | 'accent';
  label: string;
  /** A spinner instead of the dot: the item is being set up right now. */
  busy?: boolean;
  /** Nothing to say about the item (its harness is not installed): a quiet dash instead of a badge; `label` is its text for assistive technology. */
  none?: boolean;
}

/** The column title of each MCP entry. */
export function mcpTitle(kind: McpKind): string {
  return kind === 'mcp' ? t('onboarding.setup.mcp.title') : t('onboarding.setup.memoryMcp.title');
}

/** The server name and command of each MCP entry. */
const MCP_SERVER: Record<McpKind, string> = { mcp: 'harness-dashboard', memoryMcp: 'harness-memory' };

/** The JSON a person adds to an MCP file when the app leaves it alone, one key per line so it never wraps mid-token. */
export function mcpSnippet(kind: McpKind): string {
  const server = MCP_SERVER[kind];

  return `{
  "mcpServers": {
    "${server}": {
      "command": "${server}",
      "args": ["mcp"]
    }
  }
}`;
}

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
      return { tone: 'success', label: t('onboarding.setup.chip.installed') };
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

/** The chip of one of the harness's MCP entries (`mcp` the dashboard's, `memoryMcp` the memory server's). */
export function mcpChip(setup: HarnessSetup, kind: McpKind, phase: ItemPhase, waiting: boolean): Chip {
  return rowChip(setup, phase, waiting, () => {
    const mcp = setup[kind];

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

/** Harness Skills across every harness found: installed in all of them, in some, or in none yet. */
export function skillsOverviewChip(harnesses: HarnessSetup[]): Chip {
  const found = harnesses.filter((entry) => entry.installed);
  const have = found.filter((entry) => entry.skills.type === 'installed').length;

  if (found.length === 0) {
    return { tone: 'neutral', label: t('onboarding.setup.chip.notInstalled') };
  }

  if (have === found.length) {
    return { tone: 'success', label: t('onboarding.setup.chip.installed') };
  }

  return have > 0 ? { tone: 'warning', label: t('onboarding.setup.chip.partial') } : { tone: 'warning', label: t('onboarding.setup.chip.missing') };
}

/** The Harness Skills row: a running or finished install in any harness first, then the state across all of them. */
export function skillsRowChip(harnesses: HarnessSetup[], phase: ItemPhase, waiting: boolean): Chip {
  return runChip(phase, waiting) ?? skillsOverviewChip(harnesses);
}

/** The harnesses the skills row installs in: found ones that have the skills or can be given them. A manual step or none at all leaves a harness out. */
export function skillsTargets(harnesses: HarnessSetup[]): HarnessSetup[] {
  return harnesses.filter((entry) => entry.installed && (entry.skills.type === 'install' || entry.skills.type === 'installed'));
}

/** The state of a companion's binary, or what a run is doing to it. */
export function companionChip(companion: DashboardSetup, phase: ItemPhase, waiting: boolean): Chip {
  const running = runChip(phase, waiting);
  if (running) {
    return running;
  }

  if (companion.installed) {
    return { tone: 'success', label: t('onboarding.setup.chip.installed') };
  }

  if (!companion.supportedOnThisOs) {
    return { tone: 'neutral', label: t('onboarding.setup.chip.unavailable') };
  }

  return { tone: 'warning', label: t('onboarding.setup.chip.notInstalled') };
}
