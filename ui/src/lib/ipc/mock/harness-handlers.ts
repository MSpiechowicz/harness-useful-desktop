import type { ConfigView } from '../bindings/ConfigView';
import type { ExtensionsView } from '../bindings/ExtensionsView';
import type { HarnessId } from '../bindings/HarnessId';
import type { HarnessSummary } from '../bindings/HarnessSummary';
import { IpcFailure } from '../transport';
import { fixtures } from './fixtures';
import type { MockState } from './state';
import { withOpenCodeChat } from './opencode-chat';
import { MockOmpRoles } from './omp-roles';
import { isNewer } from './release-handlers';
import { MockSkillsRoles } from './skills-roles';
import type { Handlers } from './types';

type HarnessCommands =
  | 'harness_list'
  | 'harness_detect'
  | 'harness_pick_binary'
  | 'harness_clear_binary'
  | 'harness_releases'
  | 'harness_update'
  | 'config_view'
  | 'omp_map_role'
  | 'omp_set_role'
  | 'skills_set_role'
  | 'extensions_view'
  | 'commands_catalog';

/** Version the fake "Choose binary…" dialog reports: a build newer than the tested range. */
const PICKED_VERSION = '99.0.0';

/** The newest version each harness CLI has in the mock: ahead of the installed ones in `fixtures/harness_list.json`. */
const MOCK_LATEST_VERSIONS: Record<HarnessId, string> = {
  omp: '18.7.0',
  pi: '1.5.0',
  opencode: '1.20.0',
  claude: '3.0.0',
  codex: '1.0.0',
};

function summaryOf(summaries: HarnessSummary[], id: HarnessId): HarnessSummary {
  const found = summaries.find((summary) => summary.info.id === id);
  if (!found) {
    throw IpcFailure.from({ code: 'notFound', message: `Unknown harness ${id}.` });
  }

  return found;
}

/** Without an open project the project layer is absent, like the real adapters. */
function withoutProjectLayer(view: ConfigView): ConfigView {
  return {
    ...view,
    sections: view.sections.filter((section) => section.source.layer !== 'project'),
    notices: view.notices.filter((notice) => !/project/i.test(notice.message)),
  };
}

function withoutProjectScope(view: ExtensionsView): ExtensionsView {
  const user = <T extends { scope: string }>(items: T[]) => items.filter((item) => item.scope !== 'project');

  return {
    ...view,
    plugins: user(view.plugins),
    skills: user(view.skills),
    promptTemplates: user(view.promptTemplates),
    agents: user(view.agents),
    themes: user(view.themes),
    packages: user(view.packages),
    extensionModules: user(view.extensionModules),
    mcpServers: view.mcpServers.filter((server) => server.source.layer !== 'project'),
    notices: view.notices.filter((notice) => !/project/i.test(notice.message)),
  };
}

export function harnessHandlers(state: MockState): Pick<Handlers, HarnessCommands> {
  const pristine = structuredClone(fixtures.harnesses.map(withOpenCodeChat));
  const summaries = structuredClone(pristine);
  const ompRoles = new MockOmpRoles();
  const skillsRoles = new MockSkillsRoles();

  function replace(id: HarnessId, next: HarnessSummary): HarnessSummary {
    summaries[summaries.findIndex((summary) => summary.info.id === id)] = next;

    return structuredClone(next);
  }

  return {
    harness_list: () => structuredClone(summaries),

    harness_detect: ({ harness }) => structuredClone(summaryOf(summaries, harness)),

    harness_pick_binary: ({ harness }) => {
      const current = summaryOf(summaries, harness);
      const path = `/home/demo/.local/bin/${current.info.binaryName}`;

      state.overrides = [...state.overrides.filter((override) => override.harness !== harness), { harness, path }];

      return replace(harness, {
        ...current,
        detection: {
          ...current.detection,
          installed: true,
          binaryPath: path,
          version: PICKED_VERSION,
          versionSupport: 'untested',
          installHint: null,
        },
      });
    },

    harness_clear_binary: ({ harness }) => {
      state.overrides = state.overrides.filter((override) => override.harness !== harness);

      return replace(harness, structuredClone(summaryOf(pristine, harness)));
    },

    // Like the backend: nothing while the lookup is off, else every installed harness compared with its latest version.
    harness_releases: ({ installed }) => {
      return state.prefs.checkHarnessUpdates
        ? installed.map(({ harness, version }) => {
            const latestVersion = MOCK_LATEST_VERSIONS[harness];

            return {
              harness,
              latestVersion,
              updateAvailable: isNewer(latestVersion, version),
              updateCommand: `${summaryOf(summaries, harness).info.binaryName} update`,
            };
          })
        : [];
    },

    harness_update: ({ harness }) => {
      const current = summaryOf(summaries, harness);
      const command = `${current.info.binaryName} update`;

      if (!current.detection.installed) {
        throw IpcFailure.from({ code: 'notFound', message: `${current.info.name} is not installed.` });
      }

      const latest = MOCK_LATEST_VERSIONS[harness];

      if (!current.detection.version || !isNewer(latest, current.detection.version)) {
        return { updated: false, summary: structuredClone(current), message: 'Already up to date.', command };
      }

      const summary = replace(harness, { ...current, detection: { ...current.detection, version: latest } });

      return { updated: true, summary, message: null, command };
    },

    config_view: ({ harness, project }) => {
      state.requireKnown(project);
      const fixture = structuredClone(fixtures.config[harness]);
      const view = harness === 'omp' ? ompRoles.decorate(fixture) : skillsRoles.decorate(fixture);

      return project === null ? withoutProjectLayer(view) : view;
    },

    omp_map_role: ({ role, selector }) => ompRoles.map(role, selector),
    omp_set_role: ({ role, selector }) => ompRoles.change(role, selector),

    skills_set_role: ({ harness, scope, role, selector }) => skillsRoles.change(harness, scope, role, selector, fixtures.config[harness]),

    extensions_view: ({ harness, project }) => {
      state.requireKnown(project);
      const view = structuredClone(fixtures.extensions[harness]);

      return project === null ? withoutProjectScope(view) : view;
    },

    commands_catalog: ({ harness }) => structuredClone(fixtures.commands[harness]),
  };
}
