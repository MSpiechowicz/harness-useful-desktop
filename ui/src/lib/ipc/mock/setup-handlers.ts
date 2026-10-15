import type { DashboardInstallResult } from '../bindings/DashboardInstallResult';
import type { ExtensionAction } from '../bindings/ExtensionAction';
import type { HarnessId } from '../bindings/HarnessId';
import type { HarnessSetup } from '../bindings/HarnessSetup';
import type { HarnessSummary } from '../bindings/HarnessSummary';
import type { InstallLogLine } from '../bindings/InstallLogLine';
import type { McpState } from '../bindings/McpState';
import type { RecommendedSetup } from '../bindings/RecommendedSetup';
import type { SkillsSetup } from '../bindings/SkillsSetup';
import { IpcFailure } from '../transport';
import { rosterFor, type RosterEntry } from './setup-roster';
import type { Handlers } from './types';

type SetupCommands = 'setup_status' | 'setup_install_dashboard' | 'setup_register_mcp';

/**
 * What the mock recommended setup starts from. The browser preview picks one with `?setup=<name>`:
 * `none` (the default) is a machine with nothing set up, `configured` has everything in place,
 * `fail` makes the dashboard installer fail, and `refused` leaves OMP's MCP file unreadable.
 * `both` has PI installed next to OMP (its skills are a manual step), `bothRefused` adds the unreadable file to that.
 * `many` has five harnesses in every kind of state (see `setup-roster.ts`), `manyLong` a sixth with a very long name.
 */
export const SETUP_SCENARIOS = ['none', 'configured', 'fail', 'refused', 'both', 'bothRefused', 'many', 'manyLong'] as const;
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

const MCP_FILE: Record<HarnessId, string> = { omp: '~/.omp/agent/mcp.json', pi: '~/.pi/agent/mcp.json' };
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

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function skillsOffer(harness: HarnessId): SkillsSetup {
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

export interface SetupMock {
  handlers: Pick<Handlers, SetupCommands>;
  /** Tells the mock that an extension action went through, so the skills it installs show as installed. */
  actionApplied: (harness: HarnessId, action: ExtensionAction) => void;
}

/**
 * In-memory recommended setup. Installing the dashboard, running the skills' actions and registering the
 * MCP entry move the state the next `setup_status` reports, like the real backend's re-read.
 */
export function setupHandlers(harnesses: () => Promise<HarnessSummary[]> | HarnessSummary[], scenario: () => SetupScenario = setupScenarioFromLocation): SetupMock {
  let dashboardInstalled = false;
  const skills = new Set<HarnessId>();
  const mcp = new Set<HarnessId>();

  /** The `configured` scenario starts with everything in place; read on first use so a page URL set before it counts. */
  let started = false;
  function start(): void {
    if (started) {
      return;
    }

    started = true;

    if (scenario() === 'configured') {
      dashboardInstalled = true;
      (['omp', 'pi'] as const).forEach((id) => {
        skills.add(id);
        mcp.add(id);
      });
    }
  }

  const refusesOmp = () => scenario() === 'refused' || scenario() === 'bothRefused';

  function mcpState(harness: HarnessId): McpState {
    if (mcp.has(harness)) {
      return { type: 'configured', file: MCP_FILE[harness], name: 'harness-dashboard' };
    }

    if (refusesOmp() && harness === 'omp') {
      return { type: 'refused', file: MCP_FILE[harness], reason: 'unparsable' };
    }

    return { type: 'missing', file: MCP_FILE[harness], exists: harness === 'omp' };
  }

  /** A harness of the `many` roster: its own starting state, moved by what a run did to it. */
  function rosterSetup(entry: RosterEntry): HarnessSetup {
    const id = entry.id;
    const registered = mcp.has(id) && entry.mcp.type === 'missing';

    return {
      harness: id,
      installed: entry.installed,
      skills: skills.has(id) ? { type: 'installed', version: '0.2.36' } : entry.skills,
      mcp: registered ? { type: 'configured', file: entry.mcp.file, name: 'harness-dashboard' } : entry.mcp,
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
      installed: summary.detection.installed || (id === 'pi' && (scenario() === 'both' || scenario() === 'bothRefused')),
      skills: skills.has(id) ? { type: 'installed', version: '0.2.36' } : skillsOffer(id),
      mcp: mcpState(id),
    };
  }

  return {
    handlers: {
      setup_status: async (): Promise<RecommendedSetup> => {
        start();

        return {
          dashboard: { installed: dashboardInstalled, version: dashboardInstalled ? VERSION : null, supportedOnThisOs: true, command: COMMAND, sideEffects: SIDE_EFFECTS },
          harnesses: (await harnesses()).map(harnessSetup),
        };
      },

      setup_install_dashboard: async ({ confirmed, onLog }): Promise<DashboardInstallResult> => {
        start();

        if (!confirmed) {
          throw IpcFailure.from({ code: 'forbidden', message: 'The dashboard installer runs only after the user confirmed it.' });
        }

        const fails = scenario() === 'fail';

        for (const line of fails ? OUTPUT_FAILED : OUTPUT_OK) {
          await delay(LINE_MS);
          onLog(line);
        }

        dashboardInstalled ||= !fails;

        return {
          ok: !fails,
          summary: fails ? 'The installer exited with code 22.' : `Installed harness-dashboard ${VERSION}.`,
          exitCode: fails ? 22 : 0,
          timedOut: false,
          truncated: false,
          version: fails ? null : VERSION,
          command: COMMAND,
        };
      },

      setup_register_mcp: async ({ harness }): Promise<McpState> => {
        start();
        await delay(LINE_MS);

        const entry = rosterFor(scenario())?.find((candidate) => candidate.id === harness);

        if (entry) {
          mcp.add(harness);

          return rosterSetup(entry).mcp;
        }

        if (!refusesOmp() || harness !== 'omp') {
          mcp.add(harness);
        }

        return mcpState(harness);
      },
    },

    actionApplied: (harness, action) => {
      if (action.type === 'pluginInstall' && action.target === SKILLS_TARGET) {
        skills.add(harness);
      }
    },
  };
}
