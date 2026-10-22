# Claude Code support — research and plan (resumable)

Issue #20. Research 2026-10-22 against Claude Code **2.1.304** (pacman `claude-code`, `/usr/bin/claude` wrapping
`/opt/claude-code/bin/claude`) and the TypeScript Agent SDK `@anthropic-ai/claude-agent-sdk` **0.3.308** (its
`sdk.d.ts` is the protocol reference, line numbers below point into it). Status: **plan approved 2026-10-22 (all decisions as proposed), branch `feat/claude-code`, one commit per
step.** The Status table says where to resume.

Claude Code moves fast (a release every few days). Before resuming, re-check `claude --version`, the SDK version and
the unknowns in [To confirm by recording](#to-confirm-by-recording).

## Goal

Claude Code becomes the fourth harness with the same parity as OMP, PI and OpenCode: detection, onboarding wizard,
Config and Extensions views (plugins, marketplaces, skills, agents, commands, MCP servers), Sessions (list, open,
touched files, export, delete, resume, fork), plugin and marketplace actions, Share setup, Recommended setup (Harness
Skills, dashboard, memory and desktop MCP servers), native chat (streaming, tool cards, approvals and approval
modes, model and thinking pickers, slash commands incl. skills and plugin commands, steering, follow-up queue,
remove queued, abort, new / resume / fork / clone / rename / compact, HTML export, stats and context meter, copy last
reply, `!command`, images, AI naming, question dialogs), hooks (#16), team hooks (#26), routines (#14) and the
notes guard (#39).

Not in scope: a login flow of our own (see [Terms](#terms)), Claude's background sessions (`claude rm`, remote
control), live subagent transcripts in the chat (task events only, like OpenCode), output styles and themes as
extension kinds, project or local scope plugin installs, Bedrock / Vertex / Foundry specific UI.

## Terms

From code.claude.com/docs/en/legal-and-compliance and /agent-sdk/overview, fetched 2026-10-22:

- Third-party developers may not offer claude.ai login in their products, route requests through Free / Pro / Max
  credentials on behalf of users, or "collect, store, or intermediate Claude.ai credentials or session tokens".
- Explicitly allowed: "an end user … signing in to the unmodified Claude Code binary with their own Claude
  subscription". The binary must not be modified and no built-in authentication method may be removed, disabled or
  restricted.
- Branding: our product or agent must not be named "Claude Code". Saying the app "runs Claude Code" in plain text is
  fine.

What the app does, and must keep doing: it spawns the user's own installed, unmodified `claude`, which uses whatever
login the user made through Anthropic's own flow. The app never offers a login, never reads `.credentials.json` or the
Keychain, never passes `--bare` or anything that strips auth methods, and shows "sign in from a terminal"
(`claude` then `/login`) when the CLI reports no login, as it already does for OMP and PI. `system/init.apiKeySource`
tells the app which kind of login is in use without touching a credential.

## Protocol

**Chat runs over the CLI's stream-json mode with the control protocol**, the same interface the Agent SDK wraps. Not
ACP (issue #27 says Claude Code is better served natively). One long-lived process per chat:

```
claude --output-format stream-json --verbose --input-format stream-json \
       --permission-prompt-tool stdio --include-partial-messages \
       [--resume <id> [--fork-session [--resume-session-at <uuid>]]] [--session-id <uuid>] \
       [--permission-mode <mode>] [--model <m>]
```

- **Handshake:** the host sends `control_request{subtype:"initialize", hooks, agents, title, supportedDialogKinds…}`
  (4650). The reply (4732) carries `commands` (slash commands with description, argumentHint, aliases, builtin),
  `agents`, `models`, `account`, `output_style`, `claude_code_version`, `pending_permission_requests`. Each turn starts
  with `system/init` (6167): session_id, model, cwd, tools, mcp_servers, permissionMode, slash_commands, skills,
  plugins, effort, `capabilities[]` (feature flags such as `interrupt_receipt_v1`), `apiKeySource`.
- **Input:** `{type:"user", message:{role:"user", content}, uuid, priority:"now"|"next"|"later", shouldQuery}` (6508),
  plus `control_response` and `control_cancel_request`.
- **Output** (`SDKMessage`, 5556): `assistant`, `user` (tool results), `stream_event` (partial deltas), `result`
  (exactly one per turn: `success` or `error_*`, with `total_cost_usd`, `usage`, `modelUsage`, `num_turns`,
  `duration_ms`), `system/*` (init, status `compacting`, compact_boundary, local_command_output, commands_changed,
  task_started / progress / updated / notification, session_state_changed `idle|running|requires_action`, hook_*,
  api_retry, permission_denied, notification), `conversation_reset`, `keep_alive`.
- **CLI → host requests:** `can_use_tool` (4991), `hook_callback` (5410), `request_user_dialog` (5234),
  `elicitation`, `mcp_message` (only for host-hosted MCP servers, unused).
- **Host → CLI requests** (5205): `interrupt{cancel_queued}`, `cancel_async_message{message_uuid}`, `stop_task`,
  `set_permission_mode`, `set_model`, `set_max_thinking_tokens`, `apply_flag_settings{effortLevel}`,
  `rename_session{title, source:"host"}`, `get_context_usage`, `get_session_cost`, `list_models`, `reload_skills`,
  `reload_plugins`, `mcp_status`, `rewind_files`, `get_settings`. Each gets one `control_response` with
  `subtype: success|error` and the `request_id`.

**Fit:** requests carry ids, but user messages do not and turns end with `result`, not a response frame. It does not
fit `RpcDialect`, so it gets its own engine (`ChatProtocol::ClaudeStream`), built on `AgentProcess`, the sink, the
coalescer and slots like OpenCode 2. It is a stdio engine, so no HTTP client, no password, no orphaned servers.

### Parity map

| Item | Mechanism | Confidence |
| --- | --- | --- |
| Prompt, images | user frame with text and image content blocks | d.ts, docs |
| Steering | user frame, `priority:"next"`: read in the same turn after the running tool calls finish | docs |
| Follow-up queue | `priority:"later"`: held until the turn ends, sent as a new turn (queued sends may merge into one turn, 5922) | docs |
| Remove queued | `cancel_async_message{message_uuid}`, so every send sets `uuid` | d.ts 3959 |
| Abort | `interrupt` (`cancel_queued:true` also drops the queue) | d.ts 4799 |
| New session | new process (fresh `--session-id`) | d.ts |
| Resume | `--resume <id>` | d.ts, docs |
| Fork at message | `--resume <id> --fork-session --resume-session-at <uuid>` (fork point = last entry of the kept turn) | d.ts 2135–2180 |
| Clone | `--resume <id> --fork-session` without a cut | d.ts |
| Rename | `rename_session` live, a `custom-title` JSONL line offline | d.ts 3316 |
| Compact | user frame `/compact <instructions>`, then `status:compacting` and `compact_boundary{pre_tokens, post_tokens}` | docs |
| Stats, context meter | `result` totals plus `get_context_usage` (percentage, max tokens) and `get_session_cost` | d.ts 5905–6000, 4023 |
| Model / thinking | `set_model` (list from initialize `models`), `apply_flag_settings{effortLevel}` (low…max) | d.ts |
| Approval prompts | `can_use_tool` → `{behavior:"allow", updatedInput?, updatedPermissions?}` or `{behavior:"deny", message}`. "Always" echoes `permission_suggestions` with destination `session` | d.ts 2555, docs |
| Question dialogs | `can_use_tool` for `AskUserQuestion`, answered as allow with `updatedInput.answers`; also `request_user_dialog`, `elicitation` | docs, sdk-tools.d.ts |
| Slash commands | initialize `commands`, `system/init.slash_commands`, live via `system/commands_changed`, `reload_skills` / `reload_plugins` | d.ts 9655, 3811 |
| Subagents | `task_*` system events, `parent_tool_use_id` on frames | d.ts 6254–6390 |
| `!command` | no native frame. The app runs the command (project cwd, same env, shared `!command` output paging), then sends a user frame with `shouldQuery:false` holding `<bash-input>` / `<bash-stdout>` / `<bash-stderr>` so the model sees it next turn and resume shows it | inferred, d.ts 2166 |
| History, export, last reply | the session JSONL (shared reader with the Sessions view), HTML through the shared `export_html` | d.ts 920, local files |
| AI naming | separate short process on the user's default model, like OMP and PI (`utility_args`, no model flag): `--tools "" --thinking disabled --no-session-persistence`, one prompt, then exit | inferred |
| Hooks, routines, notes guard | app-side and harness-agnostic, only need `ToolEnd` with touched paths and settled turns | repo |

## Files and CLI facts

- **Install:** native installer (`curl -fsSL https://claude.ai/install.sh | bash`, `irm https://claude.ai/install.ps1 |
  iex`; launcher `~/.local/bin/claude`), Homebrew `claude-code`, WinGet `Anthropic.ClaudeCode`, npm
  `@anthropic-ai/claude-code`, apt / dnf / apk. `--version` prints `2.1.304 (Claude Code)`, so `first_semver` works.
- **Spawn env:** `DISABLE_UPDATES=1` on every spawn, like OpenCode's auto-update switch.
- **Env:** `CLAUDE_CONFIG_DIR` moves `~/.claude` and the global file (`$CLAUDE_CONFIG_DIR/.claude.json`) → captured.
  Presence-only: `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_OAUTH_TOKEN`,
  `CLAUDE_CODE_OAUTH_REFRESH_TOKEN`, `ANTHROPIC_CUSTOM_HEADERS`, `AWS_BEARER_TOKEN_BEDROCK`, `ANTHROPIC_FOUNDRY_*`.
  Shown as "set": `CLAUDE_CODE_USE_BEDROCK` / `_VERTEX` / `_FOUNDRY`.
- **Settings layers** (strict JSON, highest first): managed (`/etc/claude-code/managed-settings.json` and
  `managed-settings.d/`, `/Library/Application Support/ClaudeCode/`, `C:\Program Files\ClaudeCode\`), `--settings`,
  `.claude/settings.local.json`, `.claude/settings.json`, `~/.claude/settings.json`. Secrets: `env`, `apiKeyHelper`,
  `otelHeadersHelper`, hook `headers`, `pluginConfigs`.
- **`~/.claude.json`** (≈110 KB, rewritten by every running claude, backups in `~/.claude/backups/`): top-level
  `mcpServers` (user scope), `projects[<abs path>]` (local-scope `mcpServers`, `allowedTools`,
  `enabledMcpjsonServers`, `hasTrustDialogAccepted`, …), and private keys `oauthAccount`, `userID`, `machineID`,
  `githubRepoPaths`. The app reads only `mcpServers` and the current project's entry, redacted, and never writes it.
- **MCP:** precedence local > project (`.mcp.json`) > user > plugin. Stdio entry `{"type":"stdio","command","args","env"}`.
  Writes go through `claude mcp add-json --scope user <name> '<json>'` and `claude mcp remove --scope user <name>`.
  Never `claude mcp list` / `get` (they start the servers).
- **Plugins:** `~/.claude/plugins/installed_plugins.json` is the same v2 shape OMP reads ("Claude-compatible", reuse
  the parser). `known_marketplaces.json` is an object keyed by name with a `source` object and `installLocation`
  (new reader; catalogs in `<installLocation>/.claude-plugin/marketplace.json`, close to OMP's). Enabled state is
  `enabledPlugins` merged across settings layers. CLI: `claude plugin install|uninstall|enable|disable|update <id>
  -s user`, `claude plugin marketplace add|remove|update`, `--json` for the outcome, no dry run.
- **Skills / agents / commands:** `~/.claude/{skills/<n>/SKILL.md, agents/*.md, commands/*.md}`, the same under
  `.claude/` in the project, and in plugins (namespaced `plugin:name`).
- **Sessions:** `~/.claude/projects/<cwd, every non-alphanumeric → '-'>/<uuid>.jsonl`, plus `<uuid>/subagents/` and
  `<uuid>/tool-results/`. Message lines have `uuid`, `parentUuid`, `isSidechain`, `sessionId`, `cwd`, `gitBranch`,
  `timestamp`, `message.content[]`. Titles: last `custom-title`, else last `ai-title`. Compaction: `system /
  compact_boundary` + a user line with `isCompactSummary`. Touched files: `tool_use` Write / Edit / MultiEdit
  (`file_path`), NotebookEdit (`notebook_path`). Unknown line types are ignored. Delete = the `.jsonl`, the `<uuid>/`
  folder, `file-history/<uuid>/`, `session-env/<uuid>/`; refused while `~/.claude/sessions/<pid>.json` names the
  session as running.
- **Deny list additions:** `.credentials.json` (exact name), `history.jsonl`, `paste-cache/`, `shell-snapshots/`,
  `file-history/`, `session-env/`, `backups/`, `.claude.json.backup*`, `remote-settings.json`, `policy-limits*.json`,
  `stats-cache.json`, `mcp-needs-auth-cache.json`, `statsig/`, `debug/`, `uploads/`, `image-cache/`, `feedback*/`,
  `chrome/`, `projects/*/tool-results/`, `plugins/data/`, `*.heapsnapshot`.
- **Harness Skills:** a Claude plugin already. `claude plugin marketplace add MSpiechowicz/harness-useful-skills
  --scope user`, then `claude plugin install useful-skills@useful-skills-local --scope user` (harness-useful-skills
  README 64–78). It brings its own MCP server and hooks.
- **Dashboard:** already tracks Claude (`dashboard_args.rs:62`), and its palette has `claude: 'orange'`
  (`ui/src/lib/dashboard/palette.ts:46-60`).

## Decisions

Approved by the user on 2026-10-22 as proposed ("do it all in one separate branch"). Decisions 3–8 may still change
before the steps that use them (S5, S9, S12).

1. **Scope:** full parity on one branch `feat/claude-code` (like OpenCode 2), one commit per step, chat marked
   "Preview" until the user's real check passes.
2. **Real-traffic recording (user, 2026-10-22: default model):** record the stream with the user's default model on
   the user's own login, so thinking and effort events are covered too. It runs in a scratch project under the
   session scratchpad (sessions land in `~/.claude/projects/<scratch slug>/` and are deleted afterwards). Recordings
   are scrubbed (`account`, emails, org ids) before they become fixtures. Cost: a few cents, or a small part of the
   subscription limit.
3. **Approval modes:** the app's three modes map to Claude as Always ask → `default` plus an injected `PreToolUse`
   hook (via initialize) that answers `ask` for every tool, Write → `default`, Yolo → not offered as an override (it
   needs `--allow-dangerously-skip-permissions`, which the forbidden-pattern guard rejects). Claude's own
   `permissions.defaultMode` (incl. `acceptEdits`, `plan`, `auto`, `bypassPermissions`) is shown read-only in the
   chat header as the configured mode. Onboarding gets an approval notice only when that configured mode is
   `bypassPermissions` or `auto`.
4. **MCP registration:** new `McpTarget::ClaudeCli`: read `~/.claude.json` to find entries, write only through `claude
   mcp add-json / remove --scope user`. The dashboard, memory and desktop MCP servers and Share setup's MCP import all
   use it.
5. **Harness Skills:** generalize `SkillsStrategy::OmpMarketplace` into `Marketplace { source, plugin }` and use it
   for Claude with `MSpiechowicz/harness-useful-skills` and `useful-skills@useful-skills-local`.
6. **AI naming (user, 2026-10-22):** utility process as for OMP and PI: no model flag (the user's default model, so
   model restrictions in managed settings and Bedrock / Vertex aliases keep working), `--tools "" --thinking disabled
   --no-session-persistence`, one prompt.
7. **Naming:** harness id `claude`, name "Claude Code", short "CC", orange tokens from the dashboard palette (check the
   contrast with OMP's amber in both themes).
8. **Settings sources:** chats load the user's normal sources (user, project, local, managed), so the chat behaves as
   in the terminal, including the user's own Claude hooks. App hooks stay app-side, so nothing runs twice.

## Architecture

- `HarnessId::Claude` (`"claude"`), `harnesses/claude/` with `mod.rs`, `paths.rs`, `config.rs`, `extensions/{plugins,
  marketplaces,mcp,agents}.rs`, `catalog.rs`, `sessions/{mod,list,transcript,delete}.rs`, `cli.rs`, `launch.rs`,
  `share.rs`.
- `ChatProtocol::ClaudeStream`, `AgentSession::Claude(ClaudeSession)`.
- `business/src/claude_stream/`: pure code. `wire/` (serde types for frames and control requests), `mapper/`
  (frames → `AgentEvent`: deltas, tool cards with native tool ids, `ToolEnd` touched paths, task events, results,
  compaction, quiet `!command` runs), `tools.rs` (tool ids and paths), `permissions.rs` (mode mapping, suggestion
  echo), `stats.rs`.
- `business/src/agent/claude/`: engine. `startup.rs` (spawn, initialize, ready), `io.rs` (NDJSON reader, request id
  table for control replies, timeouts), `prompts.rs` (prompt, steer, follow-up, remove queued, abort), `dialogs.rs`
  (approvals, questions, cancels), `settings.rs` (model, thinking, mode, rename), `ops.rs` (new, resume, fork,
  clone), `runs.rs` (`!command`, compaction), `reports.rs` (stats, last reply, export), `history.rs`.
- Fake agent: `business/src/bin/fake_agent/claude.rs`, `--dialect claude`, replaying recorded scenarios and answering
  control requests.
- Every file stays under 500 lines (`bun run guard`).

## Steps

Order: S1 → (Track A: S2 → S3 → S5 → S6 ∥ Track B: S4 ∥ Track C: S0 → S7 → S8 → S9 → S10) → S11 → S12 → S13.

- **S0 recording** (main session, needs decision 2): recorder script in the scratchpad covering every item of
  [To confirm by recording](#to-confirm-by-recording); scrubbed recordings to `business/tests/fixtures/claude/stream/`.
- **S1 skeleton and detection:** `HarnessId::Claude`, registry, `harnesses/claude/mod.rs` (info, capabilities with
  `chat` false until S10, `supported_versions` `2.1.`, install hint per OS, `spawn_env` `DISABLE_UPDATES=1`), `paths`,
  `env.rs` (`CLAUDE_CONFIG_DIR` captured, secrets presence-only), `security/deny.rs` additions with canary fixtures,
  `ChatProtocol::ClaudeStream` and an `AgentSession` arm returning `Unsupported`, doc-only module stubs. Tests:
  `detection.rs`, `harness_paths.rs`, `canary.rs`, `adapter_canary.rs`, `model.rs`.
- **S2 config view** (A): the five settings layers, managed drop-ins, `~/.claude.json` user and project MCP entries,
  CLAUDE.md files listed (names only), presence notices for env. Fixture home `business/tests/fixtures/claude-home/`,
  snapshot test `claude_views.rs`.
- **S3 extensions view** (A): plugins (shared v2 parser from `omp/plugins.rs` moved to `pi_family` or a shared
  module, `enabledPlugins` merge), marketplaces (new reader, catalog entries and update marks reused), skills, agents
  (`capabilities.agents`), commands as prompt templates, MCP servers (user, local, project, plugin) with redaction.
- **S4 sessions** (B): list (title rules, times, message count), resolve, tree (`parentUuid`, sidechains skipped,
  compaction boundaries), transcript, files touched, delete with the running check. Fixtures
  `business/tests/fixtures/sessions/claude/` (synthetic, shaped after the local structure, no real content). Tests
  `sessions_claude*.rs`.
- **S5 CLI actions and Recommended setup** (A): `cli.rs` plugin and marketplace actions with `--json`,
  `SkillsStrategy::Marketplace`, `McpTarget::ClaudeCli` (read `~/.claude.json`, write through the CLI), memory and
  desktop MCP registration through it, setup status. Tests `claude_cli_actions.rs`, `setup_status.rs`,
  `setup_mcp_register_claude.rs` (recording `ProcessRunner`).
- **S6 Share setup** (A): marketplaces, plugins and MCP servers; import of MCP servers through `claude mcp add-json`.
  Tests `sync_export_claude.rs`, `sync_import_claude.rs`.
- **S7 wire and mapper** (C, after S0): `claude_stream/{wire,mapper,tools,stats}` from the recordings. Tests
  `claude_wire.rs`, `claude_mapper.rs`.
- **S8 engine core** (C): startup, initialize, prompt, streaming, steer, follow-up, remove queued, abort, model,
  thinking, slash commands, resume, crash and no-login paths; fake agent dialect. Tests `claude_engine*.rs`.
- **S9 approvals and modes** (C): `can_use_tool` dialogs (once / always / deny), `AskUserQuestion`, `request_user_dialog`,
  `control_cancel_request`, mode mapping incl. the ask-all hook, configured mode read-only.
- **S10 ops** (C): new, fork, clone, rename, compact (with instructions, `compactInstructions` true), stats and
  context meter, export, last reply, history, `!command`, images, AI naming; set `chat` and `chatPreview` true.
- **S11 hooks, routines, notes guard:** tool ids and touched paths for hooks (`Edit`, `Write`, `MultiEdit`,
  `NotebookEdit`), routines parse arm and the MCP tool enum built from `HarnessId::ALL` instead of the hardcoded
  `["omp","pi","opencode"]` (`routines/format/action.rs:63-68`, `hooks_mcp/routine_tools.rs:223-224`),
  `memoryChatGuards` once S0 shows `HARNESS_MEMORY_SESSION` reaches stdio MCP children. Tests
  `src-tauri/src/hooks/tests_claude.rs`, `src-tauri/src/routines/tests_agent_claude.rs`.
- **S12 UI:** badge and tokens (`Badge.svelte`, `HarnessBadge.svelte`, `tokens.css`, `app.css`), onboarding card and
  approval notice variant, i18n in all five locales, mock fixtures and handlers, `tauri.conf.json` descriptions,
  README table. Copy rules: no semicolons, "Harness family".
- **S13 docs and wrap-up:** `adding-a-harness.md` (Claude as the stdio-engine reference, fix the fit-check
  paragraph), `security-privacy.md` (Claude chat section, terms, files never read), `hooks-format.md` tool table,
  `routines-format.md`, this plan's status table and real-provider checklist. Then reviews (max 3 correctness + 3
  security passes), `cargo clean`, hand-over.

## Status

| # | Step | Status |
| --- | --- | --- |
| S0 | Recording | in progress |
| S1–S13 | | not started |

## To confirm by recording

1. The exact `control_response` shape for `can_use_tool` (camelCase `PermissionResult` assumed).
2. Whether a user frame may precede the initialize reply, and whether `system/init` repeats every turn.
3. `--permission-mode default` vs `manual` (CLI help lists `manual`, the SDK type says `default`).
4. Typed `!cmd` in stream-json, and how a `shouldQuery:false` bash append shows on resume.
5. When `--fork-session` assigns the new id (spawn or first prompt), and how the fork is linked on disk.
6. `/compact <instructions>` and the order `status` → `compact_boundary` → `result`.
7. `priority:"next"` / `"later"`, the `uuid` echo and `cancel_async_message`.
8. `total_cost_usd` and `get_session_cost` on a subscription login.
9. `commands_changed` after a skill or plugin is added mid-session.
10. Whether the workspace trust dialog blocks a stream-json run in an untrusted folder (decides `projectTrust`).
11. Whether stdio MCP children inherit `HARNESS_MEMORY_SESSION` (decides `memoryChatGuards`).
12. `AskUserQuestion` multi-select answer format, and the no-login error text.

## Risks

- **Internal formats:** the session JSONL and many frame fields are internal and change between releases. Unknown
  line and frame types are ignored, recordings pin the tested version, and `supported_versions` marks newer minors as
  Untested.
- **`~/.claude.json` races:** never written by the app; the CLI does it.
- **Cost:** every chat turn and AI naming bills the user's login. AI naming runs one prompt with no tools and no
  thinking.
- **Terms:** see [Terms](#terms). Any feature that would touch credentials or offer a login is out of scope.
- **Colour:** Claude orange next to OMP amber may read as the same harness in badges.
