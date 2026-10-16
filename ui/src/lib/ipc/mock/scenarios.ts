import { motion } from '$lib/stores/motion.svelte';

import { agentHandlers } from './agent-handlers';
import { ecosystemHandlers } from './ecosystem-handlers';
import { extensionActionHandlers } from './extension-action-handlers';
import { gitHandlers } from './git-handlers';
import { harnessHandlers } from './harness-handlers';
import { hostHandlers } from './host-handlers';
import { projectHandlers } from './project-handlers';
import { releaseHandlers } from './release-handlers';
import { sessionHandlers } from './session-handlers';
import { setupHandlers, setupScenarioFromLocation } from './setup-handlers';
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

  const base = {
    ...hostHandlers(state),
    ...harnessHandlers(state),
    ...projectHandlers(state),
    ...agentHandlers(state, agentPace),
    ...sessionHandlers(state),
    ...gitHandlers(state),
  };

  const actions = extensionActionHandlers(state, base.extensions_view);

  // The preview's `?setup=many` pretends the app knows more harnesses than it does, so the setup step can be seen with them.
  const harnesses = async () => {
    const list = await base.harness_list(undefined);
    const roster = rosterFor(setupScenarioFromLocation());

    return roster && list[0] ? rosterSummaries(roster, list[0]) : list;
  };
  const setup = setupHandlers(harnesses, setupScenarioFromLocation, () => state.setMemoryEnabled(true));
  const ecosystem = ecosystemHandlers();

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

      return status;
    },
    ...releaseHandlers(state),
    ...updateHandlers(agentPace),
    ...actions,
    ...setup.handlers,
    ...syncHandlers(),
    // The switch reports the memory server the setup mock installs.
    memory_set_enabled: ({ enabled }) => ({ memoryEnabled: state.setMemoryEnabled(enabled).memoryEnabled, server: setup.memoryServer() }),
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
