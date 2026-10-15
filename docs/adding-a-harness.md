# Adding a harness

A harness is one coding-agent CLI. The app supports OMP and PI today. This guide adds a new one, called `acme`
(binary `acme`) in the examples. The UI renders from `HarnessInfo`, `Capabilities` and the views an adapter returns,
so most of the work is Rust in `business/`.

**Fit check.** The chat engine drives a harness over stdio with newline-delimited JSON requests that carry an `id`,
answered by `response` frames (`pi_family::rpc`). A CLI that can be driven that way fits, even when its frames
differ: the `RpcDialect` decides every frame it sends and decodes. A CLI with another protocol (for example a
one-shot streaming mode without request ids) needs engine work first. Whether Claude Code or Codex fit depends on
their protocol; neither is implemented, and that engine work is not covered here.

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
10. [Tests](#10-tests)
11. [UI touch points](#11-ui-touch-points)

## 1. HarnessId

`business/src/model/harness.rs`. The variant serializes lowercase, so `Acme` becomes `"acme"` in JSON and in the
generated `HarnessId.ts`.

```rust
pub enum HarnessId { Omp, Pi }
pub const ALL: [HarnessId; 2] = [HarnessId::Omp, HarnessId::Pi];
pub fn binary_name(self) -> &'static str { /* "omp", "pi" */ }
```

Add the variant, extend `ALL` (display order) and `binary_name`. The compiler flags every other exhaustive `match`.

## 2. Adapter and registry

Create `business/src/harnesses/acme/mod.rs` with a unit struct and `impl Harness`. Start from
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
Self::new(vec![Box::new(Omp), Box::new(Pi)])   // add Box::new(Acme)
```

The registry order is the display order. Nothing else enumerates harnesses by hand: the shell, the `open_external`
link allowlist (each harness's `homepage` and install `docs_url`) and the reveal roots (each harness's `paths`) all
iterate the registry.

### The `Harness` trait

`business/src/harness/mod.rs`. Required methods have no body; the rest have defaults you override only when the CLI
differs.

| Method | Required | Default | Meaning |
| --- | --- | --- | --- |
| `id()` | yes | | The `HarnessId`. |
| `info()` | yes | | `HarnessInfo`: name, short name (`"OMP"`), binary name, description, homepage. |
| `capabilities()` | yes | | `Capabilities` flags that decide which tabs and actions the UI shows. |
| `paths(env, project)` | yes | | `HarnessPaths`: `config_root`, `agent_dir`, `data_root`, `profile`, `project_dir`. |
| `config_view(env, project)` | yes | | Redacted `ConfigView` (user layer, then profiles and project). |
| `extensions_view(env, project)` | yes | | Plugins, skills, MCP servers, themes, ... as `ExtensionsView`. |
| `catalog()` | yes | | The `CommandCatalog`. |
| `launch_spec(request)` | yes | | The RPC command line as a shell-free `ExecSpec`. |
| `dialect()` | yes | | The `&'static dyn RpcDialect`. |
| `sessions_dir(env, project)` | yes | | Folder holding the saved sessions of a project. |
| `install_hint()` | yes | | Copyable install command and docs URL, shown when the CLI is missing (never run). |
| `companion_setup()` | yes | | `CompanionSetup`: how Harness Skills are installed and where the dashboard MCP server is registered. See [Recommended setup](#9-recommended-setup). |
| `binary_name()` | no | `self.id().binary_name()` | Executable name looked up on `PATH`. |
| `version_args()` | no | `["--version"]` | Arguments that print the version. |
| `parse_version(output)` | no | first `x.y.z` (`first_semver`) | Extracts the version. |
| `supported_versions()` | no | `[]` | Version prefixes tested with this app, e.g. `&["18."]`. |
| `version_support(version)` | no | `Supported` on a prefix match, else `Untested` | Never returns `Unknown`; that is detection's "no version" state. |
| `detect(env, runner, override)` | no | `harness::detect` | See [Detection](#3-detection). |
| `summary(...)` | no | info + capabilities + detection | What `harness_list` returns. |
| `list_sessions`, `resolve_session`, `session_tree`, `session_messages`, `session_files_touched` | no | PI-family session files | See [Sessions](#7-sessions). |
| `delete_session` | no | PI-family session files | Deletes a session file with its companion folder and backups. |
| `session_folder(env, project)` | no | `sessions_dir`, not configured | Where the sessions live and whether the project's own settings chose that folder (shown when deleting). |
| `configured_approval_mode(env, project)` | no | `None` | Only for harnesses with `capabilities.approvalModes`. |
| `cli_action(action, project)` | no | `Unsupported` error | See [CLI actions](#8-cli-actions). |
| `cli_failure_line(stdout, stderr)` | no | `None` (exit code decides) | For CLIs that exit 0 on failure. |

Signature examples:

```rust
fn paths(&self, env: &Env, project: Option<&Path>) -> HarnessPaths;
fn launch_spec(&self, request: &LaunchRequest<'_>) -> Result<ExecSpec>;
fn dialect(&self) -> &'static dyn RpcDialect;
fn sessions_dir(&self, env: &Env, project: &Path) -> PathBuf;
```

## 3. Detection

Implement `binary_name` (or rely on `HarnessId::binary_name`), `install_hint`, `paths` and, when the CLI differs,
`version_args`, `parse_version` and `supported_versions`. `detect` then works unchanged:

- It resolves the binary with `resolve_binary`: the user's override (only when it is an absolute existing file whose
  name is exactly the binary name, see `override_name_matches`), then absolute `PATH` entries, then the usual install
  directories (`~/.bun/bin`, `~/.local/bin`, `~/.cargo/bin`, `~/.npm-global/bin`, Homebrew paths on macOS, `/usr/local/bin`). A shell is never asked for `PATH`. Relative `PATH` entries are
  ignored.
- Every spawn of the binary (version probe, CLI actions, RPC launch) appends the binary's own directory and the same
  install directories to the child's `PATH` (`launcher_search_dirs`, `ExecSpec::search_dirs`): an app started from
  the Finder has launchd's `PATH`, and a launcher such as `#!/usr/bin/env bun` must still find its interpreter.
- It runs `<binary> <version_args>` through a `ProcessRunner` with `VERSION_TIMEOUT` (5 s) and a 64 KiB output cap,
  and reports the config directory from `paths(env, None).config_root`.
- Failures become a short secret-free `Detection.error`; a missing binary yields `install_hint`.

Environment variables your `paths` reads must be listed in `CAPTURED_VARS` (`business/src/env.rs`); `Env` captures
nothing else from the process.

## 4. Config and extension views

All file reads go through `SafeReader` (`security::deny`), never `std::fs` directly. `SafeReader` refuses
deny-listed files (databases, logs, keys, credential stores, blob caches), follows symlinks hop by hop, accepts
only regular files, and caps the size (`DEFAULT_MAX_BYTES` = 2 MiB). If your CLI keeps credentials in a file whose
name or extension is not on the list, add it to `DENIED_NAMES`, `DENIED_EXTENSIONS`, `DENIED_PREFIXES` or
`DENIED_DIRS` in `business/src/security/deny.rs` and extend the canary fixture (see [Tests](#10-tests)).

Config values must pass through `Redactor` before they leave the business layer. The `pi_family` helpers already
do this: `read_document` reads through `SafeReader`, `section`/`section_from` redact the parsed value into a
`ConfigSection`, and `source` describes the file and layer.

```rust
// business/src/harnesses/pi/config.rs
use crate::pi_family::{SectionText, optional_section, section, source};
```

If you add a new way to turn an unparsed file into output, redact there too (`Redactor.redact_at` with a
`RedactionContext` keeps key-path rules such as `mcpServers.<name>.env`; `redact_text` masks free text). Rules are
listed in [security-privacy.md](security-privacy.md#secret-redaction).

Extensions: reuse `pi_family::scan_skills`, `parse_servers` (MCP), `themes_in_dir`, `extension_module_paths` and
`package_manifest`; write adapter code only for what is specific (plugin registries, package layers).
`Capabilities` must match what the views fill in.

## 5. Commands

Implement `CommandCatalog` (`business/src/harness/catalog.rs`) and return it from `Harness::catalog`.

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
them (`approvalModes`, `projectTrust`); the manager filters them.

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

## 7. Sessions

`sessions_dir` is required. The default `list_sessions`, `resolve_session`, `session_tree`, `session_messages` and
`session_files_touched` read PI-family session files (`<timestamp>_<id>.jsonl`, entries linked by `id`/`parentId`)
through `pi_family::sessions`. A harness with the same file format only supplies the folder. Helpers:
`pi_dir_name`, `omp_dir_name`, `canonical_text`, `configured_session_dir`. A harness with another format overrides the
five methods. The webview only sends a `SessionRef { harness, id }`; the id is resolved by a shallow scan of the
project's folder, so no other file can be named, and companion directories (subagent transcripts, tool logs) are never
entered.

## 8. CLI actions

Extension changes (install, remove, enable, upgrade) run the harness CLI. Implement `cli_action(action, project)`:
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

`run_cli_action` runs the binary without a shell in the project (or the home directory), with a 5 minute timeout and
capped output, strips ANSI escapes and redacts both streams. See `business/src/harnesses/omp/cli.rs` and
`business/src/harnesses/pi/cli.rs`.

## 9. Recommended setup

**Every newly supported harness must be added to the Recommended setup step** of onboarding (also reachable from
**Run recommended setup** on the Ecosystem screen). The step installs the Harness Skills and registers the Harness
Dashboard MCP server for each detected harness. `Harness::companion_setup` is a required method, so the compiler
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

- `OmpMarketplace`: the CLI has OMP's plugin marketplace. The app runs `marketplace add` and `plugin install` (user
  scope) through `extension_action`, so `cli_action` must support both ([CLI actions](#8-cli-actions)). Only a harness
  with that exact plugin system qualifies.
- `Package { source }`: the CLI has its own package manager (PI's `pi install <source>`). The app installs the source
  for the user through `extension_action` (`PackageInstall`), so `cli_action` must support it. The step reports the
  skills as installed when the harness's user settings list the package (`user_has_package`, in
  `business/src/ecosystem/setup.rs`); extend that if your CLI records packages elsewhere.
- `Manual { command, note }`: the user copies a command; the app never runs it. Use it when you have not verified an
  install command against the CLI yet, and say so in `note` (PI does). The step reports the skills as installed
  when the harness's user settings list the package (`user_has_package`, in `business/src/ecosystem/setup.rs`);
  extend that if your CLI records packages elsewhere.
- `Unsupported { reason }`: the skills do not exist for this harness.

`McpTarget`:

- `UserMcpJson { also_read }`: `<agent_dir>/mcp.json`, where `agent_dir` comes from `paths(env, None)`. The app merges
  `{"command": "harness-dashboard", "args": ["mcp"]}` into its `mcpServers` object (`business/src/ecosystem/mcp_register.rs`).
  Choose it only after confirming that the CLI reads `mcpServers` from that user file. List other files of the agent
  directory that can also define servers in `also_read` (OMP adds `.mcp.json`); they are only read, to find an
  existing entry.
- `Unsupported`: the CLI has no user MCP file the app can write. The step then shows "No MCP file to edit" for it.

The webview sends only a harness id; the file path comes from `paths` and the installer command is fixed in Rust, so
no new command or permission is needed. Add the harness to the setup copy as well: the strings are under
`onboarding.setup.*` in `ui/src/lib/i18n/locales/*/onboarding.ts` (all five languages), and the mock in
`ui/src/lib/ipc/mock/setup-handlers.ts` keeps a per-harness MCP file and skills offer (`Record<HarnessId, ...>`).

## 10. Tests

Add tests next to the existing ones in `business/tests/`. Follow the same pattern for the new harness.

| Test | Where | What to cover |
| --- | --- | --- |
| Detection | `detection.rs` | Binary lookup, override validation, version parsing and support, install hint. |
| Paths | `harness_paths.rs`, `session_paths.rs` | `paths` and `sessions_dir` for default and overridden env. |
| Views | `omp_views.rs`, `pi_views.rs` | Snapshot (`insta`) of `config_view` and `extensions_view` over a fixture home `business/tests/fixtures/acme-home/` and the demo project. |
| Leaks | `canary.rs`, `adapter_canary.rs`, `safe_reader.rs` | Hide `CANARY-7f3a9c` in deny-listed files and secret values of your fixture home; no view may contain it. `adapter_canary.rs` iterates every registered harness; add your fixture home to its environments. |
| Catalog | `catalog.rs` | Native, live and shared merge; every command's invocation. |
| Dialect | `rpc_dialects.rs`, `rpc_commands.rs`, `rpc_normalize.rs` | Encoded frames per action, decoded recorded frames, handshake. |
| Sessions | `sessions_store.rs` | Listing, tree and transcript over fixture sessions in `business/tests/fixtures/sessions/acme/`. |
| CLI actions | `cli_actions.rs` | Argument construction, validation errors, dry run, failure line. |
| Chat engine | `agent_session.rs`, `agent_manager.rs`, `agent_dialogs.rs`, `agent_failures.rs` | The real session engine against `fake_agent`. |
| Recommended setup | `setup_status.rs`, `setup_mcp_register.rs` | `setup_status`: skills and MCP state of your harness on an empty machine, with the skills installed and with the MCP entry present (build the fixture files in a temp home). `setup_mcp_register.rs`: the `mcp.json` merge for your `also_read` files. The installer itself is harness-independent (`setup_installer.rs`). |
| Fixtures | `mock_fixtures.rs` | Add the harness's views and catalog to the exported mock fixtures. |

**Fake agent.** `business/src/bin/fake_agent.rs` plays a JSONL scenario: `{"emit": {...}}` writes a frame,
`{"expect": "get_state", "reply": {...}}` waits for a request of that type and answers it, `{"exit": 1}`,
`{"hang": true}`, `{"stderr": "..."}`. Add scenarios under `business/tests/fixtures/streams/` (for example
`acme_startup.jsonl`, `acme_basic.jsonl`) and start them with `ScenarioSpawner` from
`business/tests/agent_support/mod.rs`. `--dialect omp|pi|mcp` is a command-line switch; it only changes the opening
(the OMP `ready` frame is hard-coded for `omp`). If your handshake needs another opening frame, extend `main` in
`fake_agent.rs`. The shell's agent command tests in `src-tauri` need the binary too; see
[architecture.md](architecture.md#testing).

Run `cargo test -p harness_business`. `cargo test -p harness_business export_` regenerates the bindings and mock
fixtures; commit the result, because CI fails on a difference.

## 11. UI touch points

The UI renders any harness from data, with these exceptions:

- **Bindings.** `HarnessId.ts` and the other generated types update with `export_` (never hand-edit).
- **Badge colour.** `ui/src/lib/components/Badge.svelte` has `HARNESSES: Record<HarnessId, string>` mapping a harness
  to the classes `bg-<id>-wash text-<id>`. The type check fails until you add the entry. Define the tokens
  `--harness-<id>` and `--harness-<id>-wash` for both themes in `ui/src/lib/design/tokens.css`, and the
  `--color-<id>` and `--color-<id>-wash` aliases in `ui/src/app.css`.
- **Mock.** `ui/src/lib/ipc/mock/fixtures.ts` builds `Record<HarnessId, ...>` entries for config, extensions,
  commands, session info and session lists. Add the exported fixtures there. `harness_list.json` and
  `project_recent.json` are synthetic (`business/tests/mock_fixtures.rs`); add the new harness summary.
- **Setup step.** The mock's per-harness maps in `ui/src/lib/ipc/mock/setup-handlers.ts` (MCP file, skills offer) and
  the `onboarding.setup.*` copy; see [Recommended setup](#9-recommended-setup).
- **Copy.** Strings that name the supported harnesses (`shortDescription` and `longDescription` in
  `src-tauri/tauri.conf.json`, the README, the views' empty-state text in `ui/src/features/shell/views.ts`) are
  written by hand.

Then run `bun run check`, `bun run test` and `bun run guard`.
