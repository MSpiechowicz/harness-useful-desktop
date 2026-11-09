import type { AppInfo } from '../bindings/AppInfo';
import type { MockState } from './state';
import type { Handlers } from './types';

type HostCommands = 'app_info' | 'prefs_get' | 'prefs_set' | 'open_external' | 'reveal_path' | 'launch_sound' | 'chat_alerts_listen' | 'chat_alerts_view' | 'drop_listen' | 'attachment_save';

const MOCK_INFO: AppInfo = {
  name: 'Harness Desktop',
  version: '0.1.0',
  os: 'linux',
  arch: 'x86_64',
  debug: true,
};

/** App facts, preferences, the launch sound, chat alerts and the "open something outside the window" commands (no-ops in a browser). */
export function hostHandlers(state: MockState): Pick<Handlers, HostCommands> {
  return {
    app_info: () => MOCK_INFO,
    prefs_get: () => state.snapshot(),
    prefs_set: ({ prefs }) => state.save(prefs),
    open_external: () => null,
    reveal_path: () => null,
    launch_sound: () => null,
    // The mock has no native notifications: nothing raises a chat alert in a browser.
    chat_alerts_listen: () => null,
    chat_alerts_view: () => null,
    // The mock has no native window: files dropped in a browser arrive through HTML5 drag and drop.
    drop_listen: () => null,
    attachment_save: ({ name, data }) => ({ name, path: `/home/demo/.cache/attachments/mock/${name}`, bytes: atob(data).length }),
  };
}
