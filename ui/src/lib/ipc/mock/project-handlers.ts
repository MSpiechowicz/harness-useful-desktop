import { IpcFailure } from '../transport';
import { scopeOf } from './project-scopes';
import type { MockState } from './state';
import type { Handlers } from './types';

type ProjectCommands = 'project_pick' | 'project_open' | 'project_close' | 'project_recent' | 'project_forget' | 'project_scopes';

/** The project commands of the mock backend. `memoryUp` says whether Harness Memory answers, which `project_scopes` needs for the keys. */
export function projectHandlers(state: MockState, memoryUp: () => boolean = () => true): Pick<Handlers, ProjectCommands> {
  return {
    project_pick: () => {
      const picked = state.pickFolder();

      return picked ? state.activate(picked) : null;
    },

    project_open: ({ path }) => {
      state.requireKnown(path);

      const project = state.recents.find((recent) => recent.path === path) ?? state.active;
      if (!project || !project.exists) {
        throw IpcFailure.from({ code: 'notFound', message: 'The project folder no longer exists.' });
      }

      return state.activate(project);
    },

    project_close: () => {
      state.active = null;

      return null;
    },

    project_recent: () => structuredClone(state.recents),

    project_forget: ({ path }) => {
      state.recents = state.recents.filter((project) => project.path !== path);

      if (state.active?.path === path) {
        state.active = null;
      }

      return structuredClone(state.recents);
    },

    project_scopes: () => {
      const recents = state.recents.filter((project) => project.path !== state.active?.path);
      const projects = state.active ? [state.active, ...recents] : recents;

      return projects.map((project) => scopeOf(project, memoryUp()));
    },
  };
}
