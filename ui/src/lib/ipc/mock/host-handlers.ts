import type { AppInfo } from '../bindings/AppInfo';
import type { MockState } from './state';
import type { Handlers } from './types';

type HostCommands = 'app_info' | 'prefs_get' | 'prefs_set' | 'open_external' | 'reveal_path';

const MOCK_INFO: AppInfo = {
  name: 'Harness Desktop',
  version: '0.1.0',
  os: 'linux',
  arch: 'x86_64',
  debug: true,
};

/** App facts, preferences and the "open something outside the window" commands (no-ops in a browser). */
export function hostHandlers(state: MockState): Pick<Handlers, HostCommands> {
  return {
    app_info: () => MOCK_INFO,
    prefs_get: () => state.snapshot(),
    prefs_set: ({ prefs }) => state.save(prefs),
    open_external: () => null,
    reveal_path: () => null,
  };
}
