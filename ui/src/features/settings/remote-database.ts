import type { MessageKey } from '$lib/i18n/i18n.svelte';

/** The sub-headings of a remote database panel look like the section headings of Settings, one step smaller in the card. */
export const REMOTE_SUBHEADING = 'pt-4 text-sm font-medium tracking-wider text-fg-3 uppercase';

/** The words of the connection part of a remote database panel, in the keys of the Harness family member that owns it. */
export interface RemoteDatabaseCopy {
  connectionHeading: MessageKey;
  databaseTitle: MessageKey;
  databaseDescription: MessageKey;
  /** The point about the restart a change needs. */
  restart: MessageKey;
  /** The point about where the settings are stored. Takes `{ path }`. */
  stored: MessageKey;
  urlLabel: MessageKey;
  /** The placeholder while the saved URL stays. */
  urlKeep: MessageKey;
  /** Takes `{ url }`. */
  urlSaved: MessageKey;
  urlShow: MessageKey;
  urlHide: MessageKey;
  tokenLabel: MessageKey;
  tokenKeep: MessageKey;
  tokenPlaceholder: MessageKey;
  tokenShow: MessageKey;
  tokenHide: MessageKey;
  test: MessageKey;
  save: MessageKey;
  /** Shown after a save went through. */
  saved: MessageKey;
  /** What is wrong with the saved file. Takes `{ problem }`. */
  problem: MessageKey;
}

/** What one Harness family member offers as its team database. */
export interface RemoteDatabaseConfig<Kind extends string> {
  /** The databases to choose from, in the order of the menu. */
  kinds: readonly Kind[];
  /** The choice before anything is saved. */
  defaultKind: Kind;
  /** The URL schemes each database accepts; the backend checks again and decides. */
  schemes: Record<Kind, readonly string[]>;
  /** The databases that take a separate auth token. */
  tokenKinds: readonly Kind[];
  kindLabels: Record<Kind, MessageKey>;
  urlPlaceholders: Record<Kind, MessageKey>;
  urlProblems: Record<Kind, MessageKey>;
  copy: RemoteDatabaseCopy;
}

export function urlMatchesKind<Kind extends string>(config: RemoteDatabaseConfig<Kind>, kind: Kind, url: string): boolean {
  const lower = url.trim().toLowerCase();

  return config.schemes[kind].some((scheme) => lower.startsWith(scheme) && lower.length > scheme.length);
}

/**
 * The saved URL can stand in for a typed one when its scheme fits the chosen database (databases that share schemes keep the
 * saved URL when the user switches between them). The backend checks again and decides.
 */
export function savedUrlFits<Kind extends string>(config: RemoteDatabaseConfig<Kind>, kind: Kind, urlRedacted: string | null): boolean {
  return urlRedacted !== null && urlMatchesKind(config, kind, urlRedacted);
}

export function takesToken<Kind extends string>(config: RemoteDatabaseConfig<Kind>, kind: Kind): boolean {
  return config.tokenKinds.includes(kind);
}
