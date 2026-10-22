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
import { IpcFailure } from '../transport';
import { OPENCODE_SKILLS, OPENCODE_SKILLS_COMMAND, OPENCODE_SKILLS_NO_NODE, opencodeSnippet, rosterFor, type RosterEntry } from './setup-roster';
import type { Handlers } from './types';

type SetupCommands = 'setup_status' | 'setup_install_dashboard' | 'setup_install_memory' | 'setup_install_opencode_skills' | 'setup_register_mcp' | 'setup_register_memory' | 'setup_register_desktop_mcp';

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
 * `noNode` finds no Node.js, so the Harness Skills cannot be installed for OpenCode and its row says why.
 */
export const SETUP_SCENARIOS = ['none', 'configured', 'fail', 'refused', 'both', 'bothRefused', 'memory', 'many', 'manyLong', 'opencodeOnly', 'manualEdit', 'noNode'] as const;
export type SetupScenario = (typeof SETUP_SCENARIOS)[number];

/** Pause between two installer log lines; tests do not wait. */
const LINE_MS = import.meta.env.MODE === 'test' ? 0 : 160;

const VERSION = '0.9.4';
const COMMAND = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-dashboard/main/install.sh | sh";
const SIDE_EFFECTS = [
  'Installs the Harness Dashboard app to ~/.local/lib/harness-dashboard and the harness-dashboard command to ~/.local/bin.',
  'Adds Harness Dashboard to your applications menu.',
  'Appends a line that puts ~/.local/bin on your PATH to your shell profile, if it is not on PATH yet.',
  'Stops a running dashboard server or app so the new version can start; `harness-dashboard mcp` keeps running.',
];

const MEMORY_VERSION = '1.0.0';
const MEMORY_COMMAND = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-memory/main/install.sh | sh";
const MEMORY_SIDE_EFFECTS = [
  'Installs the harness-memory command to ~/.local/bin, replacing an older one there.',
  'Appends a line that puts ~/.local/bin on your PATH to your shell profile, if it is not on PATH yet.',
  'Running memory servers keep working; each harness starts the new version next time.',
];

const MEMORY_OUTPUT_OK: InstallLogLine[] = [
  { stream: 'stdout', line: '==> Downloading harness-memory-x86_64-unknown-linux-gnu (latest)' },
  { stream: 'stdout', line: '==> Checksum verified' },
  { stream: 'stdout', line: '==> Installed harness-memory to ~/.local/bin/harness-memory' },
  { stream: 'stdout', line: `==> harness-memory ${MEMORY_VERSION}` },
];

const MEMORY_OUTPUT_FAILED: InstallLogLine[] = [
  { stream: 'stdout', line: '==> Downloading harness-memory-x86_64-unknown-linux-gnu (latest)' },
  { stream: 'stderr', line: 'error: download failed' },
];

const OPENCODE_SKILLS_VERSION = '0.2.40';

const OPENCODE_SKILLS_OUTPUT_OK: InstallLogLine[] = [
  { stream: 'stdout', line: '==> Checking Node.js (22 or newer)' },
  { stream: 'stdout', line: '==> Installing the Harness Skills into ~/.config/opencode' },
  { stream: 'stdout', line: '==> Adding the useful_skills MCP server to opencode.json' },
  { stream: 'stdout', line: `==> Harness Skills ${OPENCODE_SKILLS_VERSION} installed. Restart OpenCode to load them.` },
];

const OPENCODE_SKILLS_OUTPUT_FAILED: InstallLogLine[] = [
  { stream: 'stdout', line: '==> Checking Node.js (22 or newer)' },
  { stream: 'stderr', line: 'error: could not download the Harness Skills package' },
];

/** Each harness's user MCP file, empty for a harness with none the app writes (Codex, for now). Claude Code's is only read, its CLI writes it. */
const MCP_FILE: Record<HarnessId, string> = { omp: '~/.omp/agent/mcp.json', pi: '~/.pi/agent/mcp.json', opencode: '~/.config/opencode/opencode.json', claude: '~/.claude.json', codex: '' };
/** OpenCode's config when it has comments: the app never rewrites it. */
const OPENCODE_COMMENTED_FILE = '~/.config/opencode/opencode.jsonc';
const SKILLS_TARGET = 'harness-useful-skills@harness-useful-skills';
/** The Harness Skills plugin of Claude Code (its marketplace names itself `useful-skills-local`). */
const CLAUDE_SKILLS_TARGET = 'useful-skills@useful-skills-local';

const OUTPUT_OK: InstallLogLine[] = [
  { stream: 'stdout', line: 'Downloading harness-dashboard 0.9.4 for linux-x86_64' },
  { stream: 'stdout', line: 'Verifying checksum… ok' },
  { stream: 'stdout', line: 'Installing to ~/.local/lib/harness-dashboard' },
  { stream: 'stdout', line: 'Linking ~/.local/bin/harness-dashboard' },
  { stream: 'stdout', line: 'Done. Run `harness-dashboard` to start.' },
];

const OUTPUT_FAILED: InstallLogLine[] = [
  { stream: 'stdout', line: 'Downloading harness-dashboard 0.9.4 for linux-x86_64' },
  { stream: 'stderr', line: 'curl: (22) The requested URL returned error: 404' },
  { stream: 'stderr', line: 'Could not download the release archive.' },
];

/** Reads the scenario from the page URL (browser preview only); unknown names fall back to the default. */
export function setupScenarioFromLocation(search: string = globalThis.location?.search ?? ''): SetupScenario {
  const name = new URLSearchParams(search).get('setup');

  return SETUP_SCENARIOS.find((scenario) => scenario === name) ?? 'none';
}

/**
 * What turning project memory off leaves for the user to remove: only a commented OpenCode config (the `manualEdit` and
 * `opencodeOnly` scenarios) holds an entry the app will not rewrite.
 */
export function manualRemovalsOf(scenario: SetupScenario): ManualRemoval[] {
  if (scenario !== 'manualEdit' && scenario !== 'opencodeOnly') {
    return [];
  }

  return [{ harness: 'opencode', file: OPENCODE_COMMENTED_FILE, name: 'harness-memory' }];
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function skillsOffer(harness: HarnessId, nodeFound: boolean): SkillsSetup {
  if (harness === 'codex') {
    return { type: 'unavailable', reason: 'Harness Skills for Codex are not available yet.', code: null };
  }

  if (harness === 'opencode') {
    return nodeFound ? OPENCODE_SKILLS : OPENCODE_SKILLS_NO_NODE;
  }

  if (harness === 'pi') {
    return { type: 'manual', command: 'pi install git:github.com/MSpiechowicz/harness-useful-skills', note: 'Installing the skills as a PI package has not been verified yet.' };
  }

  return {
    type: 'install',
    actions: [
      { type: 'marketplaceAdd', source: 'MSpiechowicz/harness-useful-skills' },
      { type: 'pluginInstall', target: harness === 'claude' ? CLAUDE_SKILLS_TARGET : SKILLS_TARGET, scope: 'user' },
    ],
  };
}

/** One companion installer of the mock: what it prints and reports. */
interface MockInstaller {
  binary: string;
  version: string;
  command: string;
  output: InstallLogLine[];
  failedOutput: InstallLogLine[];
  failedSummary: string;
}

const DASHBOARD_INSTALLER: MockInstaller = {
  binary: 'harness-dashboard',
  version: VERSION,
  command: COMMAND,
  output: OUTPUT_OK,
  failedOutput: OUTPUT_FAILED,
  failedSummary: 'The installer exited with code 22.',
};

const MEMORY_INSTALLER: MockInstaller = {
  binary: 'harness-memory',
  version: MEMORY_VERSION,
  command: MEMORY_COMMAND,
  output: MEMORY_OUTPUT_OK,
  failedOutput: MEMORY_OUTPUT_FAILED,
  failedSummary: 'error: download failed',
};

const OPENCODE_SKILLS_INSTALLER: MockInstaller = {
  binary: 'Harness Skills for OpenCode',
  version: OPENCODE_SKILLS_VERSION,
  command: OPENCODE_SKILLS_COMMAND,
  output: OPENCODE_SKILLS_OUTPUT_OK,
  failedOutput: OPENCODE_SKILLS_OUTPUT_FAILED,
  failedSummary: 'error: could not download the Harness Skills package',
};

/** Plays `installer` line by line; it fails when `fails`. */
async function runInstaller(installer: MockInstaller, fails: boolean, onLog: (line: InstallLogLine) => void): Promise<DashboardInstallResult> {
  for (const line of fails ? installer.failedOutput : installer.output) {
    await delay(LINE_MS);
    onLog(line);
  }

  return {
    ok: !fails,
    summary: fails ? installer.failedSummary : `Installed ${installer.binary} ${installer.version}.`,
    exitCode: fails ? 22 : 0,
    timedOut: false,
    truncated: false,
    version: fails ? null : installer.version,
    command: installer.command,
  };
}

export interface SetupMock {
  handlers: Pick<Handlers, SetupCommands>;
  /** The memory server as `setup_status` reports it (also the answer of `memory_set_enabled`). */
  memoryServer: () => DashboardSetup;
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
): SetupMock {
  let dashboardInstalled = false;
  let memoryInstalled = false;
  const skills = new Set<HarnessId>();
  const mcp = new Set<HarnessId>();
  const memoryMcp = new Set<HarnessId>();
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
      (['omp', 'pi', 'opencode'] as const).forEach((id) => {
        skills.add(id);
        mcp.add(id);
        memoryMcp.add(id);
        desktopMcp.add(id);
      });
      mcp.add('opencode');
      memoryMcp.add('opencode');
      desktopMcp.add('opencode');
    }

    if (scenario() === 'memory') {
      memoryInstalled = true;
      memoryMcp.add('omp');
    }
  }

  const refusesOmp = () => scenario() === 'refused' || scenario() === 'bothRefused';
  const commentedOpencode = () => scenario() === 'manualEdit';

  /** The name an MCP entry has in the file, by the set that holds the harnesses it is registered in. */
  const entryName = (registered: Set<HarnessId>) => (registered === mcp ? 'harness-dashboard' : registered === memoryMcp ? 'harness-memory' : 'harness-desktop');

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

  /** OpenCode's skills come from their own installer, which records its own version. */
  const installedSkills = (harness: HarnessId): SkillsSetup => ({ type: 'installed', version: harness === 'opencode' ? OPENCODE_SKILLS_VERSION : '0.2.36' });

  /** A harness of the `many` roster: its own starting state, moved by what a run did to it. */
  function rosterSetup(entry: RosterEntry): HarnessSetup {
    const id = entry.id;
    const registered = mcp.has(id) && entry.mcp.type === 'missing';
    const memoryRegistered = memoryMcp.has(id) && entry.memoryMcp.type === 'missing';
    const desktopRegistered = desktopMcp.has(id) && entry.desktopMcp.type === 'missing';

    return {
      harness: id,
      installed: entry.installed,
      skills: skills.has(id) ? installedSkills(id) : entry.skills,
      mcp: registered ? { type: 'configured', file: entry.mcp.file, name: 'harness-dashboard' } : entry.mcp,
      memoryMcp: memoryRegistered ? { type: 'configured', file: entry.memoryMcp.file, name: 'harness-memory' } : entry.memoryMcp,
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
      desktopMcp: mcpState(id, desktopMcp),
    };
  }

  function memoryServer(): DashboardSetup {
    start();

    return { installed: memoryInstalled, version: memoryInstalled ? MEMORY_VERSION : null, supportedOnThisOs: true, command: MEMORY_COMMAND, sideEffects: MEMORY_SIDE_EFFECTS };
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

      setup_install_opencode_skills: async ({ confirmed, onLog }): Promise<DashboardInstallResult> => {
        const result = await install(OPENCODE_SKILLS_INSTALLER, confirmed, onLog);

        if (result.ok) {
          skills.add('opencode');
        }

        return result;
      },

      setup_register_mcp: async ({ harness }): Promise<McpState> => {
        start();
        await delay(LINE_MS);

        const entry = rosterFor(scenario())?.find((candidate) => candidate.id === harness);

        if (entry) {
          mcp.add(harness);

          return rosterSetup(entry).mcp;
        }

        if ((!refusesOmp() || harness !== 'omp') && !(commentedOpencode() && harness === 'opencode')) {
          mcp.add(harness);
        }

        return mcpState(harness);
      },

      setup_register_memory: async ({ harness }): Promise<McpState> => {
        start();
        await delay(LINE_MS);

        if (!memoryInstalled) {
          throw IpcFailure.from({ code: 'invalidInput', message: 'Harness Memory is not installed' });
        }

        memoryOn();
        const entry = rosterFor(scenario())?.find((candidate) => candidate.id === harness);

        if (entry) {
          memoryMcp.add(harness);

          return rosterSetup(entry).memoryMcp;
        }

        if ((!refusesOmp() || harness !== 'omp') && !(commentedOpencode() && harness === 'opencode')) {
          memoryMcp.add(harness);
        }

        return mcpState(harness, memoryMcp);
      },

      // The app is the server, so nothing has to be installed first.
      setup_register_desktop_mcp: async ({ harness }): Promise<McpState> => {
        start();
        await delay(LINE_MS);

        const entry = rosterFor(scenario())?.find((candidate) => candidate.id === harness);

        if (entry) {
          desktopMcp.add(harness);

          return rosterSetup(entry).desktopMcp;
        }

        if ((!refusesOmp() || harness !== 'omp') && !(commentedOpencode() && harness === 'opencode')) {
          desktopMcp.add(harness);
        }

        return mcpState(harness, desktopMcp);
      },
    },

    memoryServer,

    actionApplied: (harness, action) => {
      if (action.type === 'pluginInstall' && (action.target === SKILLS_TARGET || action.target === CLAUDE_SKILLS_TARGET)) {
        skills.add(harness);
      }
    },
  };
}
