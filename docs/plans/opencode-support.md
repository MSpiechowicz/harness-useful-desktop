# OpenCode support — plan and research (resumable)

Status: **levels 1, 2 and 4 are implemented (2026-10-16). Level 3 (chat) is implemented over OpenCode's native
server API, not ACP (2026-10-19, issue #22, branch `feat/opencode-chat`): see [opencode-native-chat.md](opencode-native-chat.md).**
The ACP sections below are kept as research for the generic ACP engine (#27).
Detection, the redacted config view, extensions (including the new Agents category), sessions through the CLI, the
Recommended setup MCP entries and the chat (marked "Preview" in the UI) work. The decisions taken are recorded in
[Decisions taken](#decisions-taken), what research got wrong in [Corrected facts](#corrected-facts). The rest of this
document is the original research (2026-10-09, OpenCode **v1.18.35**, released 2026-10-06, docs at opencode.ai/docs
dated 2026-10-08, source `github.com/anomalyco/opencode`, formerly `sst/opencode`, default branch `dev`), kept for
level 3 and as a reference. Where it disagrees with the two sections named above, they win.

OpenCode moves fast. Before resuming level 3, re-check the version, the ACP method list and the paths below against the
current release. The implementation follows the 1.19.3 sources and replays recorded CLI output in its tests
(`business/tests/fixtures/sessions/opencode/`).

## Implemented

| Level | State | Where |
| --- | --- | --- |
| 1 Detection, config, gate | Done | `business/src/harnesses/opencode/{mod,paths,config,catalog}.rs`, `pi_family/jsonc.rs`, `process/resolve.rs` (`~/.opencode/bin`), `env.rs`, `security/{deny,rules}.rs` |
| 2 Sessions | Done | `harnesses/opencode/sessions.rs`, `process/blocking.rs`, `src-tauri/src/commands/sessions.rs` |
| 3 Chat | Done (Preview) | Native server API (`opencode serve`), see [opencode-native-chat.md](opencode-native-chat.md) |
| 4 Extensions | Done | Reading in `harnesses/opencode/extensions*.rs`. Install runs `opencode plugin <module> -g` (`opencode/cli.rs`), removal edits the global config (`opencode/plugin_remove.rs`, through `Harness::config_action`). `capabilities.cliActions` is true |
| Harness Skills install | Done | `SkillsStrategy::Installer`: the Harness Skills' `install.sh` / `install.ps1` from Recommended setup (`ecosystem/installer.rs`, `ecosystem/skills_record.rs`) |
| Share setup | Done | Plugins and MCP servers (`opencode/share.rs`, `sync/collect_opencode.rs`, `sync/validate*.rs`) |

Also done: Recommended setup registers the dashboard and memory MCP servers in OpenCode's global config
(`McpTarget::OpenCodeConfig`, JSONC-aware, a file with comments gives `McpState::ManualEdit` with a snippet to
paste), the Agents extensions tab, the OpenCode onboarding card with an allow-by-default notice, and the chat gate in the UI.
Also done: Harness Skills install from Recommended setup (needs Node.js 22 or newer, installed when the skills'
install record holds a version), Extensions install and remove of npm plugins (a config with comments, a
`[spec, options]` entry or a plugin another file defines is left to the user with a note), and Share setup of npm
plugins and MCP servers.
Not done: plugin updates (`opencode plugin -f`), project-scope plugin installs, `[spec, options]` entries in Share
setup and removal, reading `opencode.db` or `storage/`. Untested: the installer and plugin actions on Windows, and a real
`opencode plugin` run (the argv follows `opencode plugin --help` of 1.19.3). The checklist for a harness like this is in
[adding-a-harness.md](../adding-a-harness.md).

## Decisions taken

The six open decisions of the research, answered:

1. **Scope.** Levels 1, 2 and 4 first, chat later (done in level 3 over the native server API, not ACP).
2. **Auto-update.** Every OpenCode spawn carries `OPENCODE_DISABLE_AUTOUPDATE=1` (`Harness::spawn_env`): the version
   probe, session list, export and delete, and any chat launch later.
3. **Inherited `OPENCODE_PERMISSION` / `OPENCODE_CONFIG_CONTENT`.** Kept, not stripped. The app records only whether
   they are set (`PRESENCE_VARS` in `env.rs`, never the values) and the config view says so in a notice.
4. **Onboarding.** An allow-by-default notice for OpenCode when it is the daily driver, like the OMP approval step. The
   app never changes OpenCode's permissions.
5. **`~/.opencode/`.** Shown as a config layer (and read for skills, agents, commands and plugins), as OpenCode reads it.
6. **Agents.** A new extensions category (`ExtensionsView.agents`, `AgentInfo`, `Capabilities.agents`), not config view
   only.

Further decisions: `supported_versions` is `1.18.` and `1.19.` (since the OpenCode 2 work also `2.1.`, see
[OpenCode 2 (v2 column)](#opencode-2-v2-column)). The chat gate is a new
`Capabilities.chat` flag, next to `agents` and `cliActions`. Sessions are linear (`session_tree` false: no resume or
fork). Sessions come only from the CLI, so a new `Harness::cli_sessions` hook and a synchronous `BlockingRunner` exist.

## Corrected facts

Findings from building it that differ from, or add to, the research below:

- **No `scout` built-in agent in 1.19.3.** The built-ins are `build` and `plan` (primary), `general` and `explore`
  (subagents), plus the hidden `compaction`, `title` and `summary`.
- **`opencode session list` prints nothing when there are no sessions** (not `[]`). The app treats empty output as an
  empty list.
- **`export` and `session delete` find a session of any project**, and `session list` covers the whole git worktree of
  the working directory. The app keeps only sessions whose directory is the project or lies inside it, and deletes only a
  session the project's list shows.
- **OpenCode 1.19 stopped the terminal UI's auto-install of updates.** `OPENCODE_DISABLE_AUTOUPDATE=1` is still set on
  every spawn.
- **OpenCode 2 exists** (`opencode upgrade --major`), so 2.x was detected as untested, not unsupported. 2.1 is
  supported now, see [OpenCode 2 (v2 column)](#opencode-2-v2-column).
- **`--pure` on every session call** (`session list`, `export`, `session delete`): OpenCode runs them without external
  plugins.
- **Session ids** are checked (`ses_` prefix, characters safe as a CLI argument) before anything is spawned.
- **Trailing commas.** OpenCode reads its config as JSONC with trailing commas allowed. Only comments stop the app from
  rewriting a config file.

## Contents

- [Implemented](#implemented)
- [Decisions taken](#decisions-taken)
- [Corrected facts](#corrected-facts)
- [OpenCode 2 (v2 column)](#opencode-2-v2-column)
- [Proposed scope ladder](#proposed-scope-ladder)
- [Integration points in this repo](#integration-points-in-this-repo)
- [OpenCode facts](#opencode-facts)
- [ACP findings (chat protocol)](#acp-findings-chat-protocol)
- [Engine changes ACP needs](#engine-changes-acp-needs)
- [Security: deny-list and redaction](#security-deny-list-and-redaction)
- [Never call](#never-call)
- [Open decisions](#open-decisions)
- [Open questions](#open-questions)

## OpenCode 2 (v2 column)

OpenCode 2 (2.1.x, 2026-10) is the same harness in the app. Plan, steps and status: [opencode-v2.md](opencode-v2.md).
What differs from the 1.18 / 1.19 behaviour described in this file (the research below describes OpenCode 1):

| Topic | OpenCode 1 (1.18, 1.19) | OpenCode 2 (2.1) |
| --- | --- | --- |
| Install | curl script (npm `opencode-ai` on Windows) | `npm install -g @opencode/cli` everywhere. The install script refuses 2.x. The package also has an `opencode2` bin, which the app ignores |
| Detection | `--version` prints `1.19.3` | prints `opencode v2.1.2`. The version selects the generation (`OpenCodeGeneration`: major 1 and unknown versions are V1, major 2 and later V2), set by `Harness::note_version` and stale until the next detection |
| Spawn env | `OPENCODE_DISABLE_AUTOUPDATE=1` | same, plus `OPENCODE_DISABLE_MODELS_FETCH=1` and `OPENCODE_DISABLE_FILEWATCHER=1` on session commands |
| Background service | none | `opencode serve --service`, started by many CLI commands. The app never starts it |
| CLI calls | `--pure` on session commands | `--standalone` on every call that takes it. Commands that call the service are avoided (`debug config`, `debug agents`, `plugin list`, `mcp list`) |
| Config files | `opencode.json[c]`, plus a legacy global `config.json` | `opencode.json[c]` only, no `config.json` layer |
| `OPENCODE_CONFIG_DIR` | an extra config directory, read after the others | replaces the global directory |
| `OPENCODE_PERMISSION` | read | gone, `permissions` in the config replaces it. `OPENCODE_CONFIG`, `OPENCODE_CONFIG_CONTENT`, `OPENCODE_DISABLE_PROJECT_CONFIG` still work |
| Plugins key | `plugin: [string \| [spec, options]]` | `plugins: [string \| {package, options}]`, legacy `plugin` still read |
| MCP key | `mcp.<name>` with `enabled` | `mcp.servers.<name>`, legacy `mcp.<name>` still read, no `enabled` on entries the app writes |
| Agents, commands | `agent`, `command` | `agents`, `commands`, legacy keys read. Discovery directories are unchanged |
| Plugin install | `opencode plugin <module> -g` | `opencode plugin add <module>` (global `plugins`, no service) |
| Plugin remove | the app edits the `plugin` list | the app edits `plugins` and the legacy `plugin` list (OpenCode 2's own `plugin remove` is not used) |
| Merged MCP names (Share setup collision check) | `opencode debug config` | `opencode api --standalone GET /api/config`, which starts a private server and lists the merged config documents |
| Sessions | `session list --pure --format json`, `export --pure`, `session delete --pure` | `session list --standalone --format json -n N`, `session export --standalone <id>`, `session delete --standalone <id>`. Times are Unix milliseconds, `title` may be missing, a missing session answers `Session not found: <id>` |
| Existing sessions | n/a | OpenCode 2 does not read OpenCode 1 sessions, and the app does not import them. A migrated session may still hold OpenCode 1 tool names |
| Chat server | `opencode serve`, `/session` and `/event` routes | `opencode serve --hostname 127.0.0.1 --port 0` (no `--mdns`), `/api/...` routes with `{data}` envelopes, `GET /api/info` as readiness, another event vocabulary. `ChatProtocol::OpenCodeServerV2` |
| Approval modes | rules in `permission` | rules in `agents.<name>.permissions` (`{action, resource, effect}`), through the chat server's `OPENCODE_CONFIG_CONTENT` |
| Hook tool ids | `bash`, `apply_patch`, `task`, `edit` / `write` with `filePath` | `shell`, `patch`, `subagent`, `edit` / `write` with `path`. See [hooks-format.md](../hooks-format.md#matcher) |

Unchanged on OpenCode 2: one private server per chat on 127.0.0.1 with a random per-launch password, the Basic
auth user `opencode`, never changing OpenCode's config files from a chat, never passing `--auto`, and never calling
share, auth, upgrade or config-write routes. The Harness Skills plugin has a V2 entry (`harness-useful-skills`,
`opencode/v2.js`) next to the V1 one.

## Proposed scope ladder

| Level | Scope | Effort | Risk |
| --- | --- | --- | --- |
| 1 | Detection (incl. `~/.opencode/bin`), wizard card + install hint, read-only redacted config viewer, deny-list additions, generalized approval step, "chat coming soon" gate | S–M (1–2 days) | low |
| 2 | Sessions list + transcript via `opencode session list --format json` and `opencode export <id>` (never the DB) | M | medium |
| 3 | Chat over ACP (`opencode acp`): engine work below, approvals mapped to the existing approval cards, model/agent pickers from `configOptions`, resume via `session/load` | L (1–2 weeks) | high |
| 4 | Extensions: commands, agents, skills, plugins, MCP, themes (read); actions only `opencode plugin <module> [-g]` (no dry-run) | M | low read / medium actions |

Recommendation: ship level 1 first, then 2–4. ACP work (level 3) is reusable for other ACP agents later
(Gemini CLI, Zed's ACP adapters for Claude Code and Codex).

### Level 1 details

- `HarnessId::OpenCode` (wire `"opencode"`), binary `opencode`, `--version`/`-v` prints a bare `1.18.35`.
- Install hint: `curl -fsSL https://opencode.ai/install | bash` or `npm install -g opencode-ai`; docs
  `https://opencode.ai/docs/`.
- Config view: JSONC parsing (no JSONC parser in `business/` today; add one or a comment/trailing-comma stripper);
  layers global + `.opencode/` dirs + project file; show `{env:VAR}` / `{file:path}` substitutions as names only and
  never follow `{file:...}`.
- `supported_versions = &["1.18."]` (high release velocity).
- Chat gate: either a new `chat: bool` capability the UI checks, or `launch_spec` returning `Unsupported` (verify the
  UI handles it).
- Caveat: **any** OpenCode invocation (even `--version`) creates its XDG directories, so `config_dir_exists` is
  meaningless after the first detection probe. The binary is ~185 MB (Bun-compiled); watch the 5 s probe timeout on a
  cold start.

## Integration points in this repo

Follow `docs/adding-a-harness.md`. Files and symbols:

- **Model:** `business/src/model/harness.rs` — `HarnessId::OpenCode`, extend `ALL`, `binary_name`. Regenerate TS with
  `cargo test -p harness_business export_`. `Capabilities` has no chat flag yet (see level 1). `ExtensionsView`
  (`business/src/model/extensions.rs`) has no "agents" category.
- **Adapter:** new `business/src/harnesses/opencode/{mod,paths,config,extensions,catalog,launch,dialect,sessions,cli}.rs`;
  register in `business/src/harnesses/mod.rs` and `HarnessRegistry::with_defaults`
  (`business/src/harness/registry.rs`). All five session methods must be overridden (not PI-family).
- **Binary resolution:** `business/src/process/resolve.rs::extra_search_dirs` lacks `~/.opencode/bin` (curl installer
  target; the installer edits shell rc files the app never reads). Arch (`/usr/bin`) and npm/bun globals are covered.
- **Env:** `business/src/env.rs::CAPTURED_VARS` — add `XDG_STATE_HOME`, `XDG_CACHE_HOME`, `OPENCODE_CONFIG`,
  `OPENCODE_CONFIG_DIR`, maybe `OPENCODE_DISABLE_PROJECT_CONFIG` (`XDG_DATA_HOME`/`XDG_CONFIG_HOME` already there).
- **Redaction:** `business/src/security/rules.rs` `SECRET_CONTAINERS = ["env","headers"]` must gain `environment`
  (OpenCode MCP local servers), or MCP env secrets leak. Run MCP `command` arrays through `redact_text`.
- **MCP parsing:** OpenCode uses `mcp.<name> = {type:"local", command:[...], environment, enabled, timeout}` or
  `{type:"remote", url, headers, oauth:{clientId,clientSecret,scope}|false, enabled, timeout}` — not the
  `mcpServers` shape `pi_family::parse_servers` expects.
- **Engine (level 3 only):** done without ACP: `business/src/agent/opencode/` (the session engine),
  `business/src/opencode_server/` (HTTP and event-stream client, route allowlist, event mapper),
  `business/src/agent/engine.rs` (the seam next to the RPC `agent/session.rs`) and `business/src/bin/fake_agent/opencode.rs`
  (`--dialect opencode`). The original pointers were `agent/session.rs` and `pi_family/rpc/`, which stay the OMP and PI engine.
- **UI:**
  - colours and badge: `ui/src/lib/components/Badge.svelte` (`HARNESSES` record), `ui/src/lib/design/tokens.css`
    (`--harness-opencode`, `--harness-opencode-wash`, both themes), `ui/src/app.css` (`--color-opencode*`);
  - mocks: `ui/src/lib/ipc/mock/fixtures.ts`, `business/tests/mock_fixtures.rs` (`harness_list.json`,
    `project_recent.json`), omp/pi ternaries in `ui/src/lib/ipc/mock/extension-action-handlers.ts` and
    `agent-handlers.ts`;
  - onboarding copy that names only OMP and PI: `HarnessesStep.svelte`, `DoneStep.svelte`, `WelcomeStep.svelte`;
    the approval step and its gate (`onboarding.svelte.ts`, `this.driver === 'omp'`) are OMP-only and need
    generalizing (OpenCode is allow-by-default); `HarnessCard.svelte` is data-driven;
  - other copy: `src-tauri/tauri.conf.json` descriptions, README, `ui/src/features/shell/views.ts` empty states.
- **Tests:** `business/tests/{detection,harness_paths,session_paths,canary,adapter_canary,safe_reader,catalog,rpc_dialects,cli_actions,mock_fixtures}.rs`,
  a fixture home `business/tests/fixtures/opencode-home/`, `ui/src/features/onboarding/onboarding.test.ts`.

## OpenCode facts

### Install and binary

- `curl -fsSL https://opencode.ai/install | bash` → `$HOME/.opencode/bin` (appends PATH to rc files, incl.
  `fish_add_path`); `npm|bun|pnpm install -g opencode-ai`, `yarn global add opencode-ai`;
  `brew install anomalyco/tap/opencode`; `sudo pacman -S opencode` / `paru -S opencode-bin`;
  `choco`/`scoop install opencode`; `mise use -g github:anomalyco/opencode`; Docker `ghcr.io/anomalyco/opencode`.
- Docs banner mentions "OpenCode v2" (`opencode.ai/v2`, empty at research time); npm still 1.18.35.

### Paths (`packages/core/src/global.ts`, xdg-basedir)

| What | Path |
| --- | --- |
| data | `$XDG_DATA_HOME/opencode` (`~/.local/share/opencode`) |
| config | `$XDG_CONFIG_HOME/opencode` (`~/.config/opencode`), override `OPENCODE_CONFIG_DIR` |
| state | `~/.local/state/opencode` |
| cache | `~/.cache/opencode` (bin at `~/.cache/opencode/bin`, npm plugins in `node_modules/`) |
| log | `<data>/log` (timestamped `*.log`, last 10) |
| repos | `<data>/repos` |
| tmp | `$TMPDIR/opencode` |
| Windows | `%USERPROFILE%\.local\share\opencode`, `%USERPROFILE%\.cache\opencode` |

### Config (opencode.ai/docs/config, `packages/opencode/src/config/paths.ts`)

- JSON or JSONC. Global `~/.config/opencode/opencode.json[c]`; project `opencode.json[c]` walking up from cwd to the
  worktree root.
- Config dirs in order: global dir; every `.opencode/` from cwd up to the worktree (skipped with
  `OPENCODE_DISABLE_PROJECT_CONFIG`); `~/.opencode/` (same dir as the curl installer's `bin/`); `OPENCODE_CONFIG_DIR`.
- Env: `OPENCODE_CONFIG` (file), `OPENCODE_CONFIG_CONTENT` (inline), `OPENCODE_TUI_CONFIG`.
- Managed: `/etc/opencode/` (Linux), `/Library/Application Support/opencode/` (macOS), `%ProgramData%\opencode`.
- Precedence (low→high, merged): remote `.well-known/opencode` → global → `OPENCODE_CONFIG` → project → `.opencode`
  dirs → inline → managed → macOS MDM.
- Keys: `$schema` (`https://opencode.ai/config.json`), `model`, `small_model`, `provider`, `agent`, `default_agent`,
  `subagent_depth`, `tools` (deprecated since 1.1.1 → `permission`), `permission`, `mcp`, `plugin`, `command`,
  `theme` (in `tui.json`, schema `https://opencode.ai/tui.json`), `server`, `shell`, `share`, `instructions`,
  `autoupdate`, `snapshot`, `formatter`, `lsp`, `watcher`, `compaction`, `attachment.image`, `disabled_providers`,
  `enabled_providers`, `experimental`.
- Secrets can appear in `provider.<id>.options.apiKey` (literal, `{env:X}`, `{file:~/x}`), `mcp.<n>.environment`,
  `mcp.<n>.headers`, `mcp.<n>.oauth.clientSecret`, `mcp.<n>.command` args. Server password only via env
  (`OPENCODE_SERVER_PASSWORD`).

### Credentials

- `<data>/auth.json` (mode 0600, `packages/opencode/src/auth/index.ts`), `<data>/mcp-auth.json` (MCP OAuth), and a
  `Credential` table inside the SQLite DB (`packages/core/src/credential.ts`).

### Sessions (`packages/core/src/database/database.ts`)

- SQLite `<data>/opencode.db` (or `opencode-<channel>.db`; override `OPENCODE_DB`). The DB also holds credentials —
  **never read it**.
- Legacy JSON `<data>/storage/{project,session/<projectID>,message/<sessionID>,part/<messageID>,session_diff}/*.json`,
  migrated once into the DB (marker `storage/migration`).
- Linear sessions; fork (`POST /session/:id/fork`, ACP `unstable_forkSession`) creates a new session; `parentID` marks
  child/subagent sessions; `revert`. Not a PI-style entry tree → `session_tree: false`.
- CLI: `opencode session list --format json [-n N]` (root sessions of the cwd's project:
  `[{id,title,updated,created,projectId,directory}]`), `opencode export <sessionID> [--sanitize]`
  (`{info, messages:[...parts]}`), `opencode db path`. Ids look like `ses_…` (check against `is_safe_session_id`).

### Permissions (opencode.ai/docs/permissions)

- Keys: `read`, `edit`, `glob`, `grep`, `bash`, `task`, `skill`, `lsp`, `question`, `webfetch`, `websearch`,
  `external_directory`, `doom_loop`, `"*"`. Values `allow` / `ask` / `deny`; objects map glob patterns (last match
  wins). Agent-level `permission` overrides global.
- **Defaults: mostly `allow`**; `doom_loop` and `external_directory` default to `ask`; `.env`/`.env.*` reads denied;
  the Plan agent asks for edit and bash. Answers: once / always (session) / reject. `--auto` approves everything not
  denied. Env override `OPENCODE_PERMISSION`.

### Extensions

- **Commands:** `{command,commands}/**/*.md` in config dirs or the `command` key; frontmatter `description`, `agent`,
  `model`, `subtask`; placeholders `$ARGUMENTS`, `$1..$N`, `` !`cmd` ``, `@file`; built-ins `/init`, `/undo`,
  `/redo`, `/share`, `/help`.
- **Agents:** `~/.config/opencode/agents/`, `.opencode/agents/`, or the `agent` key; keys `description`, `mode`
  (primary/subagent/all), `model`, `prompt`, `temperature`, `permission`, `steps`, `disable`, `hidden`, `color`.
  Built-ins: build, plan (primary); general, explore (subagents; research listed a `scout` too, 1.19.3 has none); hidden
  compaction, title, summary.
- **Skills:** `<name>/SKILL.md` under `.opencode/skills`, `~/.config/opencode/skills`, `.claude/skills`,
  `~/.claude/skills`, `.agents/skills`, `~/.agents/skills` (walking up to the worktree root).
- **Plugins:** local `.opencode/plugins/`, `~/.config/opencode/plugins/` (.js/.ts) or npm via `plugin`; installed with
  Bun at startup into `~/.cache/opencode/node_modules/`; a `package.json` in a config dir triggers `bun install` at
  startup — **any launch can hit the network**.
- **Themes:** `~/.config/opencode/themes/*.json`, `<project>/.opencode/themes/*.json`.
- **Mutating CLI:** `opencode plugin <module> [-g] [-f]` (installs and edits config, no dry-run) is the only
  candidate for `cli_action`; `mcp add` and `agent create` are interactive.

## ACP findings (chat protocol)

**Recommendation: chat over ACP — `opencode acp [--cwd DIR] [--port 0] [--hostname 127.0.0.1]`**
(opencode.ai/docs/acp, `packages/opencode/src/cli/cmd/acp.ts`, `src/acp/*`).

Why not the others:

- `opencode run [msg] --format json` is one-shot with no request ids, and in non-auto mode it **auto-rejects every
  permission request** (`run.ts` ~801–818) — it cannot carry approvals.
- `opencode serve` (HTTP + SSE, OpenAPI at `/doc`, basic auth via `OPENCODE_SERVER_PASSWORD`/`_USERNAME`,
  SDK `@opencode-ai/sdk`) would need an HTTP client, SSE parsing, port/password management and a long-lived server —
  a much bigger engine fork. It could later back read-only views.

ACP facts:

- Transport: stdio, ndjson **JSON-RPC 2.0** (`@agentclientprotocol/sdk` `ndJsonStream`). Internally `opencode acp`
  also starts its own HTTP server on 127.0.0.1 (random port) and drives it with SDK v2; it is unauthenticated unless
  `OPENCODE_SERVER_PASSWORD` is set.
- `initialize` → `protocolVersion: 1`, `agentCapabilities { loadSession: true, mcpCapabilities {http, sse},
  promptCapabilities {embeddedContext, image}, sessionCapabilities {close, fork, list, resume} }`,
  `authMethods: [{ id: "opencode-login" }]` (with client `_meta["terminal-auth"]` it offers `args: ["auth","login"]` —
  **never use**), `agentInfo { name: "OpenCode", version }`.
- Client → agent methods: `session/new`, `session/load`, `session/list`, `session/resume`, `session/close`,
  `unstable_forkSession`, `session/set_config_option` (ids `model`, `mode`, effort/variant), `session/set_mode`,
  `unstable_setSessionModel`, `session/prompt` (**resolves only at turn end** with `stopReason`: `end_turn` |
  `cancelled` | `max_tokens` | `refusal`), `session/cancel` (**notification**, no reply).
- Agent → client stream: `session/update` notifications — `agent_message_chunk`, `agent_thought_chunk`, `tool_call`,
  `tool_call_update`, `usage_update`, `config_option_update`, `available_commands_update` (`{name, description}`).
- Approvals: the agent sends a **request** `session/request_permission` with options `once` (allow_once),
  `always` (allow_always), `reject` (reject_once). The client answers
  `{ outcome: { outcome: "selected", optionId } }`; any error or other outcome means reject.
- On an `edit` approval OpenCode also tries the client's `fs/write_text_file` (errors swallowed) — the client must
  answer unknown agent → client methods (`fs/*`, `terminal/*`) with a JSON-RPC error (-32601) and advertise no `fs` /
  `terminal` capabilities.
- `session/load` replays the transcript as `session/update` notifications, not as a response.
- Docs: `/undo` and `/redo` are unsupported in ACP.

## Engine changes ACP needs

1. **JSON-RPC 2.0 framing** (`{jsonrpc, id, method, params}`; responses `{id, result|error}` without `type`). Map to
   `Inbound::Response` in the dialect; `codec.rs::fault` only extracts a response id when `type == "response"`.
   Precedent: `business/src/ecosystem/mcp_client.rs`, `fake_agent --dialect mcp`.
2. **Handshake:** new `Handshake` variant — `initialize`, then `session/new` or `session/load` — replacing the
   `get_state` probe; `session.rs::startup` expects state from `get_state`, so build a synthetic state from the
   `session/new` reply (modes, `configOptions`/models).
3. **Per-session protocol state:** every request carries the ACP `sessionId`, but dialects are stateless `&'static` and
   `encode` only gets `EncodeContext` — that state must reach the encoder.
4. **Long-lived prompt:** `session/prompt` resolves at turn end (PI acks immediately); `pending.rs::timeout_for` and
   `on_prompt_ack` need a "runs until `stopReason`" kind.
5. **Outgoing notifications:** abort is `session/cancel` with no id and no reply — fire-and-forget frames.
6. **Agent → client requests:** `session/request_permission` → existing `UiRequest` select (Allow once / Always allow /
   Reject); `encode_ui_response` → `{jsonrpc, id, result: {outcome: {outcome: "selected", optionId}}}`; cancel →
   `{outcome: "cancelled"}`; unknown methods → `AutoReply` JSON-RPC error -32601.
7. **History:** `history_request` needs a "replay" mode (`session/load` streams notifications).
8. **Live data:** commands from `available_commands_update` → `Inbound::Commands`; models/agents from
   `configOptions`; `SetModel`/mode via `session/set_config_option`; no `get_session_stats` (only `usage_update`), no
   bash frame, no `ExportHtml`/`Clone` → `Unsupported`; `/compact` and `/new` map to prompt text or `session/new`;
   `slash_commands_via_prompt: true`.
9. **Launch hardening:** random per-launch `OPENCODE_SERVER_PASSWORD` in `ExecSpec.env` (ACP opens a localhost HTTP
   server); consider `OPENCODE_DISABLE_AUTOUPDATE=1`; decide whether to strip inherited `OPENCODE_PERMISSION` and
   `OPENCODE_CONFIG_CONTENT`; extend `scripts/check-forbidden.ts` with `--auto`, `--yolo`,
   `--dangerously-skip-permissions` and the terminal-auth `auth login` args.
10. **Fake agent:** `acp` dialect in `business/src/bin/fake_agent.rs` (JSON-RPC like `mcp`, plus agent-initiated
    requests and notifications) and scenario fixtures.

## Security: deny-list and redaction

- Already denied by name/extension: `auth.json`, `mcp-auth.json`, `*.db` (incl. `opencode.db`,
  `opencode-<channel>.db`), `*.db-wal`, `*.db-shm`, `*.log`.
- Add to `DENIED_DIRS` (scoped to an `opencode` ancestor, like the existing `agent`/`blobs` rule, because the names
  are generic): `storage` (legacy transcripts and tool output), `repos` (cached git repos), `snapshot` (project git
  snapshots), and `node_modules` under `~/.cache/opencode`.
- Consider denying desktop-app state `opencode.settings.dat`, `opencode.global.dat`, `opencode.workspace.*.dat`.
- Never follow `{file:...}` substitution targets; never read the file named by `OPENCODE_DB`.
- Redaction: add `environment` to `SECRET_CONTAINERS`; `apikey`, `clientsecret`, `authorization`, `oauth` are already
  covered; MCP `command` args through `redact_text`.
- Canary fixture must hide the marker in `auth.json`, `mcp-auth.json`, `opencode.db`, `provider.x.options.apiKey`,
  `mcp.x.environment.*`, `mcp.x.headers.Authorization`, `mcp.x.oauth.clientSecret` and an MCP command arg; no view may
  contain it.

## Never call

`auth login`, `providers login` (alias `auth`), `mcp auth`, `mcp logout`, `auth logout`, `account`, `opencode db`
(raw SQL shell), `mcp list` / `mcp debug` (they spawn MCP servers), `export` without an id (may prompt). Never pass
`--auto`, `--yolo` or `--dangerously-skip-permissions`.

## Open decisions

All six were decided on 2026-10-16, see [Decisions taken](#decisions-taken). The original questions:

1. Scope: level 1 now and the rest later (recommended), or straight through to chat.
2. Auto-update: run OpenCode with `OPENCODE_DISABLE_AUTOUPDATE=1` (recommended).
3. Inherited `OPENCODE_PERMISSION` / `OPENCODE_CONFIG_CONTENT`: strip or keep.
4. Onboarding: how to present OpenCode's allow-by-default permissions (the analogue of OMP "yolo").
5. Show `~/.opencode/` as a config layer (it doubles as the installer's `bin/` dir)?
6. Agents: a new extensions category (model change) or config view only.

## Open questions

1. What "OpenCode v2" changes (repo has `specs/v2`, `sdk/v2`, `event-v2-bridge`) — API, storage or ACP may shift.
2. Do `session list`, `export` and `acp` trigger plugin `bun install`, models.dev fetches
   (`OPENCODE_DISABLE_MODELS_FETCH`) or autoupdate at startup? Unverified (testing would mutate state).
3. Legacy global `config.json` support; singular dir names for agents/plugins (commands accept both).
4. Slash commands in ACP prompt text (assumed parsed server-side).
5. Exact `session/new` response (models vs `configOptions`) and the thinking/variant option id — record a real session.
6. Can ACP `session/list` replace CLI listing for level 2 without starting a model session?
7. Session id charset vs `is_safe_session_id`.

## Local machine notes (2026-10-09)

- OpenCode 1.18.35 is installed at `~/.opencode/bin/opencode` (curl installer); not on the app's search path.
- Running `--version` during research created empty dirs: `~/.config/opencode/`, `~/.local/share/opencode/{log,repos}/`,
  `~/.local/state/opencode/`, `~/.cache/opencode/bin/`, `/tmp/opencode/` (no files, no credentials).
