export type ConfigMode = 'structured' | 'source';

const STORAGE_KEY = 'harness.configMode';

function stored(): ConfigMode {
  try {
    return window.localStorage.getItem(STORAGE_KEY) === 'source' ? 'source' : 'structured';
  } catch {
    // Storage can be unavailable (private mode, a locked-down webview); the choice then lasts for the session.
    return 'structured';
  }
}

/** Structured or the file's own text, remembered across launches on a best-effort basis. */
export class ConfigModeStore {
  value = $state<ConfigMode>(stored());

  set(mode: ConfigMode): void {
    this.value = mode;

    try {
      window.localStorage.setItem(STORAGE_KEY, mode);
    } catch {
      // Not remembered; the choice still applies now.
    }
  }
}

export const configMode = new ConfigModeStore();
