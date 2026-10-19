/**
 * How often the app looks again at things that change without it, so no view needs a Refresh button.
 * Every interval lives here, named for what it keeps current, so one place tunes them all.
 * A poll that runs through `autoRefresh` also runs when the window regains focus and never while it is hidden.
 */

/** Git status of the open project: the Changes list, its diffs and the count in the rail. */
export const GIT_POLL_MS = 4_000;

/** Saved sessions of the open project and the transcript on screen: sessions started or written in a terminal show up. */
export const SESSIONS_POLL_MS = 5_000;

/** The configuration shown in Config. */
export const CONFIG_POLL_MS = 5_000;

/** The plugins, marketplaces and packages shown in Extensions. */
export const EXTENSIONS_POLL_MS = 5_000;

/** Memory scripts, which an agent can add or change while the list is open. */
export const MEMORY_SCRIPTS_POLL_MS = 5_000;

/** Memory notes and the open note. */
export const MEMORY_NOTES_POLL_MS = 15_000;

/** A backstop for the change event of the hooks folder. */
export const HOOKS_POLL_MS = 15_000;

/** A backstop for the change events of the routines folder, the approvals and the runs. */
export const ROUTINES_POLL_MS = 15_000;

/** The run history on screen: a run that starts or ends while it is open shows up by itself, and a run that is going on grows in duration. */
export const ROUTINE_RUNS_POLL_MS = 5_000;

/** Ecosystem and the dashboard behind it. Their stores keep their own freshness windows, so a lapse costs a request, not every tick. */
export const ECOSYSTEM_POLL_MS = 30_000;

/** The Harness Dashboard card that shows how to install it: the user is running the install in a terminal and the card turns into the panel by itself. */
export const DASHBOARD_INSTALL_POLL_MS = 3_000;

/** Which harnesses are installed, while the setup shows them and the user may be installing one in a terminal. */
export const HARNESS_SETUP_POLL_MS = 3_000;

/** Back in the window: harness detection runs again unless it ran this recently. */
export const HARNESS_FOCUS_GAP_MS = 10_000;

/** A chat whose harness listed no model keeps asking, for the user may be signing in elsewhere. */
export const MODELS_POLL_MS = 5_000;

/** How often the app looks for a newer release of the Harness family while it is open. The backend still asks GitHub at most every six hours. */
export const COMPANION_CHECK_MS = 30 * 60_000;

/** How often the marketplace catalogs of every installed harness are re-read while "Refresh catalogs automatically" is on. */
export const CATALOG_REFRESH_MS = 30 * 60_000;

/** How often the installed harnesses are looked at for plugin and package updates, and for catalog refreshes. */
export const EXTENSION_UPDATE_INTERVAL_MS = 10 * 60_000;

/** The first look at the extensions, a little after start so it never competes with the app opening. */
export const EXTENSION_UPDATE_FIRST_DELAY_MS = 30_000;

/** PI cannot say whether a package has an update, so asking costs a network round trip per package. */
export const PACKAGE_UPDATE_INTERVAL_MS = 60 * 60_000;

/** A read that has not answered after this long is given up on: the next poll tick runs, and the late answer is ignored if a newer read finished. */
export const FLIGHT_STALE_MS = 60_000;

/** The waits between automatic retries of a view that failed to load. The last one repeats. */
export const RETRY_BACKOFF_MS: readonly number[] = [2_000, 5_000, 15_000, 30_000];
