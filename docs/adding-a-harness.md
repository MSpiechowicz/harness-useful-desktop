# Adding a harness

A harness is one coding-agent CLI. The app supports OMP, PI, OpenCode, Claude Code and Codex today. This guide adds a new one, called
`acme` (binary `acme`) in the examples. The UI renders from `HarnessInfo`, `Capabilities` and the views an adapter
returns, so most of the work is Rust in `business/`. OMP and PI are the references for a harness the app chats with
(PI-family files and RPC). OpenCode (`business/src/harnesses/opencode/`) is the reference for a harness with another
chat protocol (its own server, see [Other chat protocols](#other-chat-protocols)) and whose saved sessions are only
reachable through its CLI. Claude Code (`business/src/harnesses/claude/`) is the reference for a stdio engine with a
protocol of its own (`ChatProtocol::ClaudeStream`, no server, no password), for session files in another format and for
a plugin marketplace plus an MCP file that only the CLI may change. Codex (`business/src/harnesses/codex/`) is the
reference for an `app-server` style engine (JSON-RPC over stdio, `ChatProtocol::CodexAppServer`), for saved sessions
read through a short-lived helper process of the CLI, for a TOML config the app only reads and for the lessons in
[Lessons from Codex](#lessons-from-codex).

**Fit check.** The chat engine drives a harness over stdio with newline-delimited JSON requests that carry an `id`,
answered by `response` frames (`pi_family::rpc`). A CLI that can be driven that way fits, even when its frames
differ: the `RpcDialect` decides every frame it sends and decodes. A CLI with another protocol (for example a
stream of events with no request ids on the user's messages) needs a chat engine of its own. OpenCode has one over HTTP
and Claude Code has one over stdio (see [Other chat protocols](#other-chat-protocols)): Claude Code has requests with
ids (the control protocol), but its user messages carry none and a turn ends with a `result` frame instead of a
response, so it does not fit `RpcDialect`. Codex has a JSON-RPC protocol over stdio (`codex app-server`) with ids on
requests, responses and server requests, but no `type` field on its frames, a handshake of its own and per-method
results, so it does not fit `RpcDialect` either and has an engine of its own.

A CLI without a fitting protocol can still ship with `capabilities.chat` false. Detection, the configuration and
extension views, sessions and Recommended setup work as for any harness. `launch_spec` returns
`BusinessError::Unsupported`, `dialect` returns any dialect (it is never reached) and the UI says "Chat coming soon"
wherever a chat would start. The chat steps below ([Commands](#5-commands) for the shared slash commands,
[RPC launch and dialect](#6-rpc-launch-and-dialect)) then shrink to those stubs, and the catalog lists every command as
unsupported.

## Steps

1. [Add a `HarnessId`](#1-harnessid)
2. [Create the adapter and register it](#2-adapter-and-registry)
3. [Detection](#3-detection)
4. [Config and extension views](#4-config-and-extension-views)
5. [Commands](#5-commands)
6. [RPC launch and dialect](#6-rpc-launch-and-dialect)
7. [Sessions](#7-sessions)
8. [CLI actions](#8-cli-actions)
9. [Recommended setup](#9-recommended-setup)
10. [Share setup](#10-share-setup)
11. [Tests](#11-tests)
12. [UI touch points](#12-ui-touch-points)

## 1. HarnessId

`business/src/model/harness.rs`. The variant serializes lowercase, so `Acme` becomes `"acme"` in JSON and in the
generated `HarnessId.ts`.

```rust
pub enum HarnessId { Omp, Pi, OpenCode, Claude }
pub const ALL: [HarnessId; 4] = [HarnessId::Omp, HarnessId::Pi, HarnessId::OpenCode, HarnessId::Claude];
pub fn binary_name(self) -> &'static str { /* "omp", "pi", "opencode", "claude" */ }
```

Add the variant, extend `ALL` (display order) and `binary_name`. The variant serializes lowercase, so `OpenCode` is
`"opencode"`. The compiler flags every other exhaustive `match`.

## 2. Adapter and registry

Create `business/src/harnesses/acme/mod.rs` with a unit struct and `impl Harness` (a harness with much to read splits
it into submodules that `mod.rs` delegates to, as `harnesses/opencode/{paths,config,extensions,catalog,sessions,setup}.rs`
do). Start from
`use crate::harness::prelude::*;`, which brings in `Harness`, `HarnessPaths`, `LaunchRequest`, `CommandCatalog`,
`RpcDialect`, `Env`, `ExecSpec` and the model types an adapter needs.

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct Acme;
```

Register it in two places:

- `business/src/harnesses/mod.rs`: `pub mod acme;` and `pub use acme::Acme;`
- `HarnessRegistry::with_defaults` in `business/src/harness/registry.rs`:

```rust
Self::new(vec![Box::new(Omp), Box::new(Pi), Box::new(OpenCode::default()), Box::new(Claude)])   // add Box::new(Acme)
```

The registry order is the display order. The routine format and the routine MCP tool take their harness ids from
`HarnessId::ALL`, so a new id needs no change there ([routines-format.md](routines-format.md)). Nothing else enumerates harnesses by hand: the shell, the `open_external`
link allowlist (each harness's `homepage` and install `docs_url`) and the reveal roots (each harness's `paths`) all
iterate the registry.

### The `Harness` trait

`business/src/harness/mod.rs`. Required methods have no body; the rest have defaults you override only when the CLI
differs.

| Method | Required | Default | Meaning |
| --- | --- | --- | --- |
| `id()` | yes | | The `HarnessId`. |
| `info()` | yes | | `HarnessInfo`: name, short name (`"OMP"`), binary name, description, homepage. |
| `capabilities()` | yes | | `Capabilities` flags that decide which tabs and actions the UI shows. See [Capabilities](#capabilities). |
| `paths(env, project)` | yes | | `HarnessPaths`: `config_root`, `agent_dir`, `data_root`, `profile`, `project_dir`. |
| `config_view(env, project)` | yes | | Redacted `ConfigView` (user layer, then profiles and project). |
| `extensions_view(env, project)` | yes | | Plugins, skills, MCP servers, themes, ... as `ExtensionsView`. |
| `catalog()` | yes | | The `CommandCatalog`. |
| `launch_spec(request)` | yes | | The RPC command line as a shell-free `ExecSpec`. `Unsupported` when `capabilities.chat` is false. |
| `dialect()` | yes | | The `&'static dyn RpcDialect`. Never reached when `capabilities.chat` is false. |
| `sessions_dir(env, project)` | yes | | Folder holding the saved sessions of a project. |
| `install_hint()` | yes | | Copyable install command and docs URL, shown when the CLI is missing (never run). |
| `companion_setup()` | yes | | `CompanionSetup`: how Harness Skills are installed and where the dashboard and project memory MCP servers are registered. See [Recommended setup](#9-recommended-setup). |
| `binary_name()` | no | `self.id().binary_name()` | Executable name looked up on `PATH`. |
| `version_args()` | no | `["--version"]` | Arguments that print the version. |
| `parse_version(output)` | no | first `x.y.z` (`first_semver`) | Extracts the version. |
| `supported_versions()` | no | `[]` | Version prefixes tested with this app, e.g. `&["18."]`. |
| `version_support(version)` | no | `Supported` on a prefix match, else `Untested` | Never returns `Unknown`; that is detection's "no version" state. |
| `detect(env, runner, override)` | no | `harness::detect` | See [Detection](#3-detection). |
| `note_version(version)` | no | nothing | Called by `harness::detect` after the version probe with the version read (`None` when it read none or the binary is missing). For a harness whose behaviour depends on its version: OpenCode keeps an `OpenCodeGeneration` (`V1` for 1.x, `V2` for 2.x) in an atomic and every method that differs reads it. `opencode_generation()` reports it (`None` for other harnesses). Views that depend on it run `ensure_detected` first (`src-tauri/src/commands/harness.rs`). |
| `summary(...)` | no | info + capabilities + detection | What `harness_list` returns. |
| `list_sessions`, `resolve_session`, `session_tree`, `session_messages`, `session_files_touched` | no | PI-family session files | See [Sessions](#7-sessions). |
| `delete_session` | no | PI-family session files | Deletes a session file with its companion folder and backups. |
| `cli_sessions()` | no | `None` | `Some(&dyn CliSessions)` for a harness whose sessions are only reachable through its CLI. It replaces the session file methods above. See [Sessions](#7-sessions). |
| `spawn_env()` | no | none | `&'static [(&str, &str)]` set on every spawn of the binary: the version probe, CLI actions, the chat launch and CLI sessions. OpenCode sets `OPENCODE_DISABLE_AUTOUPDATE=1` so no probe updates the binary. |
| `session_folder(env, project)` | no | `sessions_dir`, not configured | Where the sessions live and whether the project's own settings chose that folder (shown when deleting). |
| `configured_approval_mode(env, project)` | no | `None` | Only for harnesses with `capabilities.approvalModes`. |
| `native_approval_mode(env, project)` | no | `None` | The harness's own permission mode for the project by its own name (`NativeApprovalMode { name, unattended }`), which the chat header shows read-only and onboarding uses to warn about an unattended default. Claude Code returns `permissions.defaultMode` (`harnesses/claude/modes.rs`). |
| `check_project_trust(env, project)` | no | `Ok(())` | Refuses a chat launch in a folder the harness itself does not trust (`BusinessError::Untrusted`). Only for harnesses with `capabilities.projectTrustRequired`, and not asked when the launch trusts the project once. Claude Code refuses a project without `hasTrustDialogAccepted` in its global file. |
| `cli_action(action, project)` | no | `Unsupported` error | See [CLI actions](#8-cli-actions). Needs `capabilities.cliActions`. |
| `cli_failure_line(stdout, stderr)` | no | `None` (exit code decides) | For CLIs that exit 0 on failure. |
| `cli_summary(stdout)` | no | `None` (the last line) | The summary of a successful action when the last line is not for people (Claude Code's `--json` result line). |

Signature examples:

```rust
fn paths(&self, env: &Env, project: Option<&Path>) -> HarnessPaths;
fn launch_spec(&self, request: &LaunchRequest<'_>) -> Result<ExecSpec>;
fn dialect(&self) -> &'static dyn RpcDialect;
fn sessions_dir(&self, env: &Env, project: &Path) -> PathBuf;
fn cli_sessions(&self) -> Option<&dyn CliSessions> { None }
fn spawn_env(&self) -> &'static [(&'static str, &'static str)] { &[] }
```

### Capabilities

`Capabilities` (`business/src/model/harness.rs`) are plain flags the UI reads. They must match what the adapter
really does. Beyond the extension kinds (`plugins`, `marketplaces`, `packages`, `mcp`, `skills`, `promptTemplates`,
`extensionModules`, `themes`) and the PI-family features (`profiles`, `approvalModes`, `projectTrust`,
`slashCommandsViaPrompt`, `sessionTree`, `moveSession`, `removeQueued`):

| Flag | Meaning when true | When false |
| --- | --- | --- |
| `chat` | The app drives chat sessions (`launch_spec` and the chat protocol work). OMP, PI, OpenCode and Claude Code. | The UI shows "Chat coming soon" instead of the chat, the harness picker marks the harness as coming soon, and a chat is never started with it. If no installed harness can chat, the chat view says so. |
| `chatPreview` | Chat works but is still a preview: the UI marks it with a "Preview" badge. OpenCode and Claude Code. | No badge. |
| `approvalModes` | The chat has the app's approval modes (Always ask, Write, Yolo, Auto) as a per-session override (`LaunchRequest.approval_override`) and `configured_approval_mode` reports the harness's own. OMP, OpenCode and Claude Code. | No approval picker. |
| `yoloMode` | With `approvalModes`: Yolo (tools run without asking) is one of the modes. OMP and OpenCode. | The picker and the routine form hide Yolo, and a launch asking for it is refused (`approval_override` in `business/src/agent/launch.rs`). Claude Code: the app never starts its bypass mode. |
| `autoMode` | With `approvalModes`: Auto is one of the modes, the harness's own classifier runs the calls it judges safe and asks about the rest (`--permission-mode auto`). It counts as running tools without asking. Claude Code. | The picker and the routine form hide Auto, a launch asking for it is refused (`approval_override` in `business/src/agent/launch.rs`), and a routine with `approvalMode: "auto"` is Invalid. |
| `projectTrust` | A launch may trust the project's local resources once (`LaunchRequest.trust_once`, a routine's `trustOnce`). PI and Claude Code. | `trust_once` is dropped. |
| `projectTrustRequired` | With `projectTrust`: the harness starts only in a folder it trusts itself, so `check_project_trust` runs before the launch and a refused start shows a "Trust once" toast (`ui/src/lib/stores/start-failure.ts`). Once the user trusts a project once, `TrustMemory` (`business/src/agent/trust_memory.rs`, held by the shell's `Agents`) trusts it again for the chats opened there in the same app run, never written, and never for a routine run. Claude Code. | A start is never refused for trust. |
| `agentPicker` | The chat can switch between the harness's primary agents (`AgentAction::SetAgent`, `GetAvailableAgents`, `SessionState.agent`) and the composer shows an agent picker. None today: OpenCode's engine supports it, but the picker is off because skills cover planning. | No agent picker. |
| `compactInstructions` | `/compact` takes instructions for the summary, so picking it asks for them first. OMP and PI. | `/compact` runs at once and an action with instructions is refused as unsupported. OpenCode. |
| `agents` | `extensions_view` fills `agents` (`AgentInfo`) and the Extensions view has an Agents tab. OpenCode and Claude Code. | No Agents tab. |
| `cliActions` | Extension changes run the harness CLI (`cli_action`). | The install, enable, update and remove bars are hidden and the view is read-only. |

The default for every flag is false (`Capabilities` derives `Default`), so an adapter lists only what it has.

## 3. Detection

Implement `binary_name` (or rely on `HarnessId::binary_name`), `install_hint`, `paths` and, when the CLI differs,
`version_args`, `parse_version` and `supported_versions`. `detect` then works unchanged:

- It resolves the binary with `resolve_binary`: the user's override (only when it is an absolute existing file whose
  name is exactly the binary name, see `override_name_matches`), then absolute `PATH` entries, then the usual install
  directories (`~/.bun/bin`, `~/.local/bin`, `~/.cargo/bin`, `~/.npm-global/bin`, `~/.opencode/bin` (the OpenCode install
  script's target), Homebrew paths on macOS, `/usr/local/bin`). Add yours to `extra_search_dirs` in
  `business/src/process/resolve.rs` when the CLI's installer puts it elsewhere (Claude Code's native installer puts a
  launcher in `~/.local/bin`, which is already searched). A shell is never asked for `PATH`. Relative `PATH` entries are
  ignored.
- Every spawn of the binary (version probe, CLI actions, RPC launch, CLI sessions) sets the harness's `spawn_env`
  and appends the binary's own directory and the same
  install directories to the child's `PATH` (`launcher_search_dirs`, `ExecSpec::search_dirs`): an app started from
  the Finder has launchd's `PATH`, and a launcher such as `#!/usr/bin/env bun` must still find its interpreter.
- It runs `<binary> <version_args>` through a `ProcessRunner` with `VERSION_TIMEOUT` (5 s) and a 64 KiB output cap,
  and reports the config directory from `paths(env, None).config_root`.
- Failures become a short secret-free `Detection.error`; a missing binary yields `install_hint`.

Claude Code is the example of a harness that also moves its files with a variable: `CLAUDE_CONFIG_DIR` (captured) moves
`~/.claude` and its global file. An absolute directory it names is registered with
`security::deny::add_claude_root` (`Env::from_process` does it), so the private names the deny list refuses below
`.claude` (`history.jsonl`, `backups`, `file-history`, ...) are refused below that directory too. Do the same when your
harness's private folders are only recognisable by their parent's name.

Environment variables your `paths` reads must be listed in `CAPTURED_VARS` (`business/src/env.rs`); `Env` captures
nothing else from the process. A variable that may hold secrets and that you only need to know is set (OpenCode's
`OPENCODE_PERMISSION` and `OPENCODE_CONFIG_CONTENT`) goes in `PRESENCE_VARS` instead: `Env` keeps whether it is set
(`Env::is_set`) and never its value, so a view can say "set" without reading it. Claude Code's `ANTHROPIC_API_KEY`,
`ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_OAUTH_TOKEN` and similar are in that list.

## 4. Config and extension views

All file reads go through `SafeReader` (`security::deny`), never `std::fs` directly. `SafeReader` refuses
deny-listed files (databases, logs, keys, credential stores, blob caches), follows symlinks hop by hop, accepts
only regular files, and caps the size (`DEFAULT_MAX_BYTES` = 2 MiB). If your CLI keeps credentials in a file whose
name or extension is not on the list, add it to `DENIED_NAMES`, `DENIED_EXTENSIONS`, `DENIED_PREFIXES` or
`DENIED_DIRS` in `business/src/security/deny.rs` and extend the canary fixture (see [Tests](#11-tests)).

Config values must pass through `Redactor` before they leave the business layer. The `pi_family` helpers already
do this: `read_document` reads through `SafeReader`, `section`/`section_from` redact the parsed value into a
`ConfigSection`, and `source` describes the file and layer.

```rust
// business/src/harnesses/pi/config.rs
use crate::pi_family::{SectionText, optional_section, section, source};
```

**Formats.** `read_document(path, ConfigFormat)` parses YAML (`Yaml`), JSON with comments (`Jsonc`) and otherwise JSON. A CLI whose
config files allow comments and trailing commas (OpenCode's `opencode.jsonc`) uses `ConfigFormat::Jsonc`, which goes
through `pi_family::jsonc`: `parse` (empty or comment-only text is `null`), `strip` (comments and trailing commas become
spaces, so error locations still match the file) and `has_comments` (what a rewrite as plain JSON would lose). Errors
carry a line and column, never file content. Files that merge in layers (OpenCode's global file, `OPENCODE_CONFIG`,
the project file, `.opencode/` directories) are described layer by layer in `harnesses/opencode/config.rs`. Show a
reference to an environment variable (`{env:NAME}`) by name and never resolve or follow a file reference
(`{file:path}`).

If you add a new way to turn an unparsed file into output, redact there too (`Redactor.redact_at` with a
`RedactionContext` keeps key-path rules such as `mcpServers.<name>.env`; `redact_text` masks free text). Rules are
listed in [security-privacy.md](security-privacy.md#secret-redaction).

Extensions: reuse `pi_family::scan_skills`, `parse_servers` (MCP), `themes_in_dir`, `extension_module_paths` and
`package_manifest`; write adapter code only for what is specific (plugin registries, package layers, another MCP shape).
A harness with Claude-style plugins and marketplaces reuses `harnesses/plugin_registry/` (`installed` parses
`installed_plugins.json` version 2, `catalog` reads marketplace catalogs and marks plugins that have an update). OMP and
Claude Code share it and keep what a registry entry means (enabled state, scopes, where a plugin may live) in their own
adapters. Another MCP shape gets its own parser: OpenCode's `mcp` object holds `{"type": "local", "command": [...]}` or
`{"type": "remote", "url": ...}` entries, parsed in `harnesses/opencode/extensions/mcp.rs`, and Claude Code's servers
come from several places (`~/.claude.json`, `.mcp.json` and plugins, `harnesses/claude/extensions/mcp.rs`).
`ExtensionsView` also has `agents` (`AgentInfo`: name, description, mode
`primary`, `subagent` or `all`, model, built-in, hidden, scope and path); fill it and set `capabilities.agents` when the
harness has named agents. Every command-line word and value passes redaction before it leaves the business layer.
`Capabilities` must match what the views fill in.

## 5. Commands

A harness without chat still needs a catalog. List its commands with `unsupported(reason)` invocations, as
`harnesses/opencode/catalog.rs` does. Otherwise implement `CommandCatalog` (`business/src/harness/catalog.rs`) and return it from `Harness::catalog`.

```rust
pub trait CommandCatalog: Send + Sync {
    fn native(&self) -> Vec<CommandSpec>;
    fn map_shared(&self, command: SharedCommand) -> CommandInvocation { /* default */ }
    fn shared(&self) -> Vec<CommandSpec> { /* ... */ }
    fn parse_live(&self, _raw: &[Value]) -> Vec<CommandSpec> { Vec::new() }
    fn merged(&self, live: &[Value]) -> Vec<CommandSpec> { /* native > live > shared */ }
}
```

- `native` is required: the harness's own commands with their desktop mapping.
- `map_shared` decides how the six always-offered commands (`SharedCommand::ALL`: `/new`, `/compact`, `/model`,
  `/thinking`, `/name`, `/abort`) run. The default sends the PI-family RPC or opens the model or thinking picker.
  Override it when the CLI differs.
- `parse_live` reads the command list a running agent reports. Use `parse_live_commands(raw, source, invocation)`
  with a closure mapping the harness's source label to a `CommandSource`. The default knows no live commands.
- `merged` deduplicates by name; the first source wins (native, then live, then shared).

A `CommandInvocation` is `PromptText` (send the slash text as a prompt), `Rpc { action }`, `Gui { target }`
(open a desktop view) or `Unsupported { reason }`. Build them with the helpers `rpc`, `gui`, `unsupported` and
`spec`. Set `capabilities.slash_commands_via_prompt` when slash commands run as prompt text (OMP).

## 6. RPC launch and dialect

Skip this step when `capabilities.chat` is false: return `BusinessError::Unsupported` from `launch_spec` and any
dialect from `dialect` (nothing calls it). A harness with a protocol other than the PI-family RPC reads
[Other chat protocols](#other-chat-protocols) instead of the dialect part of this step.

**Launch.** `launch_spec` receives a `LaunchRequest`: the resolved `binary`, the canonical `project`, an optional
`resume` session id (already checked with `is_safe_session_id`), an `approval_override` and `trust_once`. Build the
argv and finish with `rpc_exec_spec`, which sets the project as working directory and, for Windows batch shims,
refuses arguments that fail `is_safe_arg`.

```rust
// business/src/harnesses/pi/launch.rs
let mut args = vec!["--mode".to_owned(), "rpc".to_owned()];
if let Some(id) = request.resume {
    args.extend(flag_with("--session", id));
}
rpc_exec_spec(request.binary, request.project, &args)
```

Do not pass flags that skip approvals (`--auto-approve`) or detach the extension UI (`--no-ui`); the forbidden-pattern
guard rejects them without opt-out. The agent inherits the app's environment so it sees the user's provider
configuration. `approval_override` and `trust_once` reach `launch_spec` only for harnesses whose capabilities enable
them (`approvalModes`, `projectTrust`); the manager filters them, refuses Yolo where `yoloMode` is false, Auto where
`autoMode` is false and, where
`projectTrustRequired` is true, refuses a start in an untrusted folder unless the launch trusts the project once.

**Dialect.** `RpcDialect` (`business/src/pi_family/rpc/mod.rs`) is the protocol seam:

| Method | Required | Default | Meaning |
| --- | --- | --- | --- |
| `harness()`, `catalog()` | yes | | Identity and the catalog that merges live commands. |
| `handshake()` | yes | | `Handshake::ReadyFrame` (wait for `{"type":"ready"}`, OMP) or `Handshake::Probe` (the first `get_state` is the probe, PI). |
| `encode(action, id, ctx)` | yes | | `AgentAction` to a command frame. |
| `history_request(cursor, id)`, `history_paged()` | yes | | How the transcript is read: one request, or pages. |
| `on_prompt_ack(request_id, data, state)` | yes | | Reacts to the prompt acknowledgement (e.g. a prompt the harness handled itself completes at once). |
| `after_handshake()` | no | none | Frames sent after the handshake, e.g. OMP's `set_event_filter`. |
| `encode_bash(command, id)` | no | `bash` frame | |
| `decode(frame, state)` | no | `decode_common` | Frame to `Inbound` (`Ready`, `Response`, `Event`, `UiRequest`, `AutoReply`, `Commands`). |
| `encode_ui_response(response)` | no | PI-family reply | Answer to an extension dialog. |

`pi_family::rpc::prelude::*` brings in what a dialect needs. `encode_shared(harness, action, id, ctx)` returns the
frame for the actions OMP and PI encode identically, or `None`; handle the rest (`Clone`, `RefreshCommands`,
`GetHistory`) yourself, or reject with `BusinessError::Unsupported`.

```rust
// business/src/harnesses/pi/dialect.rs
fn encode(&self, action: &AgentAction, id: &str, context: &EncodeContext<'_>) -> Result<Value> {
    match action {
        AgentAction::Clone => Ok(bare("clone", id)),
        AgentAction::RefreshCommands => Ok(bare("get_commands", id)),
        AgentAction::GetHistory { .. } => Ok(self.history_request(None, id)),
        _ => encode_shared(HarnessId::Pi, action, id, context)?
            .ok_or_else(|| BusinessError::Internal("action without a PI command".to_owned())),
    }
}
```

Override `decode` only where the harness reports something differently (PI turns `agent_settled` into prompt
results before delegating to `decode_common`).

### Other chat protocols

`Harness::chat_protocol()` tells the manager which engine drives a chat (`ChatProtocol` in `business/src/harness/mod.rs`).
The default is `ChatProtocol::Rpc(self.dialect())`, which is OMP and PI. A harness with another protocol returns
another variant, and `AgentSession` (`business/src/agent/engine.rs`) is an enum with one engine per protocol:
`RpcSession` (`agent/session.rs`), `OpenCodeSession` (`agent/opencode/`), `OpenCodeV2Session` (`agent/opencode_v2/`) and
`ClaudeSession` (`agent/claude/`) and `CodexSession` (`agent/codex/`). One
harness can return different protocols by version: OpenCode answers `ChatProtocol::OpenCodeServer` for 1.x and
`ChatProtocol::OpenCodeServerV2` for 2.x, chosen from the generation `note_version` stored. A new engine:

- adds a `ChatProtocol` variant and an `AgentSession` variant that forwards `send`, `respond`, `bash`, `emit`,
  `on_prompt_taken`, `move_project`, `stop`, `exit_info`, `protocol_version` and `is_running`;
- builds on `AgentProcess` (`agent/process.rs`) for the child process: spawn, the event emitter, redacted rate-limited
  stderr, the exit watcher that always sends `Exited` last, stop and kill on drop, and the startup deadline. Do not
  copy that code (`bun run dup` fails on it);
- turns what the harness reports into `AgentEvent`s and `AgentAction`s into the harness's calls, so the UI needs no
  change. OpenCode's server side (HTTP and event-stream client, a closed route allowlist, the event mapper) lives in
  `business/src/opencode_server/` and its rules in [security-privacy.md](security-privacy.md#opencode-chat). OpenCode 2
  has its own in `business/src/opencode_server_v2/`, reusing the HTTP client through the `ApiRoute` trait (a new route
  type implements it, and the client stays closed to anything else);
- fills `launch_spec` with the command that starts the engine's process (`harnesses/opencode/launch.rs`), and reads
  the extra `SessionLaunch` fields it needs (`resume`, `approval_override`, `utility`) after the process is up when
  the command line cannot carry them;
- sets the capability flags the engine supports: `chat`, `chatPreview` while it is new, `agentPicker`,
  `approvalModes` and `compactInstructions` (see [Capabilities](#capabilities)).

Test the engine with `fake_agent --dialect opencode --scenario <file.jsonl>` (`--dialect opencode2` for OpenCode 2,
`--dialect claude` for Claude Code, `--dialect codex` for Codex), see [Tests](#11-tests).

**Claude Code is the reference for a stdio engine** (`ChatProtocol::ClaudeStream`, `AgentSession::Claude`). It starts one
`claude --output-format stream-json --verbose --input-format stream-json --permission-prompt-tool stdio
--include-partial-messages` process per chat (`harnesses/claude/launch.rs`, `--resume <id>` for a saved session) and
talks to it over stdin and stdout, so there is no port, no password and no server left behind. The work splits in two:

- `business/src/claude_stream/` is pure code that never spawns anything: `wire/` (serde types for the frames and control
  requests, and the encoders of what the app sends), `mapper/` (`ClaudeMapper`: one inbound frame to `AgentEvent`s plus
  hints for the engine, with tool cards, subagent tasks and the answers the CLI is owed), `tools.rs` (the tool ids and
  the project files a call touched, shared with the Sessions view), `permissions.rs` (the approval dialogs, the answers
  to them and the Always ask hook), `allow_list.rs` (the chat's own "Always allow" answers), `questions.rs` (the
  `AskUserQuestion` dialogs) and `stats.rs`. It is tested against recordings.
- `business/src/agent/claude/` is the engine on top of `AgentProcess`: `startup.rs` (spawn, the `initialize` control
  request with the Always ask hook, `ready`), `io.rs` (the stdout reader that maps every frame under the chat's lock,
  and the table that matches `control_response` frames to the requests waiting for them, each with a time limit),
  `prompts.rs` (prompt, steering, follow-up, withdrawing a queued message, abort), `dialogs.rs` (permission prompts,
  questions and the CLI's other asks, cancels), `settings.rs` (model, thinking, mode, slash commands, rename), `ops.rs`
  (new, switch, fork and clone: each starts the CLI again), `relay.rs` (one event stream across those processes),
  `runs.rs` (`!command` and compaction) and `reports.rs` (statistics, history, last reply, export).

Requests carry ids but user messages do not and a turn ends with a `result` frame, which is why it is not an
`RpcDialect`. No model flag is passed, so the chat runs on the user's own default model, and `DISABLE_UPDATES=1` is the
harness's `spawn_env`. The flags the app must never pass, the approval dialogs and modes, the folder trust gate and the
helper chat are in [security-privacy.md](security-privacy.md#claude-code). The capabilities carry the approval
policy: Claude Code sets `approvalModes` and `autoMode` but not `yoloMode`, because the app never starts its bypass mode, and it sets
`projectTrust` and `projectTrustRequired` because stream-json mode skips Claude Code's own trust dialog (see
[Capabilities](#capabilities)). `launch_spec` ignores `trust_once`: the gate is `Harness::check_project_trust` in the
manager, and a launch that trusts the project once simply skips it. The chat is a preview (`chatPreview`). The plan with
the protocol facts is [claude-code.md](plans/claude-code.md).

**Codex is the reference for an `app-server` style engine** (`ChatProtocol::CodexAppServer`, `AgentSession::Codex`): a
JSON-RPC peer on stdio, which has requests both ways. The app starts one `codex app-server` per chat
(`harnesses/codex/launch.rs`) and the work splits in two, as for Claude Code:

- `business/src/codex_rpc/` is pure code that never spawns anything: `wire/` (serde types for only what the app uses,
  taken from the bindings the CLI generates, `codex app-server generate-ts`, and the encoders of the app's requests),
  `mapper/` (`CodexMapper`: notifications and server requests to `AgentEvent`s plus hints for the engine, with `tools.rs`
  for the tool cards and the project files a call touched, `asks.rs` for approvals and questions, `turns.rs` and
  `replies.rs`) and `stats.rs`. It is tested against recordings.
- `business/src/agent/codex/` is the engine on top of `AgentProcess`: `startup.rs` (spawn, `initialize` then `initialized`,
  `model/list`, `thread/start` or `thread/resume`), `io.rs` (the stdout reader that maps every frame under the chat's lock,
  the table that matches responses to the requests waiting for them and the refusals), `prompts.rs`, `dialogs.rs` and
  `forms.rs` (approvals, questions, elicitations), `approvals.rs` (the approval modes and trust), `settings.rs` (model,
  reasoning, state, statistics) and `images.rs`.
- Saved sessions are read through a second, short-lived `codex app-server` per view load
  (`harnesses/codex/sessions/`), never from the rollout files, whose format differs between versions.

Rules for an engine like this:

- **Match requests to responses by id, and map notifications on their own.** The transport pieces (`FrameReader`,
  `PendingRequests`) fit. `RpcDialect` does not: its handshake is ready-frame based, `encode_ui_response` has no method
  context and its decoder assumes a `type` field.
- **Answer every request the server sends, and refuse what you do not serve.** Codex asks the app for approvals,
  questions and tool calls, and for token refreshes. Dynamic tool calls and `account/chatgptAuthTokens/refresh` are
  refused at once, an unknown method too (`io.rs`).
- **Decode only the fields you use, and tolerate the rest.** Frames and items gain fields between weekly releases. An
  item type the app does not show decodes as an ignored item (`wire/items.rs`).
- **Settle each turn from its own end notification** (`turn/completed`), never from a response.
- **Set the mode on the thread, not on the turn.** The approval policy and sandbox go on `thread/start` and
  `thread/resume`, so the user's writable roots survive. A mode change resumes the thread.
- Regenerate the bindings and re-record on every CLI version bump, and list the tested versions in `supported_versions`.

## 7. Sessions

Two shapes exist. A harness that keeps session files the app can read (PI family) supplies `sessions_dir` and uses or
overrides the file methods. A harness whose sessions sit behind a CLI (and, for OpenCode, in a database that also holds
credentials, which the app never opens) implements `CliSessions` and returns it from `cli_sessions()`. Both need
`sessions_dir`, which for a CLI harness is only the folder shown to the user.

**CLI sessions.** `CliSessions` (`business/src/harness/mod.rs`) has `list`, `messages`, `files_touched` and `delete`.
Each receives a `CliSessionCtx { env, project, binary, runner }`: the canonical project, the detected
`ResolvedBinary` and a `BlockingRunner` (`business/src/process/blocking.rs`). `BlockingRunner::run(&ExecSpec)` is the
synchronous twin of `ProcessRunner`, for code that runs in a blocking task: no shell, null stdin, a time limit that
kills the process group, a byte cap per stream, a timeout reported in `ExecOutput::timed_out`. Production uses
`StdRunner`, tests inject a runner that replays recorded output. The shell (`with_sessions` in
`src-tauri/src/commands/sessions.rs`) waits for the detected binary before the blocking task. Sessions are linear
here: no tree, resume or fork (`capabilities.sessionTree` false). Remember to:

- override the five file methods to return `Unsupported` (the defaults would read PI-family files),
- check every session id (`is_safe_session_id` and the CLI's own prefix) before anything is spawned,
- build each `ExecSpec` with the harness's `spawn_env` and `launcher_search_dirs`, and set a time limit and an output
  cap,
- filter by project if the CLI lists or exports sessions of other projects (OpenCode's `export` and `session delete`
  find a session of any project, and its `session list` covers the whole git worktree, so the adapter keeps sessions whose
  directory is the project or inside it),
- treat empty output as "no sessions" when the CLI prints nothing for an empty list,
- redact what you pass on, including CLI error lines.

OpenCode's commands, all with `--pure` (no external plugins): `session list --pure --format json`,
`export <id> --pure`, `session delete --pure <id>`. See `harnesses/opencode/sessions.rs`. OpenCode 2 has no `--pure`.
It takes `--standalone` instead on every call (`session list --standalone --format json -n 5000`, `session export
--standalone <id>`, `session delete --standalone <id>`), because without it the CLI starts its background service and
leaves it running. See `harnesses/opencode/sessions/v2.rs`.

**Codex** is a third shape of CLI sessions: no command lists them, but its `app-server` does. `cli_sessions()` starts a
short-lived `codex app-server` per view load (`harnesses/codex/sessions/app_server.rs`, a time limit of 30 seconds, one
at a time, the result cached) and asks `thread/list`, `thread/turns/list`, `thread/items/list` and `thread/delete`. The
adapter overrides the five file methods to return `Unsupported`, as it must for any CLI harness. What the real CLI
does is in the [plan](plans/codex.md): `thread/list` leaves `forkedFromId` out, so there is no session tree.

**Session files.** `sessions_dir` is required. The default `list_sessions`, `resolve_session`, `session_tree`, `session_messages` and
`session_files_touched` read PI-family session files (`<timestamp>_<id>.jsonl`, entries linked by `id`/`parentId`)
through `pi_family::sessions`. A harness with the same file format only supplies the folder. Helpers:
`pi_dir_name`, `omp_dir_name`, `canonical_text`, `configured_session_dir`. A harness with another format overrides the
five methods. Claude Code does: `<config>/projects/<slug>/<uuid>.jsonl` (the slug is the project path with every
non-alphanumeric character turned into `-`, cut and hashed over 200 characters), message lines linked by
`parentUuid`, and lines the Sessions view must skip (`attachment` and `api-request*` lines hold the user's CLAUDE.md
and the system prompt). Its delete removes the session's own folders as well and is refused while a running `claude`
names the session in `<config>/sessions/<pid>.json` (`harnesses/claude/sessions/`). The webview only sends a `SessionRef { harness, id }`; the id is resolved by a shallow scan of the
project's folder, so no other file can be named, and companion directories (subagent transcripts, tool logs) are never
entered.

## 8. CLI actions

Extension changes (install, remove, enable, upgrade) run the harness CLI. A harness without such actions leaves
`capabilities.cliActions` false (and the default `cli_action`, which returns `Unsupported`): the UI then hides the action
bars and the extension lists are read-only. OpenCode sets it to true, with install only (below). Otherwise implement `cli_action(action, project)`:
map an `ExtensionAction` to a `CliInvocation { args, dry_run_flag }`. Validate every user-supplied value with
`cli_target(TargetKind, value)` (non-empty, at most 512 characters, no leading `-`, only `A-Z a-z 0-9 @ . _ / : + =
, -`, no URL credentials).

```rust
Ok(CliInvocation::new(["install", source.as_str()]).push_optional(local))
// with a preview flag the CLI really honours:
Ok(CliInvocation::new(["plugin", "install", target.as_str(), "--scope", scope]).with_dry_run("--dry-run"))
```

Set `dry_run_flag` only when the CLI honours it for that action. Without it, a dry run runs nothing and returns just
the command line (`run_cli_action` handles this). Override `cli_failure_line` if the CLI exits 0 on failure (OMP
does). Return `BusinessError::Unsupported` for actions the CLI does not have; that is also the default.

A CLI without a command for an action can still support it by editing its own config file. Override
`config_action(env, action, project, dry_run)` for that: `extension_action` asks it first, and `Ok(None)` sends the
action on to `cli_action`. The edit returns a `CliResult` with `config_edit` set (`argv` and `command` are empty), and a
dry run only reports what it would do. OpenCode is the example. `cli_action` maps `PluginInstall` in the user scope to
`opencode plugin <module> -g` (`business/src/harnesses/opencode/cli.rs`), and on OpenCode 2 to `opencode plugin add <module>`. The module must pass
`cli_target(TargetKind::NpmModule, …)`, so only an npm registry name with an optional version or tag. Project
installs, upgrade (`-f`), enable, disable and marketplace actions are `Unsupported`. OpenCode has no preview flag, so a
dry run never starts the CLI. `PluginUninstall` goes through `config_action` and removes the entry from the `plugin`
list of the global config file (`plugin_remove.rs`, on OpenCode 2 the `plugins` list and the legacy `plugin` list). It never edits another file, and it leaves a file with comments, a
`[spec, options]` entry or a plugin another file defines to the user (`config_edit.manualEdit`, `ok` false).

Claude Code maps its plugin and marketplace actions to `claude plugin …` in the user scope (`--scope user`, or
`--scope project` for a project-scope install) with `--json` (`harnesses/claude/cli.rs`, `cli_summary` reads the
`message` of the result line). It has no package actions and updates one plugin at a time. It never sends `-y`/`--yes`: that
would accept a marketplace-declared command without showing it, so such a plugin is installed from a terminal. There is
no preview flag, so a dry run only reports the command line.

Codex maps its actions to `codex plugin add|remove <name@marketplace>` and `codex plugin marketplace add|remove|upgrade`
(`harnesses/codex/cli.rs`), always in the user scope and in the home folder. It has no `--json`, so the CLI's last line is
the summary, no preview flag, and no command to switch a plugin on or off or to update one, so those actions return
`Unsupported`.

`run_cli_action` runs the binary without a shell in the project (or the home directory), with a 5 minute timeout and
capped output, strips ANSI escapes and redacts both streams. See `business/src/harnesses/omp/cli.rs` and
`business/src/harnesses/pi/cli.rs`.

## 9. Recommended setup

**Every newly supported harness must be added to the Recommended setup step** of onboarding (also reachable from
**Run recommended setup** on the Ecosystem screen). The step installs the Harness Skills (where a package exists) and
registers the Harness Dashboard and Harness Memory MCP servers for each detected harness. `Harness::companion_setup` is a required method, so the compiler
enforces the first half; the rest is yours to check.

Implement it in the adapter (`business/src/harness/mod.rs` defines the types):

```rust
fn companion_setup(&self) -> CompanionSetup {
    CompanionSetup {
        skills: SkillsStrategy::Package { source: "git:github.com/owner/repo" },
        mcp: McpTarget::UserMcpJson { also_read: &[] },
    }
}
```

`SkillsStrategy`:

- `Marketplace { source, plugin }`: the CLI has a plugin marketplace (OMP's `omp plugin`, Claude Code's `claude
  plugin`). The app runs `marketplace add <source>` (unless a marketplace of the plugin's marketplace name or from
  `source` is registered) and `plugin install <plugin>` in the user scope through `extension_action`, so `cli_action`
  must support both ([CLI actions](#8-cli-actions)). A plugin the user settings switch off is offered its `plugin
  enable` instead. The registries are read per harness in `marketplace_skills` (`business/src/ecosystem/setup.rs`);
  a new marketplace CLI needs its reader there.
- `Package { source }`: the CLI has its own package manager (PI's `pi install <source>`, which PI uses). The app installs the source
  for the user through `extension_action` (`PackageInstall`), so `cli_action` must support it. The step reports the
  skills as installed when the harness's user settings list the package (`user_has_package`, in
  `business/src/ecosystem/setup.rs`); extend that if your CLI records packages elsewhere.
- `Installer(SkillsHost)`: the Harness Skills have their own installer script (`install.sh` / `install.ps1`, which
  need Node.js 22 or newer), one run per host (`HARNESS_SKILLS_HOST`: OpenCode by default, `codex`). The app runs it
  through a `Companion` (`OpenCodeSkills`, `CodexSkills`, in `business/src/ecosystem/installer.rs`) after the user
  confirms, with no CLI action. The step reports the skills as installed when the host's install record holds a version
  (`business/src/ecosystem/skills_record.rs`: a regular file, not a symlink,
  `plugins/harness-useful-skills/install-record.json` in OpenCode's config directory,
  `harness-useful-skills/install-record.json` in the Codex home). A record older than the first
  release with the host's adapter (`CODEX_SKILLS_MIN_VERSION`, 0.3.13) is offered the installer again, with
  `outdatedFrom` set so the UI says "Update available". So is a record still under the names from before Harness
  Skills 0.4.0 (folder `useful-skills`, found only while no record exists under the new name): it counts as installed,
  and re-running the installer migrates it. The installer registers the `harness-useful-skills` MCP server itself
  (`useful_skills` before 0.4.0), so the MCP matrix does not. Both server names are reserved for Share setup
  (`LEGACY_SKILLS_MCP_SERVER` in `catalog.rs`). The data files (`models.yml`, `workflow-settings/`) are read through
  `skills_data_path` (`business/src/ecosystem/skills_paths.rs`): the new folder wins, the old one is used when only it
  exists. The app never writes them, so any editor added later must write the new path when the install is new (its
  record is under the new name) and the old path when only the old record exists. On each launch the app migrates such an install by itself (`business/src/ecosystem/skills_migration.rs`, the `skills_migrate_names` command, gated by the update switches and a Harness Skills release of 0.4.0 or newer): `legacy_skills_step` in `setup.rs` says what to run per harness (the plugin install with its marketplace, or the host's installer), and a new harness with a `Marketplace` or `Installer` strategy gets it for free. A host whose adapter writes model
  roles (Codex from Harness Skills 0.4.0) lists them in its config view only for such an install. A new script host needs its own `SkillsHost` and `Companion`, a fixed command per OS in
  `catalog.rs`, and a Tauri command.
- `Manual { command, note }`: the user copies a command, the app never runs it. Use it when you have not verified an
  install command against the CLI yet, and say so in `note`. The step reports the skills as installed
  when the harness's user settings list the package (`user_has_package`, in `business/src/ecosystem/setup.rs`);
  extend that if your CLI records packages elsewhere. No harness uses it right now.
- `Unsupported { reason }`: the skills do not exist for this harness.

`McpTarget`:

- `UserMcpJson { also_read }`: `<agent_dir>/mcp.json`, where `agent_dir` comes from `paths(env, None)`. The app merges
  `{"command": "harness-dashboard", "args": ["mcp"]}` into its `mcpServers` object (`business/src/ecosystem/mcp_register.rs`).
  Choose it only after confirming that the CLI reads `mcpServers` from that user file. List other files of the agent
  directory that can also define servers in `also_read` (OMP adds `.mcp.json`); they are only read, to find an
  existing entry. The same file also receives the project memory server (below).
- `OpenCodeConfig`: the `mcp` object of OpenCode's global config file. OpenCode merges every global config file it
  finds. The app writes the one OpenCode itself would edit (`opencode.jsonc`, else `opencode.json`, in the config
  directory), creates `opencode.json` when there is neither, and only reads the others (`config.json` there, and
  `opencode.json[c]` in `~/.opencode` and in `OPENCODE_CONFIG_DIR`) to find an existing entry. The entry is
  `{"type": "local", "command": ["harness-dashboard", "mcp"], "enabled": true}`, and `"enabled": false` counts as switched
  off. The file is JSONC: a file with comments is never rewritten (a rewrite would lose them), so the setup row shows
  the entry to paste (`McpState::ManualEdit { file, snippet }`, with a copy button) instead. Only comments block a write.
  The target carries the OpenCode generation (`OpenCodeConfig(OpenCodeGeneration)`). On OpenCode 2 the entry is
  `mcp.servers.<name>` with `{"type": "local", "command": ["harness-dashboard", "mcp"]}` and no `enabled`, there is
  no `config.json`, and an existing entry is recognised in either shape.
- `ClaudeCli`: Claude Code's user scope, the top-level `mcpServers` of its global file (`~/.claude.json`, or
  `.claude.json` in `CLAUDE_CONFIG_DIR`). Every running Claude Code rewrites that file, so the app only reads it
  (`Shape::ClaudeCli`, a write through the file path is refused) and changes it through `claude mcp add-json --scope
  user <name> '<json>'` and `claude mcp remove --scope user <name>` (`business/src/ecosystem/mcp_cli.rs`), with the
  same decisions as for the other files, a 60 second limit per run and no retry. The entry is `{"type": "stdio",
  "command": "<absolute path>", "args": ["mcp"]}`.
- `CodexCli`: Codex's user `config.toml` (`$CODEX_HOME/config.toml`, else `~/.codex/config.toml`), key `mcp_servers`. The
  file is TOML with the user's own formatting and comments, so the app only reads it (`Shape::CodexCli`) and changes it
  through `codex mcp add <name> -- <absolute path> mcp` (or `--url <url>`) and `codex mcp remove <name>`
  (`business/src/ecosystem/mcp_cli.rs`, same decisions and limits as `ClaudeCli`). `codex mcp add` takes only a command
  and its arguments, or a URL and a bearer token variable, so a change whose old or new entry holds anything else
  (`env` values, `http_headers`, `enabled`, timeouts) is refused before anything runs: an entry with literal secrets is
  never rewritten.
- `Unsupported`: the CLI has no user MCP file the app can write. The step then shows "No MCP file to edit" for it, and
  the harness gets no project memory entry.

A third kind of file needs a new `McpTarget` variant, a `Shape` in `business/src/ecosystem/mcp_merge.rs` (the key that
holds the servers and the entry to write) and a case in `mcp_target` (`business/src/ecosystem/setup.rs`), which returns
the `McpFile { path, also_read, shape }` that Recommended setup and project memory write. The merge, its refusals and
the atomic write are shared.

**Project memory comes with the MCP target.** At every launch, with the **Project memory** setting on and the
`harness-memory` server installed (Recommended setup or the Settings switch runs its installer after the user confirms;
it is the separate [harness-useful-memory](https://github.com/MSpiechowicz/harness-useful-memory) program, not bundled),
the app registers it in the MCP file of each detected harness whose `companion_setup` has a target other than
`Unsupported` (`memory_setup.rs`, `register_memory_mcp`, through `mcp_target`), with no prompt and no per-harness code.
For `UserMcpJson` the entry is `{"command": "<resolved absolute path of harness-memory>", "args": ["mcp"]}`, for
`OpenCodeConfig` `{"type": "local", "command": ["<path>", "mcp"], "enabled": true}`. It is written through the same file
as the dashboard entry, so the "confirm the CLI reads the servers from that file" check above also covers it. Nothing else to
implement; verify that your CLI starts such a server, and that your `also_read` files are listed so an existing
`harness-memory` entry in them is found and not duplicated. A removed entry is not added back, and an entry the user
named `harness-memory` that is not the app's own is never changed. A JSONC file with comments is left alone. See
[security-privacy.md](security-privacy.md#project-memory).

The webview sends only a harness id; the file path comes from `paths` (or `mcp_target`) and the installer command is fixed in Rust, so
no new command or permission is needed. Add the harness to the setup copy as well: the strings are under
`onboarding.setup.*` in `ui/src/lib/i18n/locales/*/onboarding.ts` (all six languages), and the mock in
`ui/src/lib/ipc/mock/setup-handlers.ts` keeps a per-harness MCP file and skills offer (`Record<HarnessId, ...>`).

## 10. Share setup

**Share setup** (Settings) exports a harness's extensions and MCP servers as a `hud1:` string or file and imports them
on another machine. A harness takes part by declaring what it can share; the default shares nothing, so a new harness
opts in kind by kind. Format and behaviour: [share-setup.md](share-setup.md).

```rust
fn share_support(&self) -> ShareSupport {
    ShareSupport { marketplaces: false, plugins: false, packages: true, mcp_servers: true }
}
```

`ShareSupport` (`business/src/harness/mod.rs`) has one flag per `SyncItemKind`. OMP declares marketplaces, plugins and
MCP servers, PI declares packages and MCP servers, OpenCode declares plugins and MCP servers, and Claude Code and Codex
declare marketplaces, plugins and MCP servers.

- **`mcp_servers`** needs `McpTarget::UserMcpJson` (or `OpenCodeConfig`) in `companion_setup` ([Recommended setup](#9-recommended-setup)): the
  export reads, and the import merges into, the file `mcp_target_paths` returns for the harness (it returns `None` for
  any other target, so only strict-JSON `mcpServers` files are shared). `McpTarget::CodexCli` shares the
  `mcp_servers` of Codex's `config.toml` as key names only (`business/src/sync/collect_codex.rs`), and the import runs `codex mcp add`
  per server. `McpTarget::ClaudeCli` shares the user scope
  of Claude Code's global file: the import never merges it but runs `claude mcp add-json --scope user` per server
  (`business/src/sync/apply_mcp_cli.rs`, the arguments of `ecosystem::mcp_cli::add_json_args`), and a server with a
  `${…}` substitution is left out and refused. Only user scope is shared. With
  `McpTarget::Unsupported` the harness's servers are blocked as not supported. `McpTarget::OpenCodeConfig` shares the
  `mcp` key of the global config file (`local` as stdio, `remote` as http), with key names only for `environment` and
  `headers`. A server whose command, arguments or URL hold a `{env:…}` or `{file:…}` substitution is left out on
  export and refused on import. A config file with comments is not rewritten: the import skips the servers as
  `manualEdit` with the entry to paste.
- **`marketplaces`, `plugins`, `packages`** are CLI installs, and the import runs them through `cli_action`
  ([CLI actions](#8-cli-actions)). Every kind you declare must be supported there: `MarketplaceAdd { source }`
  for marketplaces, `PluginInstall { target, scope: User }` for plugins, `PackageInstall { source }` for packages
  (PI also receives no `--local`). The import builds only these actions, never remove, upgrade or project scope. A step
  the adapter refuses makes the item `blocked`, never offered. OpenCode shares only npm registry plugins of the global
  config files, as the package name plus the version or tag the config pins (`opencode plugin <name>@<version> -g`).
  Local files, git, `github:` and URL specs, and `[spec, options]` entries (options may hold secrets) are left out.
- **Reading what is installed.** The MCP reader is generic. The other kinds read the harness's own registry files:
  `collect_kind` in `business/src/sync/collect.rs` (export) and `Machine::read` in `business/src/sync/plan_presence.rs`
  (statuses `alreadyPresent`, `differentVersion`, `conflict`) currently understand OMP's marketplace and plugin
  registries, PI's `packages` setting, OpenCode's `plugin` lists (`business/src/harnesses/opencode/share.rs`), Claude
Code's `known_marketplaces.json` and `installed_plugins.json` (`business/src/harnesses/claude/setup.rs`, export in
`business/src/sync/collect_claude.rs`) and the `[marketplaces]` and `[plugins]` tables of Codex's `config.toml`
(`harnesses/codex/extensions.rs`, export in `business/src/sync/collect_codex.rs`). A harness that stores these elsewhere needs a reader there too (through
  `SafeReader`, returning only references, never credentials); one that uses the same files reuses the readers.
- **Validation is shared.** Sources and ids are checked by `business/src/sync/validate.rs` and `validate_source.rs`
  (MCP fields by `validate_mcp.rs`), which allow `npm:` (registry names, `github:owner/repo`, `https:`/`git+https:`),
  `git:` and `https://` package sources, GitHub `owner/repo` or `https://` marketplace sources, and `name@marketplace`
  or npm plugin ids. A harness whose sources look different must extend those validators deliberately, since they are the
  import's security boundary.

The bundle's harness key is the `HarnessId`, so a new harness appears in exports and imports without format changes.

## 11. Tests

Add tests next to the existing ones. `business/tests/` holds four test binaries, `core/`, `engines/`, `setup/` and `views/`:
each `main.rs` lists its modules (one file per area below) and the helpers it uses from `business/tests/support/`.
Follow the same pattern for the new harness.

| Test | Where | What to cover |
| --- | --- | --- |
| Detection | `detection.rs` | Binary lookup, override validation, version parsing and support, install hint. |
| Paths | `harness_paths.rs`, `session_paths.rs` | `paths` and `sessions_dir` for default and overridden env. |
| Views | `omp_views.rs`, `pi_views.rs`, `opencode_views.rs`, `claude_views.rs`, `claude_extensions.rs`, `codex_views.rs` | Snapshot (`insta`) of `config_view` and `extensions_view` over a fixture home `business/tests/fixtures/acme-home/` and the demo project. |
| Leaks | `canary.rs`, `adapter_canary.rs`, `safe_reader.rs`, `deny_claude.rs`, `deny_codex.rs` | Hide `CANARY-7f3a9c` in deny-listed files and secret values of your fixture home; no view may contain it. `adapter_canary.rs` iterates every registered harness; add your fixture home to its environments. |
| Catalog | `catalog.rs` | Native, live and shared merge; every command's invocation. |
| Dialect | `rpc_dialects.rs`, `rpc_commands.rs`, `rpc_normalize.rs` | Encoded frames per action, decoded recorded frames, handshake. Not needed when `capabilities.chat` is false. |
| Sessions | `sessions_store.rs`, `sessions_claude.rs`, `sessions_claude_transcript.rs`, `sessions_codex.rs` (the helper process against `fake_agent --dialect codex`) | Listing, tree and transcript over fixture sessions in `business/tests/fixtures/sessions/acme/` (Claude Code: `sessions/claude/`, synthetic files shaped after the real structure, with `attachment` and `api-request*` lines that must stay out). |
| CLI sessions | `sessions_opencode.rs`, `sessions_opencode_transcript.rs`, `blocking_runner.rs` | A replay `BlockingRunner` returns recorded CLI output (`business/tests/fixtures/sessions/opencode/`): list, empty list, transcript paging, files touched, delete, other-project filtering, invalid ids spawn nothing. `blocking_runner.rs` runs `StdRunner` against a real child process. |
| CLI actions | `cli_actions.rs`, `claude_cli_actions.rs`, `codex_cli_actions.rs` | Argument construction, validation errors, dry run, failure line. |
| Chat engine | `agent_session.rs`, `agent_manager.rs`, `agent_dialogs.rs`, `agent_failures.rs` | The real session engine against `fake_agent`. A non-RPC engine has its own set, for OpenCode `opencode_engine*.rs`, `opencode_launch.rs`, `opencode_http.rs` and the mapper tests, over recordings of the real server in `business/tests/fixtures/opencode/server/` and scenarios in `business/tests/fixtures/streams/opencode_*.jsonl`. For Claude Code: `claude_launch.rs`, `claude_wire*.rs`, `claude_mapper*.rs`, `claude_tools.rs`, `claude_stats.rs`, `claude_allow_list.rs` (the pure `claude_stream` code over the scrubbed recordings in `business/tests/fixtures/claude/stream/`), `claude_engine*.rs` (the engine against `fake_agent --dialect claude` and the scenarios in `business/tests/fixtures/streams/claude_*.jsonl`: approvals and questions in `claude_engine_asks.rs`, the approval modes in `claude_engine_modes.rs`, `!command` in `claude_engine_bash.rs`, session operations in `claude_engine_ops.rs`) and `claude_chat_start.rs` (the folder trust gate and the helper chat through the manager). For Codex: `codex_wire.rs`, `codex_mapper*.rs` (the pure `codex_rpc` code over the scrubbed recordings in `business/tests/fixtures/codex/app_server/`), `codex_engine*.rs` (the engine against `fake_agent --dialect codex` and the scenarios in `business/tests/fixtures/streams/codex_*.jsonl`) and `codex_trust.rs`. |
| Recommended setup | `setup_status.rs`, `setup_mcp_register.rs` | `setup_status`: skills and MCP state of your harness on an empty machine, with the skills installed and with the MCP entry present (build the fixture files in a temp home). `setup_mcp_register.rs`: the `mcp.json` merge for your `also_read` files. `setup_mcp_register_opencode.rs`: the same for a JSONC file (a file with comments gives `ManualEdit`, nothing written). `setup_mcp_register_codex.rs`: the `codex mcp add` / `remove` runs of `McpTarget::CodexCli`. `setup_status_claude.rs` and `setup_mcp_register_claude.rs`: the marketplace skills state and the `claude mcp add-json` / `remove` runs of `McpTarget::ClaudeCli`, over a recording `ProcessRunner`. The installer itself is harness-independent (`setup_installer.rs`). |
| Share setup | `sync_export.rs`, `sync_import.rs`, `sync_import_apply.rs`, `sync_export_claude.rs`, `sync_import_claude.rs`, `sync_export_codex.rs`, `sync_import_codex.rs` (helpers in `business/tests/support/sync_support/`) | Export: what your fixture home yields and what is excluded, with no secret literal in the code or file. Import: statuses on an empty machine and with items present, the exact CLI steps (recording `ProcessRunner`, zero runs for hostile values), idempotent second apply, MCP merge keeps other keys and never overwrites. `sync_codec.rs` and `sync_validate.rs` are harness-independent. |
| Fixtures | `mock_fixtures.rs` | Add the harness's views and catalog to the exported mock fixtures. |

**Fake agent.** `fake_agent --dialect opencode` is a scripted `opencode serve` (an HTTP server on `127.0.0.1` with its own
scenario steps: `allow`, `expect`, `hold`, `sse`, `listen`). Its step format is documented at the top of
`business/src/bin/fake_agent/opencode.rs`. `fake_agent --dialect claude` is a scripted `claude` in stream-json mode: it reads NDJSON frames on stdin, answers control
requests and can replay frames of a recording (`replay`, `include`, `$name` captures), documented at the top of
`business/src/bin/fake_agent/claude.rs`. `fake_agent --dialect codex` is a scripted `codex app-server` that speaks JSON-RPC on
stdio and can replay the frames of a recording (`replay`, `from`, `to`, `except`, `replace`), documented at the top of
`business/src/bin/fake_agent/codex.rs`. For the other dialects, `business/src/bin/fake_agent.rs` plays a JSONL scenario: `{"emit": {...}}` writes a frame,
`{"expect": "get_state", "reply": {...}}` waits for a request of that type and answers it, `{"exit": 1}`,
`{"hang": true}`, `{"stderr": "..."}`. Add scenarios under `business/tests/fixtures/streams/` (for example
`acme_startup.jsonl`, `acme_basic.jsonl`) and start them with `ScenarioSpawner` from
`business/tests/support/agent_support/mod.rs`. `--dialect omp|pi|mcp` is a command-line switch; it only changes the opening
(the OMP `ready` frame is hard-coded for `omp`). If your handshake needs another opening frame, extend `main` in
`fake_agent.rs`. The shell's agent command tests in `src-tauri` need the binary too; see
[architecture.md](architecture.md#testing).

Tests that pin the list of harnesses (for example `model.rs`, `detection.rs`, `mock_fixtures.rs` and
`setup_status.rs`) need the new harness too. The compiler does not find them, the failing tests do.

Run `cargo test -p harness_business`. `cargo test -p harness_business export_` regenerates the bindings and mock
fixtures; commit the result, because CI fails on a difference.

## 12. UI touch points

The UI renders any harness from data, with these exceptions:

- **Bindings.** `HarnessId.ts` and the other generated types update with `export_` (never hand-edit).
- **Badge colour.** `ui/src/lib/components/Badge.svelte` has `HARNESSES: Record<HarnessId, string>` mapping a harness
  to the classes `bg-<id>-wash text-<id>`. The type check fails until you add the entry. `HarnessBadge.svelte` (the
  initials tile) has a `COLORS` map with the same classes and no type check: a harness missing there gets a neutral
  wash, and a short name longer than three letters shows its capitals ("OC" for OpenCode). Define the tokens
  `--harness-<id>` and `--harness-<id>-wash` for both themes in `ui/src/lib/design/tokens.css`, and the
  `--color-<id>` and `--color-<id>-wash` aliases in `ui/src/app.css`. Match the colour the Harness Dashboard uses for
  the harness (`ui/src/lib/dashboard/palette.ts`).
- **Capability gates.** The UI reads `capabilities`, never a harness id, to decide what to show. `chat` gates every
  place a chat starts (the chat view, the harness picker, the palette and shortcuts, review comments sent to the
  agent, onboarding): `harness.chatActive` is the active harness when it can chat, else the first installed one that
  can, and `harness.chatComingSoon` names the harness to put in "Chat coming soon" when none can. `cliActions` hides the
  extension action bars and `agents` shows the Agents tab (`extension-items.ts`, `extension-groups.ts`,
  `extension-detail.ts`). Sessions hide resume and fork when the harness cannot chat. `yoloMode` and `autoMode` decide whether Yolo and Auto are offered in the approval picker and the routine form
  (`ApprovalBadge.svelte`, `routine-abilities.ts`), and a start refused with the `untrusted` code shows the "Trust once" toast
  (`start-failure.ts`).
- **Onboarding.** The harness cards are data-driven. The approval step shows a notice only for a daily driver that runs
  tools without asking (`approvalNotice` in `onboarding-state.svelte.ts`: OMP "yolo", OpenCode allow-by-default
  permissions, and Claude Code only when its `NativeApprovalMode.unattended` is true, that is `permissions.defaultMode`
  is `bypassPermissions` or `auto`). The app never changes the harness's permissions, so a new harness with a
  permissive default needs a variant of that notice.
- **Mock.** `ui/src/lib/ipc/mock/fixtures.ts` builds `Record<HarnessId, ...>` entries for config, extensions,
  commands, session info and session lists. Add the exported fixtures there. The handlers with per-harness branches
  (`extension-action-handlers.ts`, `agent-handlers.ts`, `session-handlers.ts`, `setup-handlers.ts`) need the new
  harness as well. `harness_list.json` and
  `project_recent.json` are synthetic (`business/tests/views/mock_fixtures.rs`); add the new harness summary.
- **Share setup.** Nothing to change: the export and import rows are data-driven from the plan the backend returns, and
  the harness name and badge come from the same sources as everywhere else. The sync mock
  (`ui/src/lib/ipc/mock/sync-handlers.ts`) has scenarios you can extend if you want your harness in them.
- **Setup step.** The mock's per-harness maps in `ui/src/lib/ipc/mock/setup-handlers.ts` (MCP file, skills offer) and
  the `onboarding.setup.*` copy; see [Recommended setup](#9-recommended-setup).
- **Copy.** Strings that name the supported harnesses are written by hand: `shortDescription` and `longDescription` in
  `src-tauri/tauri.conf.json`, the README's harness table and feature list, and the i18n strings in all six locales (en, de, es, fr, pl, pt;
  `ui/src/lib/i18n/locales/*/`), for example `harness.gate.noneBody`, `sessions.empty.noHarness.body` and the
  `onboarding.*` welcome, harness and done strings. `ui/src/features/shell/views.ts` only holds the generic
  placeholder text of each view and names no harness. Never join two sentences with a semicolon in a UI string.

Then run `bun run check`, `bun run test` and `bun run guard`.

## Lessons from Codex

Codex was the fifth harness and the first added after the per-harness code had grown. Three things are worth repeating.

**Silent per-harness fallbacks are now explicit.** Several places treated "the other harness" as "OMP" or "an MCP
file", so a new harness compiled and then did the wrong thing quietly. Each is now a `match` or `switch` that names
every harness, with no `_`/`else` arm, so the compiler finds the next one. When you add a harness, check these:

- `business/src/ecosystem/setup.rs`: the plugin and marketplace registries behind the skills state are chosen per harness
  in `business/src/ecosystem/setup/registries.rs` (was "Claude Code, else OMP").
- `business/src/sync/plan_presence.rs`: `Machine::read` matches the harness (was "Claude Code, else OMP").
- `business/src/sync/collect.rs`: `collect_kind` lists every harness and kind pair, including the ones that have none
  (was `(_, McpServer)` and a catch-all `_ => {}` that yielded nothing).
- `ui/src/features/onboarding/onboarding-state.svelte.ts`: `approvalNotice` names each driver, and says which ones ask by
  default.
- `ui/src/features/extensions/actions/action-model.ts`: `installAndRemoveOnly` and the user-scope-only check name each
  harness.
- `ui/src/lib/ipc/mock/setup-handlers.ts` and `setup-roster.ts`: the mock keeps a real row per harness (Codex's skills
  row is the installer script with its changes), not a placeholder.

**An `app-server` style engine is its own engine.** See [Other chat protocols](#other-chat-protocols). Take the protocol
from the bindings the CLI generates (`codex app-server generate-ts` and `generate-json-schema`, which carry no version
header, so regenerate them on every version bump) and confirm each method against the real CLI before relying on it.
Several things differed from the bindings: `thread/list` leaves `forkedFromId` out, an unknown thread answers
`-32600 "thread not loaded"`, `plugin/list` unfiltered answered 18.8 MB, and a user's configured model can be refused
for their login. The plan keeps these as "Verified facts" ([codex.md](plans/codex.md)).

**Record the real protocol, then replay it.** `business/tests/fixtures/codex/record.py` drives a real `codex app-server`
in a scratch git repo with `python3 -I record.py <project_dir> <out_dir>` and writes both directions of every
scenario to `<out_dir>/<scenario>.jsonl` as `{"dir": "in" | "out", "frame": …}`. `scrub.py <src> <dst> <project>` then
rewrites paths and the account, and cuts long lists to three entries, into
`business/tests/fixtures/codex/app_server/`. It replaces your home folder and user name by itself; pass the account
email in `SCRUB_EMAIL` and any other private path (extra writable roots) in `SCRUB_EXTRA` as `path=replacement;…`,
then check the output for anything else personal before you commit. Delete the scratch threads afterwards.
`fake_agent --dialect codex` replays the scrubbed frames inside scenarios, so the mapper and the engine are tested
against what Codex really sent.
