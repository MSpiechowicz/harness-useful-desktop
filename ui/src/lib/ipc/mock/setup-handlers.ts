import type { DashboardInstallResult } from '../bindings/DashboardInstallResult';
import type { DashboardSetup } from '../bindings/DashboardSetup';
import type { ExtensionAction } from '../bindings/ExtensionAction';
import type { HarnessId } from '../bindings/HarnessId';
import type { HarnessSetup } from '../bindings/HarnessSetup';
import type { HarnessSummary } from '../bindings/HarnessSummary';
import type { InstallLogLine } from '../bindings/InstallLogLine';
import type { ManualRemoval } from '../bindings/ManualRemoval';
import type { McpState } from '../bindings/McpState';
import type { RecommendedSetup } from '../bindings/RecommendedSetup';
import type { SkillsSetup } from '../bindings/SkillsSetup';
import { HARNESS_IDS } from '../harness-ids';
import { IpcFailure } from '../transport';
import {
  CODEX_SKILLS_INSTALLER,
  CODEX_SKILLS_VERSION,
  COMMAND,
  DASHBOARD_INSTALLER,
  delay,
  KNOWLEDGE_COMMAND,
  KNOWLEDGE_INSTALLER,
  KNOWLEDGE_SIDE_EFFECTS,
  KNOWLEDGE_VERSION,
  LINE_MS,
  MEMORY_COMMAND,
  MEMORY_INSTALLER,
  MEMORY_SIDE_EFFECTS,
  MEMORY_VERSION,
  OPENCODE_SKILLS_INSTALLER,
  OPENCODE_SKILLS_VERSION,
  runInstaller,
  SIDE_EFFECTS,
  VERSION,
  type MockInstaller,
} from './setup-installers';
import { CODEX_SKILLS, knowledgeMcpOf, OPENCODE_SKILLS, opencodeSnippet, rosterFor, SKILLS_NO_NODE, type RosterEntry } from './setup-roster';
import type { Handlers } from './types';

type SetupCommands = 'setup_status' | 'setup_install_dashboard' | 'setup_install_memory' | 'setup_install_knowledge' | 'setup_install_opencode_skills' | 'setup_install_codex_skills' | 'skills_migrate_names' | 'setup_register_mcp' | 'setup_register_memory' | 'setup_register_knowledge' | 'setup_register_desktop_mcp';

/**
 * What the mock recommended setup starts from. The browser preview picks one with `?setup=<name>`:
 * `none` (the default) is a machine with nothing set up, `configured` has everything in place,
 * `fail` makes the dashboard and memory installers fail, and `refused` leaves OMP's MCP file unreadable.
 * `memory` has PI installed next to OMP and the memory server installed and registered in OMP only, so PI's memory entry is
 * the one left to add.
 * `both` has PI installed next to OMP (its skills are a manual step), `bothRefused` adds the unreadable file to that.
 * `many` has five harnesses in every kind of state (see `setup-roster.ts`), `manyLong` a sixth with a very long name.
 * `opencodeOnly` has OpenCode alone, with a config file that has comments, so its MCP entries are left to a manual edit.
 * `manualEdit` has the usual harnesses and the same commented OpenCode file.
 * `noNode` finds no Node.js, so the Harness Skills cannot be installed for OpenCode and Codex and their rows say why.
 */
export const SETUP_SCENARIOS = ['none', 'configured', 'fail', 'refused', 'both', 'bothRefused', 'memory', 'many', 'manyLong', 'opencodeOnly', 'manualEdit', 'noNode'] as const;
export type SetupScenario = (typeof SETUP_SCENARIOS)[number];

/** Each harness's user MCP file. Claude Code's and Codex's are only read, their CLIs write them. */
const MCP_FILE: Record<HarnessId, string> = {
  omp: '~/.omp/agent/mcp.json',
  pi: '~/.pi/agent/mcp.json',
  opencode: '~/.config/opencode/opencode.json',
  claude: '~/.claude.json',
  codex: '~/.codex/config.toml',
};
/** OpenCode's config when it has comments: the app never rewrites it. */
const OPENCODE_COMMENTED_FILE = '~/.config/opencode/opencode.jsonc';
const SKILLS_TARGET = 'harness-useful-skills@harness-useful-skills';
/** The Harness Skills plugin of Claude Code: since Harness Skills 0.4.0 its marketplace and plugin carry the OMP names. */
const CLAUDE_SKILLS_TARGET = SKILLS_TARGET;

/** Reads the scenario from the page URL (browser preview only); unknown names fall back to the default. */
export function setupScenarioFromLocation(search: string = globalThis.location?.search ?? ''): SetupScenario {
  const name = new URLSearchParams(search).get('setup');

  return SETUP_SCENARIOS.find((scenario) => scenario === name) ?? 'none';
}

/**
 * What turning project memory off leaves for the user to remove: only a commented OpenCode config (the `manualEdit` and
 * `opencodeOnly` scenarios) holds an entry the app will not rewrite.
 */
export function manualRemovalsOf(scenario: SetupScenario, name: string = 'harness-memory'): ManualRemoval[] {
  if (scenario !== 'manualEdit' && scenario !== 'opencodeOnly') {
    return [];
  }

  return [{ harness: 'opencode', file: OPENCODE_COMMENTED_FILE, name }];
}

/** The marketplace and plugin of the Harness Skills in a harness that installs them from a marketplace. */
function marketplaceSkills(target: string): SkillsSetup {
  return {
    type: 'install',
    actions: [
      { type: 'marketplaceAdd', source: 'MSpiechowicz/harness-useful-skills' },
      { type: 'pluginInstall', target, scope: 'user' },
    ],
  };
}

/** What `setup_status` offers for each harness's skills, as the backend's `companion_setup` decides it. */
function skillsOffer(harness: HarnessId, nodeFound: boolean): SkillsSetup {
  switch (harness) {
    case 'omp':
      return marketplaceSkills(SKILLS_TARGET);
    case 'claude':
      return marketplaceSkills(CLAUDE_SKILLS_TARGET);
    case 'pi':
      return { type: 'manual', command: 'pi install git:github.com/MSpiechowicz/harness-useful-skills', note: 'Installing the skills as a PI package has not been verified yet.' };
    case 'opencode':
      return nodeFound ? OPENCODE_SKILLS : SKILLS_NO_NODE;
    case 'codex':
      return nodeFound ? CODEX_SKILLS : SKILLS_NO_NODE;
  }
}

export interface SetupMock {
  handlers: Pick<Handlers, SetupCommands>;
  /** The memory server as `setup_status` reports it (also the answer of `memory_set_enabled`). */
  memoryServer: () => DashboardSetup;
  /** The knowledge server as `setup_status` reports it (also the answer of `knowledge_set_enabled`). */
  knowledgeServer: () => DashboardSetup;
  /** Tells the mock that an extension action went through, so the skills it installs show as installed. */
  actionApplied: (harness: HarnessId, action: ExtensionAction) => void;
}

/**
 * In-memory recommended setup. Installing the dashboard or the memory server, running the skills' actions and registering the
 * MCP entries move the state the next `setup_status` reports, like the real backend's re-read. Registering the memory server
 * turns Project memory on (`memoryOn`), as the real command does.
 */
export function setupHandlers(
  harnesses: () => Promise<HarnessSummary[]> | HarnessSummary[],
  scenario: () => SetupScenario = setupScenarioFromLocation,
  memoryOn: () => void = () => {},
  knowledgeOn: () => void = () => {},
): SetupMock {
  let dashboardInstalled = false;
  let memoryInstalled = false;
  let knowledgeInstalled = false;
  const skills = new Set<HarnessId>();
  const mcp = new Set<HarnessId>();
  const memoryMcp = new Set<HarnessId>();
  const knowledgeMcp = new Set<HarnessId>();
  const desktopMcp = new Set<HarnessId>();

  /** The `configured` scenario starts with everything in place; read on first use so a page URL set before it counts. */
  let started = false;
  function start(): void {
    if (started) {
      return;
    }

    started = true;

    if (scenario() === 'configured') {
      dashboardInstalled = true;
      memoryInstalled = true;
      knowledgeInstalled = true;

      HARNESS_IDS.forEach((id) => {
        skills.add(id);
        mcp.add(id);
        memoryMcp.add(id);
        knowledgeMcp.add(id);
        desktopMcp.add(id);
      });
    }

    if (scenario() === 'memory') {
      memoryInstalled = true;
      memoryMcp.add('omp');
    }
  }

  const refusesOmp = () => scenario() === 'refused' || scenario() === 'bothRefused';
  const commentedOpencode = () => scenario() === 'manualEdit';

  /** The name an MCP entry has in the file, by the set that holds the harnesses it is registered in. */
  const entryName = (registered: Set<HarnessId>) => (registered === mcp ? 'harness-dashboard' : registered === memoryMcp ? 'harness-memory' : registered === knowledgeMcp ? 'harness-knowledge' : 'harness-desktop');

  /** The dashboard's entry (`registered` holds `mcp`), the memory server's (`memoryMcp`) or the app's own (`desktopMcp`) in a harness's MCP file. */
  function mcpState(harness: HarnessId, registered: Set<HarnessId> = mcp): McpState {
    if (MCP_FILE[harness] === '') {
      return { type: 'refused', file: '', reason: 'unreadable' };
    }

    if (registered.has(harness)) {
      return { type: 'configured', file: MCP_FILE[harness], name: entryName(registered) };
    }

    if (refusesOmp() && harness === 'omp') {
      return { type: 'refused', file: MCP_FILE[harness], reason: 'unparsable' };
    }

    if (commentedOpencode() && harness === 'opencode') {
      return { type: 'manualEdit', file: OPENCODE_COMMENTED_FILE, snippet: opencodeSnippet(entryName(registered)) };
    }

    return { type: 'missing', file: MCP_FILE[harness], exists: harness !== 'pi' };
  }

  /** The skills of OpenCode and Codex come from their own installer, which records its own version. */
  const SCRIPT_VERSIONS: Partial<Record<HarnessId, string>> = { opencode: OPENCODE_SKILLS_VERSION, codex: CODEX_SKILLS_VERSION };
  const installedSkills = (harness: HarnessId): SkillsSetup => ({ type: 'installed', version: SCRIPT_VERSIONS[harness] ?? '0.2.36' });

  /** A harness of the `many` roster: its own starting state, moved by what a run did to it. */
  function rosterSetup(entry: RosterEntry): HarnessSetup {
    const id = entry.id;
    const registered = mcp.has(id) && entry.mcp.type === 'missing';
    const memoryRegistered = memoryMcp.has(id) && entry.memoryMcp.type === 'missing';
    const knowledgeState = knowledgeMcpOf(entry);
    const knowledgeRegistered = knowledgeMcp.has(id) && knowledgeState.type === 'missing';
    const desktopRegistered = desktopMcp.has(id) && entry.desktopMcp.type === 'missing';

    return {
      harness: id,
      installed: entry.installed,
      skills: skills.has(id) ? installedSkills(id) : entry.skills,
      mcp: registered ? { type: 'configured', file: entry.mcp.file, name: 'harness-dashboard' } : entry.mcp,
      memoryMcp: memoryRegistered ? { type: 'configured', file: entry.memoryMcp.file, name: 'harness-memory' } : entry.memoryMcp,
      knowledgeMcp: knowledgeRegistered ? { type: 'configured', file: knowledgeState.file, name: 'harness-knowledge' } : knowledgeState,
      desktopMcp: desktopRegistered ? { type: 'configured', file: entry.desktopMcp.file, name: 'harness-desktop' } : entry.desktopMcp,
    };
  }

  function harnessSetup(summary: HarnessSummary): HarnessSetup {
    const id = summary.info.id;
    const entry = rosterFor(scenario())?.find((candidate) => candidate.id === id);

    if (entry) {
      return rosterSetup(entry);
    }

    return {
      harness: id,
      installed: summary.detection.installed || (id === 'pi' && ['both', 'bothRefused', 'memory'].includes(scenario())),
      skills: skills.has(id) ? installedSkills(id) : skillsOffer(id, scenario() !== 'noNode'),
      mcp: mcpState(id),
      memoryMcp: mcpState(id, memoryMcp),
      knowledgeMcp: mcpState(id, knowledgeMcp),
      desktopMcp: mcpState(id, desktopMcp),
    };
  }

  /** Registers one server in a harness's MCP file and answers the entry's state after it, as the harness's file allows. */
  async function register(harness: HarnessId, registered: Set<HarnessId>, pick: (setup: HarnessSetup) => McpState): Promise<McpState> {
    start();
    await delay(LINE_MS);

    const entry = rosterFor(scenario())?.find((candidate) => candidate.id === harness);

    if (entry) {
      registered.add(harness);

      return pick(rosterSetup(entry));
    }

    if ((!refusesOmp() || harness !== 'omp') && !(commentedOpencode() && harness === 'opencode')) {
      registered.add(harness);
    }

    return mcpState(harness, registered);
  }

  function memoryServer(): DashboardSetup {
    start();

    return { installed: memoryInstalled, version: memoryInstalled ? MEMORY_VERSION : null, supportedOnThisOs: true, command: MEMORY_COMMAND, sideEffects: MEMORY_SIDE_EFFECTS };
  }

  function knowledgeServer(): DashboardSetup {
    start();

    return { installed: knowledgeInstalled, version: knowledgeInstalled ? KNOWLEDGE_VERSION : null, supportedOnThisOs: true, command: KNOWLEDGE_COMMAND, sideEffects: KNOWLEDGE_SIDE_EFFECTS };
  }

  /** Runs `installer` once the user confirmed it; `ok` in the result when it installed. */
  async function install(installer: MockInstaller, confirmed: boolean, onLog: (line: InstallLogLine) => void): Promise<DashboardInstallResult> {
    start();

    if (!confirmed) {
      throw IpcFailure.from({ code: 'forbidden', message: `The ${installer.binary} installer runs only after the user confirmed it.` });
    }

    return runInstaller(installer, scenario() === 'fail', onLog);
  }

  return {
    handlers: {
      setup_status: async (): Promise<RecommendedSetup> => {
        start();

        return {
          dashboard: { installed: dashboardInstalled, version: dashboardInstalled ? VERSION : null, supportedOnThisOs: true, command: COMMAND, sideEffects: SIDE_EFFECTS },
          memory: memoryServer(),
          knowledge: knowledgeServer(),
          harnesses: (await harnesses()).map(harnessSetup),
        };
      },

      setup_install_dashboard: async ({ confirmed, onLog }): Promise<DashboardInstallResult> => {
        const result = await install(DASHBOARD_INSTALLER, confirmed, onLog);
        dashboardInstalled ||= result.ok;

        return result;
      },

      setup_install_memory: async ({ confirmed, onLog }): Promise<DashboardInstallResult> => {
        const result = await install(MEMORY_INSTALLER, confirmed, onLog);
        memoryInstalled ||= result.ok;

        return result;
      },

      setup_install_knowledge: async ({ confirmed, onLog }): Promise<DashboardInstallResult> => {
        const result = await install(KNOWLEDGE_INSTALLER, confirmed, onLog);
        knowledgeInstalled ||= result.ok;

        return result;
      },

      setup_install_opencode_skills: async ({ confirmed, onLog }): Promise<DashboardInstallResult> => {
        const result = await install(OPENCODE_SKILLS_INSTALLER, confirmed, onLog);

        if (result.ok) {
          skills.add('opencode');
        }

        return result;
      },

      setup_install_codex_skills: async ({ confirmed, onLog }): Promise<DashboardInstallResult> => {
        const result = await install(CODEX_SKILLS_INSTALLER, confirmed, onLog);

        if (result.ok) {
          skills.add('codex');
        }

        return result;
      },

      // The browser preview has no install under the old names.
      skills_migrate_names: async () => ({ migrated: [], failed: [] }),

      setup_register_mcp: ({ harness }) => register(harness, mcp, (setup) => setup.mcp),

      setup_register_memory: async ({ harness }): Promise<McpState> => {
        if (!memoryInstalled) {
          start();
          throw IpcFailure.from({ code: 'invalidInput', message: 'Harness Memory is not installed' });
        }

        memoryOn();

        return register(harness, memoryMcp, (setup) => setup.memoryMcp);
      },

      setup_register_knowledge: async ({ harness }): Promise<McpState> => {
        if (!knowledgeInstalled) {
          start();
          throw IpcFailure.from({ code: 'invalidInput', message: 'Harness Knowledge is not installed' });
        }

        knowledgeOn();

        return register(harness, knowledgeMcp, (setup) => setup.knowledgeMcp);
      },

      // The app is the server, so nothing has to be installed first.
      setup_register_desktop_mcp: ({ harness }) => register(harness, desktopMcp, (setup) => setup.desktopMcp),
    },

    memoryServer,
    knowledgeServer,

    actionApplied: (harness, action) => {
      if (action.type === 'pluginInstall' && (action.target === SKILLS_TARGET || action.target === CLAUDE_SKILLS_TARGET)) {
        skills.add(harness);
      }
    },
  };
}
