import type { CodeBuildEvent } from './bindings/CodeBuildEvent';
import type { CodeIndexState } from './bindings/CodeIndexState';
import type { CodeSwitchResult } from './bindings/CodeSwitchResult';
import { cmd } from './command-spec';
import type { EventSink } from './transport';

/** The `code_*` commands, spread into `commands`. */
export const codeCommands = {
  /**
   * The "Code graph" setting: stores `codeEnabled` and registers or removes the harness-code MCP entry. `server.installed` false means nothing
   * was registered: offer `setup_install_code`. A different choice from `code_index_set`, which switches the `code_*` tools in the server.
   */
  code_set_enabled: cmd<{ enabled: boolean }, CodeSwitchResult>(),
  /**
   * Whether the code index is on (Harness Code `status --json`) and where the switch comes from: `source` is `default` (never set), `file`
   * (the user's choice) or `env` (the server's environment overrides it). Rejects with `unsupported` for a Harness Code without the
   * `code-index` feature, `notFound` when it is not installed.
   */
  code_index_get: cmd<undefined, CodeIndexState>(),
  /**
   * Turns the code index on or off (`enable|disable --json`, off hides the `code_*` tools and keeps the index) and answers the state as the
   * server reports it afterwards (an `env` source can still override `enabled`). Errors as `code_index_get`, `process` with the server's
   * line when it cannot write the choice.
   */
  code_index_set: cmd<{ on: boolean }, CodeIndexState>(),
  /**
   * Sends the code graph builds to `onEvent`, replacing the previous listener, and at once the builds still in progress. One event when a build
   * begins, moves into another 10 % step of its files, or ends (nothing while a file is unchanged). `buildId` identifies a build (a new number is a
   * new build of the same `root`), `kind` is `full` or `bulk`, `phase` is `building`, `ready`, `error` (`error` holds the reason), or `stale`
   * and `cancelled` (dismiss the toast, neither is a failure). Only the first index of a repository and a large change are announced.
   */
  code_builds_listen: cmd<{ onEvent: EventSink<CodeBuildEvent> }, null>(),
  /**
   * Starts a background build of the code index of `project`, only when it has none, and answers at once: the build can run for minutes and
   * shows up in `code_builds_listen`. Does nothing without support in the server, with the Code index switch off, on Windows, or while a
   * run for that path is going. The page asks once per project in an app run (again after the switch is turned back on), so a build that
   * failed is tried again on the next launch, or by the agent's first code tool call.
   */
  code_prepare: cmd<{ project: string }, null>(),
  /**
   * Whether the background updater may install Harness Code now, for a user who has Harness Memory without it. True once: the answer is
   * recorded before the install starts, so a failed install, or a Harness Code removed later, is never tried again unasked. False while the
   * Code graph setting or either auto-update switch is off, and after the first true.
   */
  code_auto_install_claim: cmd<undefined, boolean>(),
};
