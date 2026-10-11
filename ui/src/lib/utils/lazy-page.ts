const RELOAD_KEY = 'harness:lazy-page-reload';
/** A second failure within this window is real and shows the error panel instead of reloading again. */
const RELOAD_WINDOW_MS = 30_000;

/**
 * Loads a lazily split page. When its code cannot be fetched because the bundle changed underneath the window (the
 * dev server re-optimized its dependencies, or an update replaced the files), the app reloads once to pick up the
 * new code instead of showing an error; a failure that repeats right after that reload is shown as usual.
 */
export async function lazyPage<T>(load: () => Promise<T>): Promise<T> {
  try {
    return await load();
  } catch (error) {
    const stale = error instanceof Error && /dynamically imported module|Importing a module script failed/i.test(error.message);
    if (stale && reloadAllowed()) {
      location.reload();

      return new Promise<T>(() => {});
    }

    throw error;
  }
}

function reloadAllowed(): boolean {
  try {
    const last = Number(sessionStorage.getItem(RELOAD_KEY) ?? 0);
    if (Date.now() - last < RELOAD_WINDOW_MS) {
      return false;
    }

    sessionStorage.setItem(RELOAD_KEY, String(Date.now()));

    return true;
  } catch {
    return false;
  }
}
