export type DiffMode = 'unified' | 'split';

const STORAGE_KEY = 'harness.diffMode';

function stored(): DiffMode {
  try {
    return window.localStorage.getItem(STORAGE_KEY) === 'split' ? 'split' : 'unified';
  } catch {
    // Storage can be unavailable (private mode, a locked-down webview); the choice then lasts for the session.
    return 'unified';
  }
}

/** Unified or side-by-side, remembered across launches on a best-effort basis. */
export class DiffModeStore {
  value = $state<DiffMode>(stored());

  set(mode: DiffMode): void {
    this.value = mode;

    try {
      window.localStorage.setItem(STORAGE_KEY, mode);
    } catch {
      // Not remembered; the choice still applies now.
    }
  }
}

export const diffMode = new DiffModeStore();
