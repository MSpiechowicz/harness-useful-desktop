import { motion } from '$lib/stores/motion.svelte';

import type { CommandArgs } from '../commands';
import { IpcFailure } from '../transport';

import { agentHandlers } from './agent-handlers';
import { ecosystemHandlers } from './ecosystem-handlers';
import { extensionActionHandlers } from './extension-action-handlers';
import { gitHandlers } from './git-handlers';
import { gitSyncHandlers } from './git-sync-handlers';
import { harnessHandlers } from './harness-handlers';
import { hooksHandlers } from './hooks-handlers';
import { hookScenarioFromLocation, MockHooks } from './hooks-state';
import { hostHandlers } from './host-handlers';
import { MockMemoryGuards } from './memory-guards-handlers';
import { memoryCodeHandlers } from './memory-code-handlers';
import { memoryEmbedderHandlers } from './memory-embedder-handlers';
import { memoryRemoteHandlers } from './memory-remote-handlers';
import { memoryNotesHandlers } from './memory-notes-handlers';
import type { CopyStores } from './memory-notes-team-handlers';
import { availabilityOf, memoryScriptsHandlers, scenarioAfterInstall, scriptScenarioFromLocation } from './memory-scripts-handlers';
import { projectHandlers } from './project-handlers';
import { routineHandlers } from './routine-handlers';
import { MockRoutines, routineScenarioFromLocation } from './routine-state';
import { MOCK_LATEST_RELEASES, releaseHandlers } from './release-handlers';
import { sessionHandlers } from './session-handlers';
import { manualRemovalsOf, setupHandlers, setupScenarioFromLocation } from './setup-handlers';
import { rosterFor, rosterSummaries } from './setup-roster';
import { MockState } from './state';
import { syncHandlers } from './sync-handlers';
import type { Handlers, Scenario } from './types';
import { updateHandlers } from './update-handlers';

export type { Handlers, Scenario } from './types';

/** Replay pace of the mock agent: instant without motion, quicker when reduced. */
function agentPace(): number {
  const levels = { full: 1, reduced: 0.5, off: 0 };

  return levels[motion.level];
}

/** In-memory backend: everything persists for the lifetime of the page only. */
export function defaultScenario(): Scenario {
  const state = new MockState();
  // `?hooks=empty|outdated|outdatedEdited|noDashboard|invalid|problem|failing` changes what the Hooks view finds, `?hooks=ok` (the default) lists the family hooks and two more.
  // `?hooks=team|teamTrusted|teamNoOrigin|teamProblem` shares hooks with a team: approval needed, trusted, a repository without origin, a refresh that failed.
  const hooks = new MockHooks(hookScenarioFromLocation, () => state.memoryEnabled);

  // `?routines=empty|invalid|problem|failing` changes what the Routines view finds, `?routines=ok` (the default) lists sample routines in every status and a long run history.
  // `?routines=team|teamTrusted|teamNoOrigin|teamProblem` shares routines with a team, as `?hooks=` does for hooks.
  const routines = new MockRoutines(routineScenarioFromLocation);

  const { attach, ...agentCommands } = agentHandlers(state, agentPace, (project) => hooks.sessionContext(project));
  // `?held=empty` starts without held saves; by default the first chat with a memory identity has three waiting for review.
  const guards = new MockMemoryGuards();
  let memoryInstalled = false;
  const memoryAvailability = () => availabilityOf(memoryInstalled ? scenarioAfterInstall(scriptScenarioFromLocation()) : scriptScenarioFromLocation());

  const base = {
    ...hostHandlers(state),
    ...harnessHandlers(state),
    ...projectHandlers(state, () => memoryAvailability().kind === 'available'),
    ...agentCommands,
    agent_start: async (args: CommandArgs<'agent_start'>) => {
      const info = await agentCommands.agent_start(args);
      guards.chatStarted(info.key, info.project, args.request.memoryChat);

      return info;
    },
    ...sessionHandlers(state, routines),
    ...gitHandlers(state),
    // `?sync=behind|dirty|clash|conflict|merge|resolved|onmain|unpublished|nomain|noremote|detached|offline|stash` changes what the branch popover finds, `?branches=1500` adds branches.
    ...gitSyncHandlers(state),
  };

  const actions = extensionActionHandlers(state, base.extensions_view);

  // The preview's `?setup=many` pretends the app knows more harnesses than it does, so the setup step can be seen with them.
  const harnesses = async () => {
    const list = await base.harness_list(undefined);
    const roster = rosterFor(setupScenarioFromLocation());

    return roster && list[0] ? rosterSummaries(roster, list) : list;
  };
  const setup = setupHandlers(harnesses, setupScenarioFromLocation, () => state.setMemoryEnabled(true));
  const ecosystem = ecosystemHandlers();
  const copyStores: CopyStores = {};

  const handlers: Handlers = {
    ...base,
    harness_list: harnesses,
    ...ecosystem,
    // The Ecosystem screen shows the memory server the setup step and Settings install, not the fixture's fixed one.
    ecosystem_status: async () => {
      const status = await ecosystem.ecosystem_status(undefined);
      const memory = status.items.find((item) => item.id === 'memory');
      const server = setup.memoryServer();

      if (memory && !server.installed) {
        Object.assign(memory, { installed: false, version: null, via: [] });
      }

      // The outdated Memory previews have Harness Memory installed: an old release (updated to 1.1.0 from the Notes or Scripts view) or a source build, which reports a pre-release version no release is ever newer than.
      const preview = scriptScenarioFromLocation();

      if (memory && preview === 'outdatedNoRelease') {
        Object.assign(memory, { installed: true, version: '1.0.1-dev' });
      } else if (memory && preview === 'outdated') {
        Object.assign(memory, { installed: true, version: memoryInstalled ? '1.1.0' : '1.0.0' });
      }

      return status;
    },
    // The outdated Memory preview has a newer release to update to, until it was updated.
    ...releaseHandlers(state, () => (scriptScenarioFromLocation() === 'outdated' && !memoryInstalled ? { ...MOCK_LATEST_RELEASES, memory: '1.1.0' } : MOCK_LATEST_RELEASES)),
    ...updateHandlers(agentPace),
    ...actions,
    ...setup.handlers,
    ...syncHandlers(),
    ...hooksHandlers(hooks),
    ...routineHandlers(routines, attach),
    // The switch reports the memory server the setup mock installs.
    memory_set_enabled: ({ enabled }) => ({
      memoryEnabled: state.setMemoryEnabled(enabled).memoryEnabled,
      server: setup.memoryServer(),
      manualRemovals: enabled ? [] : manualRemovalsOf(setupScenarioFromLocation()),
    }),
    // `?remote=postgres|libsql|problem` starts with a saved team database; `harness-memory check` passes for URLs containing "ok".
    ...memoryRemoteHandlers(),
    ...memoryEmbedderHandlers(state),
    // `?code=old|build|bulk|cancelled|stale`: a server without the code index, or a build that ends the named way.
    ...memoryCodeHandlers(),
    ...guards.handlers(),
    // Like the backend: refused while the chat's guards skip the memory-save hook, else the hook's text as the user keeps it sent to the agent.
    chat_memory_check_again: async ({ agentId }) => {
      if (guards.skipsMemorySave(agentId)) {
        throw IpcFailure.from({ code: 'invalidInput', message: 'saving to Harness Memory is off for this chat' });
      }

      const message = hooks.memorySaveFollowUp(guards.projectOf(agentId));
      await base.agent_send({ key: agentId, action: { type: 'prompt', message, images: [], streamingBehavior: null } });

      return null;
    },
    // `?scripts=empty|notInstalled|outdated|outdatedNoRelease|failed` changes what the Scripts view finds, `?scripts=ok` (the default) lists three samples.
    ...memoryScriptsHandlers(() => (memoryInstalled ? scenarioAfterInstall(scriptScenarioFromLocation()) : scriptScenarioFromLocation()), undefined, copyStores),
    // The notes follow the same server as the scripts. `?notes=empty` keeps none. The copy commands serve both, so they share the stores.
    ...memoryNotesHandlers(memoryAvailability, undefined, undefined, undefined, copyStores),
    // Installing or updating Harness Memory from the Notes or Scripts view fixes its `notInstalled` and `outdated` previews.
    setup_install_memory: async (args) => {
      const result = await setup.handlers.setup_install_memory(args);
      memoryInstalled ||= result.ok;

      // A fresh install brings back the family hooks that were missing.
      if (result.ok) {
        hooks.memberInstalled('harness-memory');
      }

      return result;
    },
    setup_install_dashboard: async (args) => {
      const result = await setup.handlers.setup_install_dashboard(args);

      if (result.ok) {
        hooks.memberInstalled('harness-dashboard');
      }

      return result;
    },
    // A completed action may be one of the setup step's, which the next `setup_status` then reports as done.
    extension_action: async (args) => {
      const result = await actions.extension_action(args);

      if (!args.dryRun && result.ok) {
        setup.actionApplied(args.harness, args.action);
      }

      return result;
    },
  };

  return { name: 'default', handlers };
}
