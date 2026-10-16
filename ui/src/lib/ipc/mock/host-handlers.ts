import type { AppInfo } from '../bindings/AppInfo';
import type { MockState } from './state';
import type { Handlers } from './types';

type HostCommands = 'app_info' | 'prefs_get' | 'prefs_set' | 'memory_set_enabled' | 'open_external' | 'reveal_path' | 'launch_sound' | 'drop_listen' | 'attachment_save';

const MOCK_INFO: AppInfo = {
  name: 'Harness Desktop',
  version: '0.1.0',
  os: 'linux',
  arch: 'x86_64',
  debug: true,
};

/** App facts, preferences, the launch sound and the "open something outside the window" commands (no-ops in a browser). */
export function hostHandlers(state: MockState): Pick<Handlers, HostCommands> {
  return {
    app_info: () => MOCK_INFO,
    prefs_get: () => state.snapshot(),
    prefs_set: ({ prefs }) => state.save(prefs),
    memory_set_enabled: ({ enabled }) => state.setMemoryEnabled(enabled),
    open_external: () => null,
    reveal_path: () => null,
    launch_sound: () => null,
    // The mock has no native window: files dropped in a browser arrive through HTML5 drag and drop.
    drop_listen: () => null,
    attachment_save: ({ name, data }) => ({ name, path: `/home/demo/.cache/attachments/mock/${name}`, bytes: atob(data).length }),
  };
}
