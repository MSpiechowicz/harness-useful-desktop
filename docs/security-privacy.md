# Security and privacy

Every statement below is checked against the code; the file or symbol that enforces it is named. Where something is
not guaranteed, it says so under [Limits](#limits).

## Guarantees in short

- Local only. No telemetry, no analytics, no accounts, no sign-in, no tokens stored.
- The app itself makes two kinds of network request, both to GitHub and both possible to turn off: the update check and
  download (only when the build carries an updater key), and the companion release lookup. See [Network](#network).
- The app never reads credential stores, databases, logs or private keys, even through a symlink.
- The only things the app writes into a harness's configuration are entries in its user `mcp.json`: the
  `harness-dashboard` entry when you choose it in Recommended setup, and the MCP servers you tick when you import a
  shared setup. Never an overwrite. See [Process spawning](#process-spawning) and [Shared setups](#shared-setups).
- A shared setup carries references and the names (never the values) of environment variables and headers, and nothing
  in it runs until you have read the review and confirmed. Secret and path detection in free-form text is best effort,
  and host names are part of sources and URLs. Bundles are not signed. See [Shared setups](#shared-setups).
- Secrets in configuration and in the app's own CLI output are masked before they reach the screen.
- No shell is spawned, with one exception: the Harness Dashboard installer of Recommended setup, a fixed command that
  runs only after you confirm it (see [Process spawning](#process-spawning)). Every process has an absolute program
  path and an explicit argument list.
- The webview has no filesystem, shell, dialog, opener or updater API. It can call only the app's own commands.

## Threat model

| Threat | Defence |
| --- | --- |
| A project (cloned repository, hostile files) tries to make the app read secrets or run code | Deny-list and `SafeReader`; hardened git (see [Limits](#limits) for filter drivers); project paths only from the native picker; the app executes no project file itself |
| Content shown in the UI (agent output, Markdown, config text) tries to script the webview or reach the network | Strict CSP, DOMPurify-sanitized Markdown, no `eval`, no `fetch`; the webview can only call allow-listed commands |
| The webview (or a compromised dependency) tries to escape through Tauri | ACL grants only app commands; no `core:*`, no plugin permissions; navigation and pop-up guards |
| A UI bug passes a path or URL the user did not choose | Rust re-validates: active/recent project check, `canonical_within`, link allowlist (any `https` URL without credentials when `openAnyLink` is on), argv charset |
| A tampered or downgraded update | Signature check against the built-in key, signed version, HTTPS only, newer versions only |
| A shared setup (pasted string or file) tries to run a command, install something unexpected or leak a secret | Every field validated in Rust; the webview sends only a plan id and item ids; only add-actions exist; review and explicit confirmation first; env and header values, `auth` and `oauth` never exported, secrets in free-form text caught by heuristics; see [Shared setups](#shared-setups) |
| Credentials leak into logs, prefs or fixtures | Payload-free logging; `prefs.json` holds only the fields of `AppPrefs`; canary tests; fixtures checked for secrets |
| Another local user reads app data | Prefs directory `0700` and file `0600` on Unix |

Not in scope: the harness CLIs themselves. They run with your privileges and talk to their providers exactly as in
your terminal (see [Limits](#limits)).

## Network

The app contains no analytics client. Its only HTTP clients are the updater and the companion release lookup; the one
other download, the dashboard installer, is made by the installer command itself, and only after consent. Enumerated:

| Request | When | Details |
| --- | --- | --- |
| Update check and download | Only if `plugins.updater.pubkey` in `src-tauri/tauri.conf.json` holds a valid public key (`configured_pubkey`, `business/src/update.rs`), and the user or the update policy asks | One URL: `UPDATE_ENDPOINT`, the GitHub release `latest.json`. The client is `https_only(true)` with 15 s connect and 30 s read timeouts (`src-tauri/src/commands/updates.rs`). The code sends nothing beyond the updater's standard headers. |
| Companion release lookup | When the Ecosystem view loads or is refreshed, at most once per companion every 6 hours while the app runs (the cache is in memory, so a restart asks again; failed lookups, including rate limits, retry after 5 minutes), and only while **Check companions for updates** (`checkCompanionUpdates` in prefs, on by default) is on. A prefs file that exists but cannot be read counts as off. | Three fixed URLs, `https://api.github.com/repos/MSpiechowicz/harness-useful-{dashboard,skills,sidebar}/releases/latest`, HTTPS only, with timeouts and a capped response. Only `tag_name` is read. The request carries a `User-Agent` (`harness-desktop/<version>`) and an `Accept` header and nothing else: no token, no cookies, no referer, no installed versions or other local data. While at least one companion is installed, all three are requested, so the request set does not reveal which ones (with none installed, nothing is requested); GitHub sees the IP address, the app version and when Ecosystem was opened. The comparison with the installed version happens on this machine (`ecosystem_releases`). Redirects are followed only to `https://api.github.com` on port 443. System proxy settings apply, as for the updater. A failed lookup only means no update badge. |
| Dashboard installer download | Only when the user confirms **Install the dashboard** in Recommended setup (onboarding, or **Run recommended setup** in Ecosystem) | The app opens no connection itself. The installer command it runs (a `curl` of `install.sh` piped to `sh` on Linux and macOS, `irm` of `install.ps1` piped to `iex` on Windows) fetches the script from `https://raw.githubusercontent.com/MSpiechowicz/harness-useful-dashboard/main/` and then whatever that script downloads, the dashboard release. GitHub sees the IP address and the request. See [Process spawning](#process-spawning). |
| Opening a link | Only when the user clicks one | `open_external` hands an allowlisted `https` URL (any `https` URL with **Open any web link** on) to the OS browser. The app itself does not fetch it. |

- **AI chat names** (`aiChatNames`, off by default). When on, after the first message of a new chat the app starts one
  throwaway agent of the chat's own harness (`utility` launch: `--no-session --no-tools --thinking off`, no resume,
  stopped right after the reply), on the chat's model, and sends it the first 500 characters of that message with a
  fixed instruction. The app opens no connection itself; the request goes through the harness exactly as your own chat
  does, and costs one very small request per chat. Without it, names come from the first message locally.
- **Automatic extension updates** (`autoUpdateExtensions`, on by default). While on, every ten minutes the app runs the
  active harness's own CLI (`omp plugin marketplace update`, `omp plugin upgrade`, `pi update --extensions`) through the
  same `extension_action` command and validation as the buttons in Extensions, and shows a toast before it installs
  anything. The app opens no connection itself; the CLI fetches from the marketplaces, npm and git sources you already
  configured. Off: nothing runs until you press an update button.

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
  which may download packages. The UI asks first and previews with the CLI's dry-run flag where one exists. Recommended
  setup installs the Harness Skills for OMP the same way (`omp plugin marketplace add`, then `omp plugin install` at
  user scope, through `extension_action`, after you confirm). For PI it only shows `pi install
  git:github.com/MSpiechowicz/harness-useful-skills` for copying; the app never runs it.
- Share setup opens no connection. Export and import read and write local files; an import's marketplace, plugin and
  package installs go through the harness CLI, which fetches from the sources in the setup you confirmed, exactly as
  `omp plugin install` or `pi install` do in your terminal.
- The Ecosystem tab shows install commands for copying (`business/src/ecosystem/catalog.rs`). The app never runs them,
  except the Harness Dashboard installer of Recommended setup (see [Process spawning](#process-spawning)).

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

The **Open any web link** setting (`openAnyLink`, off by default) widens this to any `https` URL without credentials
(`secure_link`), so links the agent posts, such as a pull request, open too. Rust reads the preference itself on each
call and fails closed: a missing or unusable prefs file keeps the allowlist. The setting is UI-managed (the webview
sets it through `prefs_set`), so it is a user choice rather than a boundary against a compromised webview, which could
already reach the agent through `agent_send` and `agent_bash`. With it on, the link text is the agent's: the real
target shows as the link's tooltip, and `https` links to `localhost` or the local network open too.

## Process spawning

- **No shell.** `build_command` (`business/src/process/mod.rs`) requires an absolute program path and passes the
  argument list verbatim. `scripts/check-forbidden.ts` rejects `Command::new("sh" | "bash" | "cmd" | ...)` with no
  opt-out. The single exception is the dashboard installer below.
- **Dashboard installer (the one shell).** Recommended setup can run the official Harness Dashboard installer
  (`setup_install_dashboard`, `business/src/ecosystem/installer.rs`). It is the only place the app runs an install
  command and names a shell. The command is a Rust constant per OS (`DASHBOARD_INSTALL_SH` and
  `DASHBOARD_INSTALL_PS1_SCRIPT` in `business/src/ecosystem/catalog.rs`): `/bin/sh -c` on Linux and macOS, Windows
  PowerShell from `<SystemRoot>\System32\WindowsPowerShell\v1.0` with `-NoProfile` on Windows. The webview sends only
  `confirmed`; no command line, path or URL. The command fails with `invalidInput` unless `confirmed` is true (the UI
  asks first and lists what the installer changes), and only one install runs at a time. The child runs in the home
  directory with `NO_COLOR=1`, inherits the app's environment, has its stdin closed, lives in its own process group
  and is killed with it after 10 minutes. Its output reaches the UI line by line with escapes stripped and secrets
  redacted (`redact_text`; 2 KiB per line, 256 KiB in all); the log keeps only counts and the exit code. Success needs
  exit code 0 and a `harness-dashboard` that reports a version afterwards, because a piped download can exit 0 after
  failing. The script comes from the `main` branch of the dashboard's repository; the app does not pin or verify it
  (see [Limits](#limits)). The guard keeps the exception narrow: rule `installer-shell` in
  `scripts/check-forbidden.ts` rejects `/bin/sh` and `powershell.exe` in any Rust file except
  `business/src/ecosystem/installer.rs` and `business/tests/`, with no opt-out; inside those files every line that
  names one must carry `guard-allow: installer-shell`.
- **Binary resolution.** `resolve_binary` uses absolute `PATH` entries only, then the standard install directories. A
  relative or empty `PATH` entry is ignored, so a project cannot plant a binary. A user-chosen override must be an
  absolute existing file whose name is exactly the harness binary name, and is set only through the native file
  dialog (`harness_pick_binary`), never through `prefs_set`. Harness children get the binary's directory and the
  standard install directories appended (never prepended) to their `PATH`, so the user's own entries keep precedence.
- **Arguments.** Extension targets pass `cli_target`: non-empty, 512 characters at most, no leading `-`, only
  `A-Z a-z 0-9 @ . _ / : + = , -`, no URL credentials. Windows `.cmd`/`.bat` shims (re-parsed by `cmd.exe`) only
  receive arguments that pass `is_safe_arg` after the leading dashes are removed. The webview sends typed actions,
  never command lines.
- **Flags never passed.** `--auto-approve`, `--no-ui`, login and credential RPCs (`get_login_providers`),
  `set_ask_dialog`. `--approve` goes to PI only for the user's explicit "trust this project once" choice, for that launch.
  The OMP `--approval-mode` flag is sent only when the user picks a per-session mode. The app does not edit either
  harness's settings, with two exceptions that both add entries to the user `mcp.json` of a detected harness:
  Recommended setup adds a `harness-dashboard` entry (`setup_register_mcp`, `business/src/ecosystem/mcp_register.rs`),
  and importing a shared setup adds the MCP servers you ticked (see [Shared setups](#shared-setups)). Both use one
  merge (`merge_servers`, `business/src/ecosystem/mcp_merge.rs`). The webview sends only a harness id (Recommended
  setup) or a plan id and item ids (import); the file is `<agent dir>/mcp.json` from the harness's own paths. For the
  dashboard, both the harness and the dashboard must be installed; the app merges `{"command": "harness-dashboard",
  "args": ["mcp"]}` into `mcpServers` and writes nothing when an entry for the dashboard already exists (in that file or
  one of the harness's other MCP files, even switched off; it never enables one). Both keep every other key and server
  and never overwrite an existing name. The merge refuses, and changes nothing, on invalid JSON, a root or
  `mcpServers` that is not an object, a symlink, anything but a regular file, a file over 2 MiB, or a file that changed
  while it was read. The write is atomic (temporary sibling, flushed, renamed) and keeps the file's permissions; a new
  file is created with `0600`. File contents are never logged or returned.
- **Shell commands from the composer.** `agent_bash` fails with `invalidInput` unless the UI passes `confirmed: true`
  after asking the user; the agent runs the command, not the app.
- **Git.** `GitCli` (`business/src/git/cli.rs`) runs `git -C <project>` with `--no-pager --literal-pathspecs
  --no-optional-locks` and `-c core.fsmonitor=false -c core.untrackedCache=false -c diff.external= -c core.pager=cat
  -c color.ui=false -c core.quotepath=off`, a 15 s timeout and a 16 MiB output cap per stream. It sets
  `GIT_TERMINAL_PROMPT=0`, `GIT_OPTIONAL_LOCKS=0`, `LC_ALL=C`, and `HOME`/`XDG_CONFIG_HOME` from the injected `Env`.
  Every inherited `GIT_*` variable is dropped (`GIT_DIR`, `GIT_CONFIG_*`, `GIT_EXTERNAL_DIFF`, `GIT_SSH*`,
  `GIT_ASKPASS`, `GIT_TRACE*`, ...). Diffs use `--no-ext-diff --no-textconv`. Git reads status, diffs and the branch
  list; its only writes are `git_discard_all` and `git_discard_files` (one file and, for a rename, its old name), which the UI runs only after the user confirms: `restore --source=HEAD
  --staged --worktree` and `clean -f -d` (no `-x`, so ignored files stay), both limited to the project directory.
- **Children.** Each child gets `kill_on_drop` and, on Unix, its own process group, which is killed with it. Agents stop by
  closing stdin, waiting 3 s, then killing the group. Output of run-to-completion commands has a time limit and a size
  cap.
- **Dashboard.** `harness-dashboard mcp` runs in the home directory, without `CLAUDE_CODE_SESSION_ID`, and stops after 30 s without
  a query.

## Shared setups

Share setup (Settings) exports the extensions and MCP servers of your harnesses as a `hud1:` string or a `.json` file
and imports one made elsewhere. Format reference: [share-setup.md](share-setup.md). Code: `business/src/sync/`.

**What an export contains.** References: marketplace sources, plugin ids, package sources, MCP commands, arguments and
URLs, and the *names* of environment variables and headers. Never their values, and never `auth`/`oauth`, `cwd` or other
MCP fields: those are structural, because the exporter copies only an allowlist of fields. Host names stay, since they
are part of sources and URLs (an internal git host or MCP URL is visible to whoever gets the file). Secrets and machine
paths inside free-form arguments and URLs are caught by heuristics (see below), which can miss an exotic form: read the
export list before sharing. Rust rebuilds the bundle from the harnesses' own user files (through `SafeReader`;
credential files are never opened) on every preview and export, and the webview sends back only item ids.

**What an export leaves out** (listed by name and reason, never by value): MCP servers whose command, arguments or URL
the redactor would change (`secrets.rs` builds on `Redactor` and `redact_text`), that carry a secret flag (`-p`/`-P` with
a non-numeric value, `--password`, `--token`, `--key`, `--pat`, `user:password` after `-u`/`--user`/`--auth`, ...), a
machine path (an absolute, home or drive path after `=` or `@`, or a system or user directory after `:`, `,`, `;` or
`scheme:///`) or a `${VAR}`/`$NAME`
reference the server does not declare, the dashboard's own `harness-dashboard` entry (Recommended setup installs it),
sources with credentials, local or unknown packages, local marketplaces, `file:`/`link:` plugin specs, project-scope
items and anything over the limits. Export runs the same validators as import, so an exported file imports again without
invalid items.

**An import is untrusted input.** A pasted string or picked file is decoded defensively (`codec.rs`): pasted text at
most 128 KiB, compressed payload 96 KiB, decompressed JSON 256 KiB read through a capped reader (zip-bomb guard), at
most 16 sections, 200 items per kind and 500 in total, serde_json's recursion limit kept. Every field of every item is
then validated in Rust (`validate.rs`, `validate_source.rs`, `validate_mcp.rs`, `secrets.rs`):

- Display safety: control characters, bidirectional controls and every Unicode format, line-separator and
  paragraph-separator character (Cf, Zl, Zp), and invisible fillers or variation selectors are rejected.
- Sources and ids: the `cli_target` charset, no leading `-` (also none right after `npm:`, `git:`, `@` or a ref, and no
  host starting with `-`), no `..`, no local path, no credentials, no secret-looking value. Marketplace sources are a
  GitHub `owner/repo` or an `https://` URL; PI sources are `npm:`, `git:` or `https://`, and a bare `ssh://` or
  `http://` source is refused. An `npm:` spec is a registry name with an optional version, range or tag,
  `github:owner/repo`, or an `https://`, `git+https://` or `git+ssh://` URL on a host name (a `git:` source takes
  `https://` only for a URL). Specs that name a path (starting with `.`, `/`, `~`, `\` or a drive letter), a tarball or
  archive (`.tgz`, `.tar`, `.tar.gz`, `.zip`), `file:`, `link:`, `git+file:`, `http:`, `git+http:` or `git:` are
  refused. Every URL or git host must be a plain host name.
- MCP: the server name pattern (`harness-dashboard` is reserved); the command must be a bare program name, not a path
  and not a shell interpreter; URLs are `http(s)` without user info; arguments and URLs may not reference environment
  variables unless the item declares the name in `envKeys` (URLs may hold none at all), because the harness expands
  `${VAR}` and `$NAME` on your machine and could send the value to the setup's author; secret flags in arguments are
  refused. Machine paths in arguments and URLs are left out on export and, as free-form text, not checked on import:
  read them in the review.
- `version` fields must be semver-like (an item with a bad one is invalid); `app` and `createdAt` (`YYYY-MM-DD`, month
  01 to 12, day 01 to 31) are dropped when they are not.

An invalid item is marked and never applied; one bad item does not stop the others. Unknown harnesses, kinds and fields
are ignored and only counted. Secret and machine-path detection in free-form arguments are heuristics, not proofs: the
review lists the exact command line or `mcp.json` entry of every item so you can read what would run.

**What an import can do.** Add only, in the user scope: `omp plugin marketplace add`, `omp plugin install --scope user`,
`pi install` (no `--local`), and an atomic merge of MCP entries into the harness's user `mcp.json`. It never uninstalls,
upgrades, downgrades, overwrites or touches project scope, preferences or harness settings. Commands are built in Rust
from validated fields and go through `cli_target` and the harness's own `cli_action`; the webview sends only a plan id,
item ids and `confirmed`. The command (`sync_import_apply`) fails with `invalidInput`, running and writing nothing,
without `confirmed: true`, a valid plan id, and a selection of selectable items of that plan. The plan lives in memory
until an apply for it starts or a new review replaces it. An attempt that is unconfirmed, made while another import is
running, or made with another plan id leaves the plan in place; once the plan id matches it is consumed, whether the
apply then succeeds, fails or refuses the selection. One apply runs at a time. If the harness binary lookup fails, the
whole import fails with that error and nothing runs; a harness that is not installed only skips its CLI items.

**MCP servers.** A stdio server is arbitrary code that the harness will start. In the review it carries a red notice and
is not ticked by default, and the app never starts it. An http or sse server runs no program, but it lets a remote
service offer tools to your agent; it is ticked by default like other items, so check its URL. The entry written holds
only `command`/`args` or `type`/`url`; a server that declares env or header keys is written with `"enabled": false`, so
nothing runs until you add the values and enable it. The file is the harness's user `mcp.json` from its own paths,
never a path from the bundle. The merge is gated by the plan's installed state: a harness that was not installed at
review time has no file, so its servers are skipped.

**Files and network.** Written: the user `mcp.json` merge above, and an exported file, which only a native save dialog
can name. Read: a setup file, which only a native open dialog can name, through `SafeReader` (256 KiB). The app opens no
network connection for sharing; the harness CLIs fetch what the plugin, marketplace and package commands name, as in
your terminal. Logs record counts and outcomes only, never sources, commands or contents. The result screen shows each
CLI's redacted summary line.

**Unsigned.** Bundles carry no signature or author. The `hud1:` string's zlib checksum (Adler-32) catches corruption,
not tampering. Import a setup only from someone you trust and read the review as you would a script (see
[Limits](#limits)). Installing a marketplace, plugin or package runs the harness CLI, whose npm or git installs may run
install scripts; the review shows each command.

## Tauri ACL, CSP and webview

- **Capability** (`src-tauri/capabilities/main.json`, window `main`): 56 `allow-<command>` permissions, one per app
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
| `mcp.json` of a harness | `<agent dir>/mcp.json` (for example `~/.omp/agent/mcp.json`), the harness's own file | Written only after you choose it: by Recommended setup (one `harness-dashboard` server) or by importing a shared setup (the MCP servers you ticked), merged in (see [Process spawning](#process-spawning)). Not app data; nothing else in it is touched. |
| Exported setup | A path you choose in the native save dialog (suggested `harness-setup-<date>.json`) | The share file of [Shared setups](#shared-setups): references and key names, no env or header values. Written atomically, only on Save. Not app data; the app never reads it back unless you import it. |
| Log files | The OS log directory | See [Logging](#logging). Rotated at 1 MiB, 3 files kept. |

`AppPrefs` (`business/src/model/prefs.rs`) has nineteen fields and never holds secrets or chat content:
`version`, `theme`, `motion`, `language` (`en`, `de`, `es`, `fr` or `pl`; anything else loads as `en`), `introEnabled`,
`soundEnabled`, `showThinking`, `defaultHarness`, `activeProject`, `rememberRecentProjects`, `recentProjects` (12 at
most), `updatePolicy`, `checkCompanionUpdates`, `autoUpdateExtensions`, `openAnyLink`, `onboardingCompleted`, `approvalNoticeSeen`,
`binaryOverrides`, `chatDefaults`.

**Server-managed fields** (`business/src/prefs/managed.rs`): `activeProject`, `recentProjects` and `binaryOverrides`
decide which directories are read and which programs run. Only Rust changes them, through the native pickers and the
project and harness commands. `prefs_set` stores the UI-managed fields only (`theme`, `motion`, `language`,
`introEnabled`, `soundEnabled`, `showThinking`, `defaultHarness`, `rememberRecentProjects`, `updatePolicy`,
`checkCompanionUpdates`, `autoUpdateExtensions`, `openAnyLink`, `onboardingCompleted`, `approvalNoticeSeen`, `chatDefaults`) and keeps the
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
- **The dashboard installer is remote code.** Once you confirm, it runs a script fetched from the `main` branch of
  `MSpiechowicz/harness-useful-dashboard` with your privileges, as if you ran the install command in your terminal. The
  app confirms first and lists what it changes, but does not pin or verify the script. To avoid it, skip the step and
  install the dashboard yourself.
- **Shared setups are unsigned.** A `hud1:` string or file has no author or signature; the checksum only detects
  corruption. An imported marketplace, plugin or package is installed by the harness CLI, which runs with your
  privileges and may execute code from it, and an imported stdio MCP server is a program the harness will run. The app
  validates what it can, shows the exact steps and asks first, but cannot judge whether the code behind a source is
  safe. Import only setups you trust.
- **Heuristics.** The entropy rule and key-name rules can hide harmless values and can miss unusual secrets. The same
  holds for Share setup: secret flags and machine paths in free-form MCP arguments and URLs are detected by patterns,
  so an exotic form can be missed on export, and host names are always part of what is shared.
- **Unsigned installers** on macOS and Windows show an OS warning on first install.
