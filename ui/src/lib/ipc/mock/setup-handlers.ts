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
import { opencodeSnippet, rosterFor, type RosterEntry } from './setup-roster';
import type { Handlers } from './types';

type SetupCommands = 'setup_status' | 'setup_install_dashboard' | 'setup_install_memory' | 'setup_register_mcp' | 'setup_register_memory';

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
 */
export const SETUP_SCENARIOS = ['none', 'configured', 'fail', 'refused', 'both', 'bothRefused', 'memory', 'many', 'manyLong', 'opencodeOnly', 'manualEdit'] as const;
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

const MCP_FILE: Record<HarnessId, string> = { omp: '~/.omp/agent/mcp.json', pi: '~/.pi/agent/mcp.json', opencode: '~/.config/opencode/opencode.json' };
/** OpenCode's config when it has comments: the app never rewrites it. */
const OPENCODE_COMMENTED_FILE = '~/.config/opencode/opencode.jsonc';
const SKILLS_TARGET = 'harness-useful-skills@harness-useful-skills';

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

function skillsOffer(harness: HarnessId): SkillsSetup {
  if (harness === 'opencode') {
    return { type: 'unavailable', reason: 'Harness Skills has no OpenCode package yet.' };
  }

  if (harness === 'pi') {
    return { type: 'manual', command: 'pi install git:github.com/MSpiechowicz/harness-useful-skills', note: 'Installing the skills as a PI package has not been verified yet.' };
  }

  return {
    type: 'install',
    actions: [
      { type: 'marketplaceAdd', source: 'MSpiechowicz/harness-useful-skills' },
      { type: 'pluginInstall', target: SKILLS_TARGET, scope: 'user' },
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
      (['omp', 'pi'] as const).forEach((id) => {
        skills.add(id);
        mcp.add(id);
        memoryMcp.add(id);
      });
      mcp.add('opencode');
      memoryMcp.add('opencode');
    }

    if (scenario() === 'memory') {
      memoryInstalled = true;
      memoryMcp.add('omp');
    }
  }

  const refusesOmp = () => scenario() === 'refused' || scenario() === 'bothRefused';
  const commentedOpencode = () => scenario() === 'manualEdit';

  /** The dashboard's entry (`registered` holds `mcp`) or the memory server's (`memoryMcp`) in a harness's MCP file. */
  function mcpState(harness: HarnessId, registered: Set<HarnessId> = mcp): McpState {
    if (registered.has(harness)) {
      return { type: 'configured', file: MCP_FILE[harness], name: registered === mcp ? 'harness-dashboard' : 'harness-memory' };
    }

    if (refusesOmp() && harness === 'omp') {
      return { type: 'refused', file: MCP_FILE[harness], reason: 'unparsable' };
    }

    if (commentedOpencode() && harness === 'opencode') {
      return { type: 'manualEdit', file: OPENCODE_COMMENTED_FILE, snippet: opencodeSnippet(registered === mcp ? 'harness-dashboard' : 'harness-memory') };
    }

    return { type: 'missing', file: MCP_FILE[harness], exists: harness !== 'pi' };
  }

  /** A harness of the `many` roster: its own starting state, moved by what a run did to it. */
  function rosterSetup(entry: RosterEntry): HarnessSetup {
    const id = entry.id;
    const registered = mcp.has(id) && entry.mcp.type === 'missing';
    const memoryRegistered = memoryMcp.has(id) && entry.memoryMcp.type === 'missing';

    return {
      harness: id,
      installed: entry.installed,
      skills: skills.has(id) ? { type: 'installed', version: '0.2.36' } : entry.skills,
      mcp: registered ? { type: 'configured', file: entry.mcp.file, name: 'harness-dashboard' } : entry.mcp,
      memoryMcp: memoryRegistered ? { type: 'configured', file: entry.memoryMcp.file, name: 'harness-memory' } : entry.memoryMcp,
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
      skills: skills.has(id) ? { type: 'installed', version: '0.2.36' } : skillsOffer(id),
      mcp: mcpState(id),
      memoryMcp: mcpState(id, memoryMcp),
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
    },

    memoryServer,

    actionApplied: (harness, action) => {
      if (action.type === 'pluginInstall' && action.target === SKILLS_TARGET) {
        skills.add(harness);
      }
    },
  };
}
