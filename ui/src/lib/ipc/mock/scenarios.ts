import { harnessHandlers } from './harness-handlers';
import { hostHandlers } from './host-handlers';
import { projectHandlers } from './project-handlers';
import { MockState } from './state';
import type { Handlers, Scenario } from './types';

export type { Handlers, Scenario } from './types';

/** In-memory backend: everything persists for the lifetime of the page only. */
export function defaultScenario(): Scenario {
  const state = new MockState();

  const handlers: Handlers = {
    ...hostHandlers(state),
    ...harnessHandlers(state),
    ...projectHandlers(state),
  };

  return { name: 'default', handlers };
}
