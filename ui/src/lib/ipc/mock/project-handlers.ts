import { IpcFailure } from '../transport';
import type { MockState } from './state';
import type { Handlers } from './types';

type ProjectCommands = 'project_pick' | 'project_open' | 'project_close' | 'project_recent' | 'project_forget';

export function projectHandlers(state: MockState): Pick<Handlers, ProjectCommands> {
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
  };
}
