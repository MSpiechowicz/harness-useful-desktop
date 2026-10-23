import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
import type { DashboardSetup } from '$lib/ipc/bindings/DashboardSetup';
import type { ExtensionAction } from '$lib/ipc/bindings/ExtensionAction';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSetup } from '$lib/ipc/bindings/HarnessSetup';
import type { McpRefusal } from '$lib/ipc/bindings/McpRefusal';
import type { McpState } from '$lib/ipc/bindings/McpState';
import type { SkillsUnavailableCode } from '$lib/ipc/bindings/SkillsUnavailableCode';
import type { OpenCodeGeneration } from '$lib/ipc/harness-ids';
import { harness as harnesses } from '$lib/stores/harness.svelte';

/** What one item of the step is doing: `idle` until a run reaches it. */
export type ItemPhase = 'idle' | 'running' | 'done' | 'failed' | 'skipped';

/** The companions the step installs from their own installers. */
export type Companion = 'dashboard' | 'memory';

/** The three MCP servers the step registers per harness: the dashboard's (`mcp`), the memory server's (`memoryMcp`) and the app's own, which lets agents manage hooks (`desktopMcp`). */
export type McpKind = 'mcp' | 'memoryMcp' | 'desktopMcp';

/** The things the step can set up: the two companions, and per harness its skills and its two MCP entries. */
export type SetupKey = Companion | `skills:${HarnessId}` | `${McpKind}:${HarnessId}`;

/** The companion each MCP entry runs, which has to be installed before the entry can be added. The app's own entry runs the app itself, so it waits for nothing. */
export const MCP_COMPANION: Record<McpKind, Companion | null> = { mcp: 'dashboard', memoryMcp: 'memory', desktopMcp: null };

/** The Harness Skills' installer script, one run per harness it installs for. */
export type SkillsInstaller = 'opencodeSkills' | 'codexSkills';

/** Everything the step runs an installer script for: the two companions, and the Harness Skills of OpenCode and Codex. */
export type Installer = Companion | SkillsInstaller;

/** The installer script that gives each harness its skills, for the harnesses whose skills come from one (`skills.type` is `script`). */
const SKILLS_INSTALLERS: Record<HarnessId, SkillsInstaller | null> = { omp: null, pi: null, claude: null, opencode: 'opencodeSkills', codex: 'codexSkills' };

/** The harnesses the skills installer script runs for, in the order their output is listed. */
export const SKILLS_INSTALLER_HARNESSES: HarnessId[] = ['opencode', 'codex'];

export function skillsInstallerOf(harness: HarnessId): SkillsInstaller | null {
  return SKILLS_INSTALLERS[harness];
}

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
  owner: Installer;
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

const MCP_TITLES: Record<McpKind, MessageKey> = { mcp: 'onboarding.setup.mcp.title', memoryMcp: 'onboarding.setup.memoryMcp.title', desktopMcp: 'onboarding.setup.desktopMcp.title' };

/** The column title of each MCP entry. */
export function mcpTitle(kind: McpKind): string {
  return t(MCP_TITLES[kind]);
}

/** The server name and command of each MCP entry. */
const MCP_SERVER: Record<McpKind, string> = { mcp: 'harness-dashboard', memoryMcp: 'harness-memory', desktopMcp: 'harness-desktop' };

/**
 * The entry a person adds to an MCP file when the app leaves it alone (a `[mcp_servers.<name>]` table for Codex, JSON for the rest), one key per line so it never wraps mid-token.
 * OpenCode 1 keeps its servers under "mcp" with `enabled`. OpenCode 2 keeps them under "mcp.servers" and has no `enabled`.
 * Either way the command is one list.
 */
export function mcpSnippet(kind: McpKind, harness?: HarnessId, generation: OpenCodeGeneration = 1): string {
  const server = MCP_SERVER[kind];

  if (harness === 'opencode' && generation === 2) {
    return `{
  "mcp": {
    "servers": {
      "${server}": {
        "type": "local",
        "command": ["${server}", "mcp"]
      }
    }
  }
}`;
  }

  if (harness === 'opencode') {
    return `{
  "mcp": {
    "${server}": {
      "type": "local",
      "command": ["${server}", "mcp"],
      "enabled": true
    }
  }
}`;
  }

  if (harness === 'codex') {
    return `[mcp_servers.${server}]
command = "${server}"
args = ["mcp"]`;
  }

  return `{
  "mcpServers": {
    "${server}": {
      "command": "${server}",
      "args": ["mcp"]
    }
  }
}`;
}

/**
 * The `claude` line an action runs, exactly as `business/src/harnesses/claude/cli.rs` builds it: always with `--json`,
 * and `--scope user` wherever the CLI takes one. An action Claude Code cannot run (a package) has no line.
 */
function claudeCommand(action: ExtensionAction): string | null {
  switch (action.type) {
    case 'pluginInstall':
      return `claude plugin install ${action.target} --scope ${action.scope} --json`;
    case 'pluginUninstall':
      return `claude plugin uninstall ${action.name} --scope user --json`;
    case 'pluginEnable':
      return `claude plugin enable ${action.name} --scope user --json`;
    case 'pluginDisable':
      return `claude plugin disable ${action.name} --scope user --json`;
    case 'pluginUpgrade':
      return action.name ? `claude plugin update ${action.name} --scope user --json` : null;
    case 'marketplaceAdd':
      return `claude plugin marketplace add ${action.source} --scope user --json`;
    case 'marketplaceRemove':
      return `claude plugin marketplace remove ${action.name} --scope user --json`;
    case 'marketplaceUpdate':
      return action.name ? `claude plugin marketplace update ${action.name} --json` : 'claude plugin marketplace update';
    default:
      return null;
  }
}

/**
 * The `codex` line an action runs, exactly as `business/src/harnesses/codex/cli.rs` builds it: plugins by
 * `name@marketplace` for the user, marketplaces upgraded rather than updated. An action Codex cannot run (switching or
 * updating a plugin, a package) has no line.
 */
function codexCommand(action: ExtensionAction): string | null {
  switch (action.type) {
    case 'pluginInstall':
      return `codex plugin add ${action.target}`;
    case 'pluginUninstall':
      return `codex plugin remove ${action.name}`;
    case 'marketplaceAdd':
      return `codex plugin marketplace add ${action.source}`;
    case 'marketplaceRemove':
      return `codex plugin marketplace remove ${action.name}`;
    case 'marketplaceUpdate':
      return action.name ? `codex plugin marketplace upgrade ${action.name}` : 'codex plugin marketplace upgrade';
    default:
      return null;
  }
}

/** The line of a harness whose CLI the app words its own way, or null for one that takes the generic form. */
function ownCommand(harness: HarnessId, action: ExtensionAction): string | null {
  switch (harness) {
    case 'claude':
      return claudeCommand(action);
    case 'codex':
      return codexCommand(action);
    case 'omp':
    case 'pi':
    case 'opencode':
      return null;
  }
}

/** The harness CLI line an action stands for, as shown in the confirmation. */
export function commandOf(harness: HarnessId, action: ExtensionAction): string {
  const own = ownCommand(harness, action);

  if (own) {
    return own;
  }

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
  cliFailed: 'onboarding.setup.refused.cliFailed',
  cliRemovedEntry: 'onboarding.setup.refused.cliRemovedEntry',
  cliBatchShim: 'onboarding.setup.refused.cliBatchShim',
};

/** Codex keeps its servers in a TOML file, so two refusals read differently for it. */
const CODEX_REFUSAL_KEYS: Partial<Record<McpRefusal, MessageKey>> = {
  unparsable: 'onboarding.setup.refused.unparsableToml',
  serversNotAnObject: 'onboarding.setup.refused.serversNotATable',
};

/** True for a refusal of a file only the harness's CLI changes, where an entry to paste would be wrong advice. */
export function isCliRefusal(reason: McpRefusal): boolean {
  return reason === 'cliFailed' || reason === 'cliRemovedEntry' || reason === 'cliBatchShim';
}

/** Why the app leaves an MCP file of `harness` alone, in a sentence; a failed CLI run names that harness's `mcp` command, a batch shim the harness itself. */
export function refusalText(reason: McpRefusal, harness: HarnessId): string {
  const key = (harness === 'codex' ? CODEX_REFUSAL_KEYS[reason] : undefined) ?? REFUSAL_KEYS[reason];

  return t(key, { command: `${harness} mcp`, harness: harnesses.get(harness)?.info.name ?? harness });
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
      case 'script':
        // An install too old for the harness is updated by the same installer.
        return { tone: 'warning', label: skills.outdatedFrom === null ? t('onboarding.setup.chip.missing') : t('onboarding.setup.chip.outdated') };
      case 'manual':
        return { tone: 'neutral', label: t('onboarding.setup.chip.manual') };
      case 'unavailable':
        return { tone: 'neutral', label: t('onboarding.setup.chip.unavailable') };
    }
  });
}

/** The chip of one of the harness's MCP entries (`mcp` the dashboard's, `memoryMcp` the memory server's, `desktopMcp` the app's own). */
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
      case 'manualEdit':
        return { tone: 'neutral', label: t('onboarding.setup.chip.manual') };
    }
  });
}

/** Harness Skills across every harness found: installed in all of them, in some, or in none yet. */
export function skillsOverviewChip(harnesses: HarnessSetup[]): Chip {
  const installed = harnesses.filter((entry) => entry.installed);
  // A harness the skills cannot be installed in at all (OpenCode without Node.js) does not hold the overview back. Its own row says why.
  const found = installed.filter((entry) => entry.skills.type !== 'unavailable');
  const have = found.filter((entry) => entry.skills.type === 'installed').length;

  if (found.length === 0) {
    return installed.length > 0 ? { tone: 'neutral', label: t('onboarding.setup.chip.unavailable') } : { tone: 'neutral', label: t('onboarding.setup.chip.notInstalled') };
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
  return harnesses.filter((entry) => entry.installed && (entry.skills.type === 'install' || entry.skills.type === 'script' || entry.skills.type === 'installed'));
}

/** Why the skills cannot be installed: the backend's code in the app's language, or else its English reason. */
export function unavailableReason(skills: { reason: string; code: SkillsUnavailableCode | null }): string {
  switch (skills.code) {
    case 'nodeMissing':
      return t('onboarding.setup.skills.unavailable.nodeMissing');
    default:
      return skills.reason;
  }
}

/** Found harnesses the skills cannot be installed in at all, each with the reason why. */
export function skillsUnavailable(harnesses: HarnessSetup[]): { harness: HarnessId; reason: string }[] {
  return harnesses.flatMap((entry) => (entry.installed && entry.skills.type === 'unavailable' ? [{ harness: entry.harness, reason: unavailableReason(entry.skills) }] : []));
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
