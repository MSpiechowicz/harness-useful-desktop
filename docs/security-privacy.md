# Security and privacy

Every statement below is checked against the code; the file or symbol that enforces it is named. Where something is
not guaranteed, it says so under [Limits](#limits).

## Guarantees in short

- Local only. No telemetry, no analytics, no accounts, no sign-in, no tokens stored.
- The app itself makes one kind of network request: the update check and download, and only when the build carries
  an updater key. See [Network](#network).
- The app never reads credential stores, databases, logs or private keys, even through a symlink.
- Secrets in configuration and in the app's own CLI output are masked before they reach the screen.
- No shell is ever spawned. Every process has an absolute program path and an explicit argument list.
- The webview has no filesystem, shell, dialog, opener or updater API. It can call only the app's own commands.

## Threat model

| Threat | Defence |
| --- | --- |
| A project (cloned repository, hostile files) tries to make the app read secrets or run code | Deny-list and `SafeReader`; hardened git (see [Limits](#limits) for filter drivers); project paths only from the native picker; the app executes no project file itself |
| Content shown in the UI (agent output, Markdown, config text) tries to script the webview or reach the network | Strict CSP, DOMPurify-sanitized Markdown, no `eval`, no `fetch`; the webview can only call allow-listed commands |
| The webview (or a compromised dependency) tries to escape through Tauri | ACL grants only app commands; no `core:*`, no plugin permissions; navigation and pop-up guards |
| A UI bug passes a path or URL the user did not choose | Rust re-validates: active/recent project check, `canonical_within`, link allowlist, argv charset |
| A tampered or downgraded update | Signature check against the built-in key, signed version, HTTPS only, newer versions only |
| Credentials leak into logs, prefs or fixtures | Payload-free logging; `prefs.json` holds only the fields of `AppPrefs`; canary tests; fixtures checked for secrets |
| Another local user reads app data | Prefs directory `0700` and file `0600` on Unix |

Not in scope: the harness CLIs themselves. They run with your privileges and talk to their providers exactly as in
your terminal (see [Limits](#limits)).

## Network

The app contains no analytics client and no HTTP client of its own besides the updater. Enumerated:

| Request | When | Details |
| --- | --- | --- |
| Update check and download | Only if `plugins.updater.pubkey` in `src-tauri/tauri.conf.json` holds a valid public key (`configured_pubkey`, `business/src/update.rs`), and the user or the update policy asks | One URL: `UPDATE_ENDPOINT`, the GitHub release `latest.json`. The client is `https_only(true)` with 15 s connect and 30 s read timeouts (`src-tauri/src/commands/updates.rs`). The code sends nothing beyond the updater's standard headers. |
| Opening a link | Only when the user clicks one | `open_external` hands an allowlisted `https` URL to the OS browser. The app itself does not fetch it. |

Everything else:

- With the placeholder or an invalid key, `update_check` and `update_install` fail with `unsupported` ("updates are not
  configured for this build") before any network access (`src-tauri/src/updates.rs`, tested).
- The update policy (`updatePolicy` in prefs) is `startupAsk` (default), `autoInstall` or `off`. With `off`, the app
  checks only when asked.
- The webview cannot open connections: the CSP allows `connect-src ipc: http://ipc.localhost` only (the dev CSP adds
  the Vite websocket), and the guard rejects `fetch`, `XMLHttpRequest`, `WebSocket` and `EventSource` in `ui/src`.
  Images are limited to `'self' data: blob:`, so remote images in Markdown do not load. Fonts are bundled.
- The Harness Dashboard widgets talk to a local process (`harness-dashboard mcp`) over stdio. The app starts it with
  `mcp` as the only argument, never passes `--db`, and never reads its database or `auth-token`. Whatever that
  program does on its own is outside this app.
- Harness CLIs make their own provider requests. Extension actions (`omp plugin install`, `pi install`) run the CLI,
  which may download packages. The UI asks first and previews with the CLI's dry-run flag where one exists.
- The Ecosystem tab shows install commands for copying. The app never runs them (`business/src/ecosystem/catalog.rs`).

## Files the app never reads

`SafeReader` (`business/src/security/deny.rs`) is the only way the business layer reads files; a grep of
`business/src` for direct file reads finds none outside it (directory listings and metadata aside). Names compare
case-insensitively.

| Rule | Entries |
| --- | --- |
| Extensions | `db`, `db-wal`, `db-shm`, `db-journal`, `sqlite`, `sqlite3`, `log`, `pem`, `key`, `p12`, `pfx` |
| Exact names | `auth.json`, `mcp-auth.json`, `install-id`, `auth-token`, `models-store.json`, `gpu_cache.json`, `credentials`, `credentials.json`, `.env` |
| Name prefixes | `.env.`, `id_rsa`, `id_ed25519` |
| Directories | `terminal-sessions`, `url-search`; `blobs` below an `agent` directory |

`SafeReader` also:

1. checks the path as given, then every symlink hop (up to 40), then the canonical path, so a link to a denied file is
   refused;
2. refuses anything that is not a regular file (a FIFO or device is never opened; Unix opens use `O_NONBLOCK`), and
   checks again on the open handle;
3. caps the size (`DEFAULT_MAX_BYTES` = 2 MiB; prefs 256 KiB) and rejects invalid UTF-8.

The deny-list also applies where git is involved. Untracked files are read by the app itself, through `SafeReader`
and `canonical_within` (a file that resolves outside the project is not read), and denied or over-512 KiB files are
listed without content. For tracked files git produces the diff, and the app clears the hunks of any denied path.
Session scans skip denied paths and never enter a session's companion directory.

Credential files are never listed in the configuration view either (PI: `auth.json`, `mcp-auth.json`,
`models-store.json`). The Harness Dashboard's `usage.db` and `auth-token` fall under the same rules.

The `canary` tests (`business/tests/canary.rs`, `adapter_canary.rs`) hide a marker in denied files and secret values of
fixture homes and fail if any read or view contains it.

## Secret redaction

Parsed configuration becomes a `ConfigValue` through `Redactor` (`business/src/security/redact.rs`); only the redacted
value may cross the IPC boundary. For each string leaf, the first matching rule wins:

1. A value starting with `!` is a secret-producing command: hidden, never executed.
2. Exactly `$NAME` or `${NAME}`: the variable name is shown, its value is never read.
3. Anything under a key named `env` or `headers`, and under `auth` or `oauth` except `type`, `scope`, `redirectUri`,
   `callbackPort`, `callbackPath`, `prompt`, `clientId`: hidden (numbers too).
4. A key on the path containing `apikey`, `token`, `secret`, `password`, `passwd`, `credential`, `authorization`,
   `bearer`, `privatekey`, `clientsecret`, `accesskey`, `sessionkey`, `cookie`, `signature` or `credentialid` (case and
   `_`/`-` ignored): strings hidden; numbers and booleans stay visible (`maxTokens: 4096`).
5. A well-known credential format anywhere in the string: `sk-`, `sk-ant-`, `ghp_`/`gho_`/`ghs_`/`ghu_`,
   `github_pat_`, `xox?-`, `AKIA...`, `AIza...`, `glpat-`, `hf_`, `npm_`, JWTs, `Bearer`/`Basic` credentials, PEM
   blocks.
6. High entropy: 32 or more characters, three or more character classes, at least 4.0 bits per character. Paths, URLs,
   UUIDs and word-like identifiers are exempt. False positives are accepted over false negatives.
7. Otherwise the text is shown, with URL userinfo and secret-looking query values masked.

`redact_text` applies the format, `NAME=value`, URL and entropy rules to free text. It is used on:

- agent stderr lines (also capped at 2 KiB per line and 20 lines per second) and process diagnostics;
- agent error messages, extension dialog messages and `bash` command output (capped at 1 MiB);
- `omp plugin` and `pi` action output (ANSI stripped first, 256 KiB per stream);
- marketplace URIs, MCP client errors, and package sources (`Redactor` at their key path).

**Not redacted:** the chat itself. Assistant text, thinking, tool arguments and tool results, and the transcripts
of saved sessions, are shown as the agent produced them. They are your own session data, shown only to you on this
machine, and are never stored by the app or logged. `redact_text` is heuristic; do not rely on it to hide a secret you
typed into a prompt.

## Path containment

- `canonical_within(root, candidate)` (`security/paths.rs`) canonicalizes both, resolving `..` and symlinks, and
  requires the result to lie inside the root (`Forbidden` otherwise).
- `validate_project_dir`: after `~` expansion the path must be absolute, exist, be a directory and not be a
  filesystem root.
- Projects: a new directory enters only through the native folder dialog (`project_pick`). The webview can re-open
  or forget only projects already stored; every command that takes a project (`agent_start`, `config_view`,
  `git_*`, `sessions_*`, `extension_action`) requires it to be the active or a recent one.
- `reveal_path` (`src-tauri/src/access.rs`): the path must be absolute, contain no `..`, and after symlink resolution
  lie inside a harness `config_root` or `agent_dir` or the active project. A missing path outside the roots is
  `Forbidden`, not `NotFound`, so the command cannot probe which files exist.
- Sessions: the webview sends `SessionRef { harness, id }`. The id (at most 128 characters, checked by
  `is_safe_session_id`) is resolved by a shallow scan of the project's session folder; it can never name another file.
- Git diff paths must be relative: no `..`, no NUL, at most 1000 paths of 4 KiB each.
- `Forbidden` messages never reach the UI: the shell replaces them with "access refused by a security rule"
  (`src-tauri/src/error.rs`).

## `open_external`

`allowed_link` (`src-tauri/src/access.rs`) parses the URL and accepts it only if it is `https`, has no credentials, and
matches an allowlist prefix on scheme, host and port, with the path equal to the prefix or continuing it at a `/`
boundary. The allowlist is two static prefixes (`https://github.com/MSpiechowicz/`,
`https://github.com/earendil-works/pi`) plus the `homepage` and install `docs_url` of each registered harness. So
`github.com.evil.com`, `github.com@evil.com`, `.../pi-evil` and `javascript:` URLs are refused. Chat Markdown links are
inert buttons that carry only `http(s)` targets; clicking calls the same command.

## Process spawning

- **No shell.** `build_command` (`business/src/process/mod.rs`) requires an absolute program path and passes the
  argument list verbatim. `scripts/check-forbidden.ts` rejects `Command::new("sh" | "bash" | "cmd" | ...)` with no
  opt-out.
- **Binary resolution.** `resolve_binary` uses absolute `PATH` entries only, then the standard install directories. A
  relative or empty `PATH` entry is ignored, so a project cannot plant a binary. A user-chosen override must be an
  absolute existing file whose name is exactly the harness binary name, and is set only through the native file
  dialog (`harness_pick_binary`), never through `prefs_set`.
- **Arguments.** Extension targets pass `cli_target`: non-empty, 512 characters at most, no leading `-`, only
  `A-Z a-z 0-9 @ . _ / : + = , -`, no URL credentials. Windows `.cmd`/`.bat` shims (re-parsed by `cmd.exe`) only
  receive arguments that pass `is_safe_arg` after the leading dashes are removed. The webview sends typed actions,
  never command lines.
- **Flags never passed.** `--auto-approve`, `--no-ui`, login and credential RPCs (`get_login_providers`),
  `set_ask_dialog`. `--approve` goes to PI only for the user's explicit "trust this project once" choice, for that launch.
  The OMP `--approval-mode` flag is sent only when the user picks a per-session mode. The app does not edit either
  harness's settings.
- **Shell commands from the composer.** `agent_bash` fails with `invalidInput` unless the UI passes `confirmed: true`
  after asking the user; the agent runs the command, not the app.
- **Git.** `GitCli` (`business/src/git/cli.rs`) runs `git -C <project>` with `--no-pager --literal-pathspecs
  --no-optional-locks` and `-c core.fsmonitor=false -c core.untrackedCache=false -c diff.external= -c core.pager=cat
  -c color.ui=false -c core.quotepath=off`, a 15 s timeout and a 16 MiB output cap per stream. It sets
  `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0`, `LC_ALL=C`, and `HOME`/`XDG_CONFIG_HOME` from the injected `Env`.
  Every inherited `GIT_*` variable is dropped (`GIT_DIR`, `GIT_CONFIG_*`, `GIT_EXTERNAL_DIFF`, `GIT_SSH*`,
  `GIT_ASKPASS`, `GIT_TRACE*`, ...). Diffs use `--no-ext-diff --no-textconv`. Git is read-only here: status, diff,
  branch list.
- **Children.** Each child gets `kill_on_drop` and, on Unix, its own process group, which is killed with it. Agents stop by
  closing stdin, waiting 3 s, then killing the group. Output of run-to-completion commands has a time limit and a size
  cap.
- **Dashboard.** `harness-dashboard mcp` runs in the home directory, without `CLAUDE_CODE_SESSION_ID`, and stops after 30 s without
  a query.

## Tauri ACL, CSP and webview

- **Capability** (`src-tauri/capabilities/main.json`, window `main`): 38 `allow-<command>` permissions, one per app
  command, generated from `COMMANDS` in `src-tauri/build.rs`. There is no `core:*` permission and no plugin
  permission. The dialog, opener, window-state, log, single-instance and updater plugins are driven from Rust only, so
  their JavaScript APIs (and the log plugin's `log` command) are unreachable. `withGlobalTauri` is `false`. Channel
  delivery for streaming is not ACL-gated and needs no grant.
- **CSP** (`app.security.csp`): `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'
  data: blob:; font-src 'self' data:; connect-src ipc: http://ipc.localhost; worker-src 'self' blob:; object-src 'none';
  base-uri 'none'; form-action 'none'; frame-src 'none'`. `script-src` allows no inline script and no `eval`. Inline
  styles are allowed (`dangerousDisableAssetCspModification` is set for `style-src`). `freezePrototype` is on.
- **Navigation.** The window is created from code (`"create": false`) so guards are attached before any page loads:
  navigation is allowed only to the app origin (plus the dev server in `tauri dev`), and every pop-up is denied
  (`src-tauri/src/window.rs`). Release builds hide the webview's default context menu except over editable text and
  selections. A second app launch only focuses the window; its argv is ignored.
- **Markdown.** Rendered with markdown-it, then DOMPurify with a fixed tag and attribute list (`ui/src/lib/markdown/sanitize.ts`).
  `{@html}` is allowed only in the Markdown component (guard rule `html-injection`); `innerHTML` assignment and
  `eval` have no opt-out.
- **Errors.** Command errors are `{ code, message }`; messages describe what failed and carry no secret value or
  file content.

## Updater

Details of keys and releases: [release-and-updater-keys.md](release-and-updater-keys.md).

- The public key is `plugins.updater.pubkey` in `tauri.conf.json`, compiled into the app. Updates are verified with
  that key only; there is no way to supply another.
- `update_check` builds the updater with the fixed endpoint (`UPDATE_ENDPOINT`), the key, and an HTTPS-only client.
  `plugins.updater.endpoints` lists the same URL over HTTPS.
- `requireSignedVersion` is `true`: the signature must have been made for the version `latest.json` announces. Missing
  or mismatching versions fail as `forbidden` (`SignedVersionMismatch`, `MissingSignedVersion`). A test deserializes the
  shipped config and asserts `require_signed_version` is set and `allow_downgrades`, `dangerous_accept_invalid_certs`
  and `dangerous_accept_invalid_hostnames` are not.
- `Update::download` verifies the signature before it returns the bytes, so nothing unverified is installed. Only a
  newer version is offered; downgrades are not allowed.
- The guard rejects any `dangerousInsecureTransportProtocol` setting.
- One install runs at a time. `app_relaunch` stops agent sessions first, as on any exit.
- macOS and Windows installers are not code-signed; see "macOS and Windows: unsigned installers" in the release
  document. Updates are still signature-verified by the updater key.

## What is stored on disk

| What | Where | Content |
| --- | --- | --- |
| `prefs.json` | The OS app-config directory for the identifier `com.mspiechowicz.harness-useful-desktop` (Tauri `app_config_dir`) | `AppPrefs` only (below). Written atomically (temp file, `sync_all`, rename); directory `0700`, file `0600` on Unix. A corrupt file is renamed to `prefs.json.corrupt-<unix-seconds>` and defaults load. |
| Window size and position | App state kept by `tauri-plugin-window-state` | Geometry only. |
| UI choices | Webview `localStorage` | Rail expanded (`harness.railExpanded`), command-palette recents (`harness.paletteRecents`), diff layout. |
| Log files | The OS log directory | See [Logging](#logging). Rotated at 1 MiB, 3 files kept. |

`AppPrefs` (`business/src/model/prefs.rs`) has twelve fields and never holds secrets or chat content:
`version`, `theme`, `motion`, `introEnabled`, `defaultHarness`, `activeProject`, `rememberRecentProjects`,
`recentProjects` (12 at most), `updatePolicy`, `onboardingCompleted`, `approvalNoticeSeen`, `binaryOverrides`.

**Server-managed fields** (`business/src/prefs/managed.rs`): `activeProject`, `recentProjects` and `binaryOverrides`
decide which directories are read and which programs run. Only Rust changes them, through the native pickers and the
project and harness commands. `prefs_set` stores the UI-managed fields only (`theme`, `motion`, `introEnabled`,
`defaultHarness`, `rememberRecentProjects`, `updatePolicy`, `onboardingCompleted`, `approvalNoticeSeen`) and keeps the
rest as stored. The exhaustive destructuring in `with_ui_fields` makes a new field fail to compile until it is
classified. Turning off `rememberRecentProjects` clears the recent list.

The app stores no tokens, no chat content, no prompts and no file contents. Saved sessions belong to the harness
(its own session folder); the app reads them and never copies them.

## Logging

Logs go to the OS log directory (and stdout in debug builds) at `Info` level (`src-tauri/src/logging.rs`). They record
what the app does: startup version, session start and stop with the session key and harness, update check results,
blocked navigations (scheme and host only), and errors whose messages name an operation, a path or a binary.

Never logged: prompts, chat content, IPC payloads, file contents, secrets, process arguments, URL paths or queries.
The filter drops the `tauri::ipc` and webview-console targets, and the webview cannot log at all (the log plugin's
command is not granted). Process errors name only the program's file name (`ExecSpec::label`).

## Limits

- **Agents are not sandboxed.** A harness runs with your privileges, your environment (so it sees your provider
  configuration) and your files. The app's deny-list, redaction and approvals do not restrict it. Approval behaviour is
  the harness's.
- **Chat content is not redacted** (see above).
- **Git filters.** A repository's `.git/config` can name clean/smudge `filter` drivers; git may run them while `status`
  or `diff` re-reads a changed file, as in a terminal (`business/src/git/mod.rs`). Open only repositories you trust.
- **Windows.** Only the direct child of a spawned process is killed (no job object yet), so grandchildren may outlive
  it.
- **Heuristics.** The entropy rule and key-name rules can hide harmless values and can miss unusual secrets.
- **Unsigned installers** on macOS and Windows show an OS warning on first install.
