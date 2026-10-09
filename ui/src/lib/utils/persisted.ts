/**
 * Best-effort persistence of small UI choices (rail width, palette recents). Storage can be
 * unavailable or full (private mode, a locked-down webview); the choice then lasts for the session.
 */

export function readStored(key: string): string | null {
  try {
    return window.localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function writeStored(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value);
  } catch {
    // Not remembered; the choice still applies now.
  }
}

/** A JSON value stored under `key`, or `fallback` when it is missing, unreadable or fails `accept`. */
export function readJson<T>(key: string, fallback: T, accept: (value: unknown) => value is T): T {
  const raw = readStored(key);
  if (raw === null) {
    return fallback;
  }

  try {
    const parsed: unknown = JSON.parse(raw);

    return accept(parsed) ? parsed : fallback;
  } catch {
    return fallback;
  }
}

export function writeJson(key: string, value: unknown): void {
  writeStored(key, JSON.stringify(value));
}
