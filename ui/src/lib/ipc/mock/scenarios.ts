import { motion } from '$lib/stores/motion.svelte';

import { agentHandlers } from './agent-handlers';
import { ecosystemHandlers } from './ecosystem-handlers';
import { extensionActionHandlers } from './extension-action-handlers';
import { gitHandlers } from './git-handlers';
import { harnessHandlers } from './harness-handlers';
import { hostHandlers } from './host-handlers';
import { projectHandlers } from './project-handlers';
import { sessionHandlers } from './session-handlers';
import { MockState } from './state';
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

  const handlers: Handlers = {
    ...base,
    ...ecosystemHandlers(),
    ...updateHandlers(agentPace),
    ...extensionActionHandlers(state, base.extensions_view),
  };

  return { name: 'default', handlers };
}
