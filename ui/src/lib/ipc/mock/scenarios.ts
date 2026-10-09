import type { AppInfo } from '../bindings/AppInfo';
import type { AppPrefs } from '../bindings/AppPrefs';
import type { CommandArgs, CommandName, CommandResult } from '../commands';

type Handlers = {
  [K in CommandName]: (args: CommandArgs<K>) => CommandResult<K> | Promise<CommandResult<K>>;
};

/** A named, self-contained backend behaviour for browser previews and e2e runs. */
export interface Scenario {
  name: string;
  handlers: Handlers;
}

const MOCK_INFO: AppInfo = {
  name: 'Harness Desktop',
  version: '0.1.0',
  os: 'linux',
  arch: 'x86_64',
  debug: true,
};

function defaultPrefs(): AppPrefs {
  return {
    version: 1,
    theme: 'system',
    motion: 'full',
    introEnabled: true,
    defaultHarness: null,
    activeProject: null,
    rememberRecentProjects: true,
    recentProjects: [],
    updatePolicy: 'startupAsk',
    onboardingCompleted: false,
    approvalNoticeSeen: false,
    binaryOverrides: [],
  };
}

/** In-memory backend: preferences persist for the lifetime of the page only. */
export function defaultScenario(): Scenario {
  let prefs = defaultPrefs();

  return {
    name: 'default',
    handlers: {
      app_info: () => MOCK_INFO,
      prefs_get: () => prefs,
      prefs_set: ({ prefs: next }) => {
        prefs = next;

        return prefs;
      },
    },
  };
}
