# Codex support — research and plan (resumable)

Issue #21. Research 2026-10-22 against Codex CLI **0.154.0** (`~/.local/bin/codex`; `~/.codex/version.json` already
announces 0.156.0). Protocol reference: the bindings the CLI generates itself, `codex app-server generate-ts` and
`generate-json-schema` (regenerate on every version bump, they carry no version header). Status: **implementation complete 2026-10-23 on branch `feat/codex`** (approved 2026-10-22, decision 11 added by the
user). Reviews 3/3 + 3/3 used. Nothing pushed. Next: the user's [Real-provider checklist](#real-provider-checklist).

Codex moves fast (weekly releases, rollout formats differ between versions: a sample session was written by 0.146.0).
Before resuming, re-check `codex --version` and regenerate the bindings.

## Goal

Codex becomes the fifth harness with the same parity as Claude Code: detection, onboarding wizard, Config and
Extensions views (skills, plugins, marketplaces, MCP servers, rules, AGENTS.md), Sessions (list, open, touched files,
export, delete, resume, fork), plugin and marketplace actions, Share setup, Recommended setup (dashboard, memory and
desktop MCP servers; Harness Skills see decision 6), native chat (streaming, tool cards, approvals and approval modes,
model and reasoning pickers, slash commands incl. skills, steering, follow-up queue, remove queued, abort, new /
resume / fork / clone / rename / compact, HTML export, stats and context meter, copy last reply, `!command`, images, AI
naming, question dialogs), hooks (#16), team hooks (#26), routines (#14) and the notes guard (#39).

Not in scope: a login flow of our own, Codex Cloud tasks (`codex cloud`), the shared app-server daemon and remote
control (`codex agents`, `remote-control`), `codex review` as its own view, realtime / voice, the ChatGPT desktop app's
bundled codex (`/usr/lib/chatgpt/resources/codex`), project-scope plugin installs.

## Terms and credentials

Codex is Apache-2.0. The app spawns the user's own `codex` on PATH, which uses whatever login the user made with
`codex login`. The app never offers a login, never reads `~/.codex/auth.json` (already denied by name in
`business/src/security/deny.rs:36`), never answers `account/chatgptAuthTokens/refresh`, and asks `account/read` only
for "is there a login and which kind" (`apiKey | chatgpt | amazonBedrock`, `requiresOpenaiAuth`). No login → "sign in
from a terminal" (`codex login`), as for the other harnesses. `config.toml` can hold secrets (`mcp_servers.*.env`,
`http_headers` with literal Authorization values): views mask them, Share setup exports names only.

## Protocol

**Chat runs over `codex app-server` (JSON-RPC over stdio, the default `--listen stdio://`)**, the interface the VS Code
extension and the ChatGPT app use. Not `codex exec --json` (one turn per process, no approvals), not ACP (#27). One
long-lived process per chat:

```
codex app-server [-c key=value …]
```

- **Handshake:** `initialize{clientInfo{name,title,version}, capabilities{experimentalApi?,
  optOutNotificationMethods}}` → `{userAgent, codexHome, platformFamily, platformOs}`, then the `initialized`
  notification. Frames are JSON-RPC without the `jsonrpc` field: requests `{id, method, params}`, responses `{id,
  result|error}`, notifications `{method, params}`.
- **Threads:** `thread/start` (cwd, model, approvalPolicy, sandbox, …), `thread/resume`, `thread/fork`,
  `thread/name/set`, `thread/compact/start`, `thread/list`, `thread/read`, `thread/turns/list`, `thread/items/list`,
  `thread/archive`, `thread/delete`, `thread/rollback` (deprecated, history only, never files).
- **Turns:** `turn/start{input[], model?, effort?, approvalPolicy?, sandboxPolicy?}`, `turn/steer{expectedTurnId,
  input}`, `turn/interrupt`. Input items: text, `localImage{path}`, `image{url}`, `skill`, `mention`.
- **Notifications:** `turn/started`, `turn/completed{status: completed|interrupted|failed}`, `item/started` /
  `item/completed` (ThreadItem: userMessage, agentMessage, reasoning, commandExecution, fileChange, mcpToolCall,
  webSearch, contextCompaction, …), `item/agentMessage/delta`, `item/reasoning/textDelta` / `summaryTextDelta`,
  `item/commandExecution/outputDelta`, `item/fileChange/patchUpdated`, `turn/diff/updated`, `turn/plan/updated`,
  `thread/tokenUsage/updated`, `thread/name/updated`, `thread/status/changed`, `thread/queue/changed`,
  `hook/started|completed`, `account/updated`, `account/rateLimits/updated`, `error{willRetry}`.
- **Server → client requests:** `item/commandExecution/requestApproval` (accept | acceptForSession |
  acceptWithExecpolicyAmendment | decline | cancel), `item/fileChange/requestApproval` (accept | acceptForSession |
  decline | cancel), `item/permissions/requestApproval` (scope turn | session), `item/tool/requestUserInput`
  (experimental, question dialog), `mcpServer/elicitation/request` (form | url), `item/tool/call` (dynamic tools, we
  register none → error), `account/chatgptAuthTokens/refresh` (always refused).
- **Other:** `model/list` (supportedReasoningEfforts, defaultReasoningEffort, inputModalities), `skills/list`,
  `plugin/list|install|uninstall`, `marketplace/add`, `mcpServerStatus/list`, `hooks/list`, `config/read`,
  `account/read`, `account/rateLimits/read`, `thread/shellCommand`, `command/exec`.

**Fit:** id-correlated requests and notifications fit the transport pieces (`FrameReader`, `PendingRequests`), not
`RpcDialect` (its handshake is ReadyFrame/Probe only, `encode_ui_response` has no method context, decoding assumes a
`type` field). So Codex gets its own engine, `ChatProtocol::CodexAppServer`, like the Claude engine: built on
`AgentProcess`, the sink, the coalescer and slots, a pure mapper module `codex_rpc/` (frames → `AgentEvent`) and an
engine `agent/codex/`. Stdio only: no ports, no password, no daemon.

### Parity map

| Item | Mechanism | Confidence |
| --- | --- | --- |
| Prompt, images | `turn/start` with text + `localImage` items (images saved to the chat's attachments dir) | bindings |
| Steering | `turn/steer{expectedTurnId}` while a turn runs | bindings |
| Follow-up queue | app-side queue, sent as `turn/start` after `turn/completed` (decision 4) | repo pattern |
| Remove queued | app-side, nothing reaches Codex | repo pattern |
| Abort | `turn/interrupt` → `turn/completed{interrupted}` | bindings |
| New / resume | `thread/start` / `thread/resume{threadId}` (excludeTurns, history via `thread/turns/list`) | bindings |
| Fork at message | `thread/fork{threadId, …}` `lastTurnId` keeps turns up to it | S0 |
| Clone | `thread/fork` without a cut | bindings |
| Rename | `thread/name/set`, live echo `thread/name/updated` | bindings |
| Compact | `thread/compact/start`, `contextCompaction` item | bindings |
| Stats, context meter | `thread/tokenUsage/updated{modelContextWindow}` | S0 |
| Model / reasoning | per turn `model` + `effort` on `turn/start`, list from `model/list` | bindings |
| Approval prompts | the three server requests above → the app's approval card; "Always" = `acceptForSession` | bindings |
| Question dialogs | `item/tool/requestUserInput`, `mcpServer/elicitation/request` (form only, url → declined with a notice) | bindings, experimental |
| Slash commands | app built-ins (new, compact, rename, model, …) + skills from `skills/list` sent as `skill` input items | bindings |
| Subagents | collab/subagent items shown as task cards (S0) | verify |
| `!command` | decision 5 | — |
| History, export, last reply | `thread/turns/list` + `thread/items/list`, HTML through the shared `export_html` | bindings |
| AI naming | short `codex exec --ephemeral`-style utility process without a model flag (S0: confirm flags), like OMP/PI/Claude | verify |
| Hooks, routines, notes guard | app-side, need `ToolEnd` with touched paths (fileChange `changes[].path`, commandExecution) and settled turns | repo |

## Files and CLI facts

- Home: `$CODEX_HOME` or `~/.codex`. Add `CODEX_HOME` to `CAPTURED_VARS` and `OPENAI_API_KEY` / `CODEX_API_KEY` to
  `PRESENCE_VARS` (`business/src/env.rs`).
- `config.toml` (TOML): top level `model`, `model_reasoning_effort`, `approval_policy`, `sandbox_mode`,
  `service_tier`; `[mcp_servers.<n>]` command/args/env/url/http_headers/enabled/timeouts and per-tool
  `approval_mode`; `[projects."<path>"] trust_level`; `[sandbox_workspace_write]`; `[marketplaces.<n>]`;
  `[plugins."<name>@<marketplace>"] enabled`; `[features]`; `[tui.*]`, `[desktop]`, `[notice]`.
- Sessions: `sessions/YYYY/MM/DD/rollout-<ISO>-<uuid>.jsonl` (types session_meta, event_msg, response_item,
  turn_context), titles in `session_index.jsonl`, plus sqlite stores (`state_5`, `thread_history_1`, `queue_1`). The
  app does not parse these: it uses the app-server (decision 3).
- Skills: `~/.codex/skills/<name>/SKILL.md` (bundled under `skills/.system/`), plugin skills via `skills/list`.
- Plugins: `codex plugin add|list|remove|marketplace`, state in `config.toml`.
- MCP: `codex mcp add|remove|list|get` (writes `config.toml` itself).
- Rules: `~/.codex/rules/*.rules` (`prefix_rule(pattern, decision)`), read-only in Extensions.
- Instructions: `~/.codex/AGENTS.md` and project `AGENTS.md`.
- Features: `codex features list`; `hooks` stable/on, `in_app_updates` stable/on (decision 8).

## Decisions (proposed)

1. **Scope:** full parity on one branch `feat/codex`, one commit per step, chat marked "Preview" until the user's
   real check passes. Same flow as Claude: S0 recording, worktree tracks, 3 correctness + 3 security passes.
2. **Approval modes:** Always ask → `approvalPolicy: untrusted` + `workspace-write`; Write → `on-request` +
   `workspace-write` (Codex's default); Yolo → `never` + `danger-full-access`, hidden unless `yoloMode` like Claude.
   No override = the user's `config.toml` values, shown read-only in the chat header. Onboarding approval notice when
   the configured policy is `never` or the sandbox is `danger-full-access`. Project trust: a chat in an untrusted
   folder asks "Trust once" (PI pattern), never writes `trust_level` itself.
3. **Sessions:** through the app-server (`thread/list`, `thread/read`, `thread/items/list`, `thread/delete`), via a
   short-lived helper process per view load, not by parsing rollout files (their format changes per version).
   Touched files from fileChange items. Archived threads hidden (no archive UI).
4. **Follow-up queue:** app-side, as for every other harness, not Codex's server queue (`thread/queue/changed`), so
   "Remove queued" works the same everywhere.
5. **`!command`:** `thread/shellCommand` (Codex's own `!`, output lands in the thread so the model sees it). It runs
   unsandboxed with full access, so it keeps the app's usual `!command` confirmation and is refused in Always ask.
6. **Harness Skills:** installed through the Harness Skills bootstrap with `HARNESS_SKILLS_HOST=codex` since
   harness-useful-skills v0.3.13 (the same `install.sh` / `install.ps1` the app runs for OpenCode, which needs
   Node.js 22 or newer and `codex` on the `PATH`). The installer adds the marketplace and the `harness-useful-skills` plugin,
   writes the `us-*` agents to `$CODEX_HOME/agents`, copies the memory runtime, registers the `harness-useful-skills` MCP
   server itself with `codex mcp add` (so the MCP matrix never adds it) and writes
   `$CODEX_HOME/harness-useful-skills/install-record.json`, the installed marker (version 0.3.13 or newer, an older record is
   offered the installer as an update). `SkillsStrategy::Installer(SkillsHost::Codex)`, `Companion::CodexSkills`,
   `setup_install_codex_skills`. Uninstall and doctor stay with the skills' tool (`useful-skills uninstall --host
   codex`), updates re-run the installer.
   **Unified names (Harness Skills 0.4.0, 2026-10-23):** one name everywhere (plugin, marketplace, MCP server,
   data folder), so the plugin and server above were `useful-skills` and `useful_skills` and the folder
   `$CODEX_HOME/useful-skills` before. The app recognises both: an install found only under the old names counts as
   installed and is offered the installer as an update (it migrates the install), the MCP server names are both
   reserved for Share setup. **Model roles:** 0.4.0 adds `$CODEX_HOME/harness-useful-skills/models.yml`, the same
   `modelRoles` schema as Claude Code and OpenCode (`<model>[:<effort>]` or `inherit`), which the installer writes into
   the generated `$CODEX_HOME/agents/us-<role>.toml` as `model` and `model_reasoning_effort` (omitted for
   `inherit`, real CLI check 2026-10-23: codex 0.154 loads and spawns such an agent file). The Codex config view lists
   the file as "Harness Skills model roles" (read-only, like Claude Code and OpenCode) only for an install of 0.4.0 or
   newer under the new names and only when the file exists. Editing the roles from the app (pickers fed by
   `model/list`) and an apply step are not built.
7. **MCP registration:** new `McpTarget::CodexCli`: read `config.toml`, write only via `codex mcp add / remove`
   (keeps the user's TOML formatting and comments). Entries with literal secrets are never rewritten.
8. **Spawn environment:** every spawn passes `-c check_for_update_on_startup=false` (S0: confirm the key) and
   disables `in_app_updates`; analytics left as the user configured it. The user's own Codex hooks run as in the
   terminal, app hooks stay app-side.
9. **AI naming:** utility process on the user's default model, no model flag, no tools, ephemeral (not saved).
10. **Naming:** harness id `codex`, name "Codex", short "CX", colour from the dashboard palette's existing Codex entry
    (`ui/src/lib/dashboard/palette.ts:48`), contrast checked in both themes.

11. **Notes guard (user, 2026-10-22: "a"; redesigned in repair round 2):** after the handshake a chat with a memory
    identity reads the project's effective MCP servers from Codex (`config/read{cwd}`) and sends
    `"mcp_servers.<name>.env_vars": [<existing…>, "HARNESS_MEMORY_SESSION"]` in the `config` of every `thread/start`,
    `thread/resume` and `thread/fork` for each enabled harness-memory stdio server listed (read again before each
    new, resumed or forked thread). Nothing is written to `config.toml` and nothing goes on the command line (works
    with `codex.cmd`). Real CLI (Round 2): the thread's server sees the variable; a thread naming a server Codex does
    not have fails with `invalid transport`, so only listed servers are named. The chat reports its own guards as
    applied (`AgentSessionInfo.memoryGuards`) only when a server got the key. The naming helper switches off every
    server of the same read. Residual: plugin servers that `config/read` does not list stay on for the helper.

## Architecture

- `business/src/harnesses/codex/`: detection, config view, extensions, setup, share, CLI actions.
- `business/src/codex_rpc/`: pure frame types (serde from the bindings, only what we use) and mapper → `AgentEvent`.
- `business/src/agent/codex/`: engine (startup + handshake, io loop, prompts, approvals, dialogs, ops, runs).
- `business/src/harnesses/codex/sessions.rs`: helper-process client for the Sessions view.
- Fixtures: `business/tests/fixtures/codex/` (S0 recordings, scrubbed), `fake_agent --dialect codex`.
- Tests as modules in the existing four binaries (`core`, `engines`, `setup`, `views`).

## Steps

- **S0 recording:** scratch project, user's login and default model, record: handshake, plain turn, reasoning,
  command + file approvals (each decision), steer, interrupt, fork at a turn, compact, rename, requestUserInput,
  shellCommand, tokenUsage, thread/list|read|items. Scrub account data. Confirm: fork cut, context window, update
  config key, utility flags, which methods need `experimentalApi`, hooks config location.
- **S1 skeleton:** `HarnessId::Codex`, registry, detection, version parsing, env vars, badge/tokens/locales (6).
- **S2 config view:** `config.toml` through `SafeReader`, secret masking, configured mode.
- **S3 extensions:** skills, plugins, marketplaces, MCP servers, rules, AGENTS.md.
- **S4 sessions:** helper client, list/open/touched files/export/delete, resume/fork entry points.
- **S5 CLI actions + setup:** plugin/marketplace actions, `McpTarget::CodexCli`, `companion_setup`, fix the silent
  fallbacks (`ecosystem/setup.rs:426`, `sync/plan_presence.rs:73`, `sync/collect.rs:163-175`, onboarding driver list,
  `action-model.ts:150`, `mock/setup-handlers.ts:234`), replace the fake Codex row in `mock/setup-roster.ts`.
- **S6 share setup:** MCP names-only, marketplaces + plugins, add-only import.
- **S7 wire + mapper:** `codex_rpc/` from the S0 recordings.
- **S8 engine core:** spawn, handshake, prompt, stream, abort, steer, queue, images, model/effort.
- **S9 approvals + modes + trust:** server requests, decisions, mode mapping, question dialogs.
- **S10 ops:** new/resume/fork/clone/rename/compact/stats/export/`!command`/AI naming; chat on as Preview.
- **S11 hooks, routines, notes guard:** tool table, touched paths, tests.
- **S12 UI polish:** approval notice, Yolo hidden, MCP rows, demo roster.
- **S13 docs:** adding-a-harness (also fix "five locales" → six), security-privacy, hooks-format, routines, README.
- Reviews 3 + 3, `cargo clean`, hand-over with a real-provider checklist.

## Status

| Step | State |
| --- | --- |
| Research + plan | done, approved 2026-10-22 |
| S0 recording | done (a9b643c): 10 scenarios in `business/tests/fixtures/codex/app_server/`, facts below |
| S1 skeleton | done (a2703b9). Light badge `#1d64c8` (dashboard blue too faint on its wash) |
| S2, S3, S5, S6 | done (merged 9814795). `sessions/` + `archived_sessions/` denied (decision 3: sessions via app-server). Unverified against the real CLI: `codex mcp add` on an existing name, `codex plugin` output/exit codes, `[marketplaces.X]` keys `ref`/`sparse_paths`, `.agents/skills` lookup, built-in `openai-curated` marketplace not listed. Config source view prints TOML in YAML style (S12) |
| S7 wire + mapper + fake_agent dialect | done (4b6c697, merged d096c22). Codex 0.154 offers commands only accept / acceptWithExecpolicyAmendment / cancel: "Always" for a command only when `acceptForSession` is offered, never the amendment (writes a lasting rule) |
| S4 sessions | done (merged 6ca290a). Real CLI check 2026-10-22: `thread/list` answers `forkedFromId: null` for every thread, forks too (only `thread/read` has it); session tree on since feat/codex: parents read once per thread in the background (`thread/read`, ≤100 per run, 3 s per batch) and kept by id in `<local data>/codex-thread-parents.json`, so a list shows them from the next poll; a fork is listed only after its own first turn (preview = its own prompt); an unknown id answers `-32600 "thread not loaded: <id>"` (→ NotFound); unloaded saved threads read fine; parent delete with forks → `-32600 "… forked history still references it"`. Cwd filter is exact (subfolder threads not listed) |
| S8 engine core | done (merged dbf2185). Spawn `-c check_for_update_on_startup=false -c features.in_app_updates=false` (both accepted by the real CLI). `summary:"auto"` accepted but gpt-5.6-terra returns no reasoning text either way. Approval overrides refused until S9. Desktop passes `attachments_dir` since S10 |
| S9 approvals + modes + trust | done (merged 21f2769). Mode sent on thread/start and thread/resume only (thread-level `sandbox` keeps the user's writable roots), a mode change resumes the thread. Yolo offered like OMP/OpenCode (`yolo_mode` capability), Auto refused. Trust once = `-c projects={"<abs>"={trust_level="trusted"}}`: real CLI check 2026-10-22, the project layer applies only with it (config/read). Open for S12: `TrustOnce.svelte` copy is PI-specific and shows without a `.codex/` folder; Windows `.cmd` launcher refuses the trust-once argument |
| S10 ops + chat on | done (7f3eb12, merged with S9 in 30d562a). Chat on as Preview. New/switch/fork/clone stay in one process and carry the chat's mode. Real CLI check 2026-10-22: `thread/fork` honours approvalPolicy + sandbox, `thread/resume` of another thread in the same process works, a `/<skill>` text + `skill` input item runs the skill. Fork keeps the chosen message's whole turn (OpenCode/PI fork before it). Helper (naming) chat keeps Codex's tools: never-ask + read-only sandbox. `!command` is not a tool card (`Hint::Shell`). Desktop passes `attachments_dir` |
| S13 docs | done (7dec72d, 6b2af82) |
| Harness Skills for Codex | done (decision 6, unified names and the read-only model roles view added with Harness Skills 0.4.0): Recommended setup and the setup matrix install and update them through the skills' bootstrap with `HARNESS_SKILLS_HOST=codex`. Unverified against a real `codex` and the real installer (tests use fakes) |
| S11 hooks, routines | done (merged 81791b0). Hooks and routines need no Codex-specific source. **Notes guard off:** Codex starts stdio MCP servers with a cleared environment (HOME, PATH, … plus the server's `env_vars`/`env`), so `HARNESS_MEMORY_SESSION` never reaches harness-memory. Open decision for the user |
| S12 UI polish | done (merged ff762b1). Trust once harness-aware (Codex: only with an untrusted `.codex/` layer), TOML source view, harness-named refusals, sessions helper update switches. Windows `.cmd` launcher still refuses the trust-once argument |
| Reviews | pass 1 done (3 correctness + 2 security; 1/3 + 1/3). Repair round 1 (one agent, user's budget) merged 188fa24: decision 11 notes guard, trust = exact folder or repo root (real CLI), untrusted default read-only (real CLI), permission grants limited to what the dialog shows, `approvalsReviewer: user` + warning when Codex answers another mode, oversized frames answered, stale abort/counts, sessions per-call deadlines + 64 MiB read budget + page fingerprints, ownership check before resume, relative CODEX_HOME dropped, sqlite side files denied, switched-off Codex servers not shared (other harnesses still share them). Real CLI: naming helper thread `config` `{"web_search":"disabled","mcp_servers.<n>.enabled":false}` works. Pass 2 done (2 correctness + 1 security; 2/3 + 2/3). Repair round 2 merged 66d636d (packages E + U): notes-guard key now in each thread's `config` for the harness-memory servers Codex lists in `config/read` (real CLI 2026-10-23: the key reaches the server on thread/start, resume and fork), memory button only when it applies, approvals show the command as it runs and every path granted, Codex's own looser settings named once per chat, symlink-safe outside-path split, count fixes, MCP names starting with `-` refused + re-read before `codex mcp add`, physical-path trust (submodule: exact path only, real CLI), fake CLI ETXTBSY race fixed, cut-short transcripts flagged. Pass 3 done (1 correctness + 1 security; 3/3 + 3/3 used). Final repairs merged 92ccab6: move destinations and outside paths named (no Always allow then), system-shell-only unwrap, unredacted commands with hidden characters spelled out, dangling symlinks followed, resolved writable-root warning, memory button follows each new thread, late turns count once. Main merged in 01c78d0. Full suite green. **Implementation complete, waiting for the user's real-provider check** |

## Verified facts (S0 recording, 2026-10-22)

Recorded with `business/tests/fixtures/codex/record.py` against 0.154.0 on the user's ChatGPT login (plan `free`) in
a scratch git repo, scrubbed by `scrub.py` (paths → `/work/project`, `/home/user`, account email, lists cut to three,
tool schemas dropped) into `business/tests/fixtures/codex/app_server/`. The scratch threads were deleted afterwards.

1. **Transport:** stdio works as documented. No `jsonrpc` field in either direction. Notifications carry
   `emittedAtMs`. Handshake `initialize` → `initialized` as in the bindings; `experimentalApi: true` accepted.
2. **Configured model can be unusable:** the user's `config.toml` model `gpt-6-astra` fails every turn ("not
   supported when using Codex with a ChatGPT account"): `warning` (no model metadata), `error{willRetry:false}`,
   `thread/status/changed{systemError}`, `turn/completed{failed, error}`. The chat must show the error text and offer
   the model picker. `model/list` marks `isDefault` (here gpt-5.6-terra, efforts low…ultra, input text + image).
   **AI naming / no-model-flag rule:** fall back to `model/list`'s default when the configured model fails.
3. **Approvals (untrusted + workspace-write):** even Codex's startup probe (`source: unifiedExecStartup`, `pwd && rg
   --files -g AGENTS.md`) asks. `commandExecution/requestApproval` carries `command`, `cwd`, `kind`, `itemId`.
   `fileChange/requestApproval` carries only `itemId`, `reason`, `grantRoot`: the paths come from the matching
   `item/started{fileChange, changes[{path, kind, diff}]}`. Every answer is echoed by `serverRequest/resolved`. A
   declined command → `item/completed{commandExecution, status failed}`, the turn continues.
4. **Steer:** only while a turn is active, otherwise `-32600 "no active turn to steer"`. The steer text appears as a
   second `userMessage` item in the same turn and the answer honours it.
5. **Interrupt:** `turn/completed{status interrupted}`.
6. **Ops:** `thread/name/set` → `thread/name/updated`. `thread/shellCommand` and `thread/compact/start` each run as
   their own turn (`turn/started` … `commandExecution` / `contextCompaction` item … `turn/completed`). A thread must be
   loaded in the process (start / resume / fork) before ops, otherwise "thread not found".
7. **Fork:** `thread/fork{lastTurnId}` keeps the turns up to and including that turn (1 of 7 here), without a cut it
   keeps all; `forkedFromId` set, the name is copied. Forks are not returned by `thread/list{cwd}` with default
   filters. A parent cannot be deleted while a fork exists ("forked history still references it").
8. **Tokens:** `thread/tokenUsage/updated{total, last, modelContextWindow}` (258400 here) → context meter without a
   lookup table. `account/rateLimits/updated` arrives every turn.
9. **History:** `thread/read{includeTurns:true}` works but emits `deprecationNotice`: use `thread/turns/list` +
   `thread/items/list` (paged, `turnId` per item).
10. **Reasoning:** `reasoning` items arrive with empty `summary` and `content` on the default settings, no deltas.
    Thinking text needs `summary` on `turn/start` (S8: try `auto` / `concise`).
11. **Ephemeral threads** work for AI naming (`thread/start{ephemeral:true}`), not listed or saved.
12. **Question dialogs:** `request_user_input is unavailable in Default mode` (stderr): the tool exists only in Plan
    collaboration mode. The dialog path is built from the bindings and tested with synthetic frames.
13. **Noise to ignore or fold:** `mcpServer/startupStatus/updated` (one per MCP server per thread start),
    `turn/moderationMetadata`, `turn/diff/updated`, `thread/settings/updated`.
14. **Workspace roots:** `runtimeWorkspaceRoots` includes the user's extra `writable_roots`.
15. **Sizes:** an unfiltered `plugin/list` returned 18.8 MB (whole remote catalogue): Extensions use
    `plugin/installed` or filters, never the bare call. `mcpServerStatus/list` carries full tool schemas.
16. **Config facts:** `-c check_for_update_on_startup=false` accepted; update prompts are also the `in_app_updates`
    feature. Hooks live in `<codex home>/hooks.json` (and a `hooks` config key), events PreToolUse,
    PermissionRequest, PostToolUse, PreCompact, PostCompact, SessionStart, SessionEnd, SubagentStart, SubagentStop.
    Profiles exist (`ConfigProfile`).

## Residuals (known, accepted)

- A reopened chat starts without the approval mode picked before an app restart (every harness, on main before this
  branch). The header shows the real mode.
- Windows npm install (`codex.cmd`): Trust once is refused by the batch-script argument check.
- Fork keeps the chosen message's whole turn including its reply (OpenCode / PI fork before the message).
- A fork appears only after its own first turn (Codex lists it then), and its parent shows from the Sessions view's
  next poll after the background read, not on the first list.
- Sessions list filters on the exact cwd: threads started in a subfolder of the project are not listed.
- Reasoning text: gpt-5.6-terra returns empty reasoning summaries even with `summary: "auto"`.
- Question dialogs (`request_user_input`) only exist in Codex's Plan mode, which the app does not expose: built from
  the bindings, tested with synthetic frames only.
- The naming helper cannot switch off MCP servers Codex does not list in `config/read` (plugin servers); it runs
  read-only with `untrusted` approvals, which it declines.
- Switched-off MCP servers are not shared from Codex (Codex cannot import one switched off); OMP/PI/OpenCode still
  share them.

## Real-provider checklist

Run with your own `codex login` on 0.154–0.156. Recordings and real-CLI checks already cover the protocol, trust,
modes, forks, skills, the notes-guard key and the sessions calls (see Verified facts and the Status table).

1. Onboarding: Codex shows as installed, the approval notice matches your `config.toml` (yours: on-request with an
   extra writable root and network).
2. Config view: `config.toml` as TOML, secrets (the context7 Authorization header, node_repl env) hidden.
3. Extensions: your plugins, marketplaces, skills (incl. `~/.agents/skills`) and MCP servers listed.
4. Recommended setup: dashboard + memory + desktop MCP rows add through `codex mcp add`; check `config.toml` after.
   Then a new chat offers the memory button ("Ask before saving") and a note save asks.
5. New chat on the configured model `gpt-6-astra`: the error from Codex shows and the picker offers gpt-5.6-terra.
6. Chat on gpt-5.6-terra in Write: streamed reply, a command approval (Allow once / Reject), a file change (Always
   allow), the context meter, stop mid-command, steer while a command runs, a queued follow-up and Remove queued.
7. Always ask: `!ls` refused; Write: `!ls` runs and shows its output.
8. `/compact`, rename, fork at the first message, clone, new chat, switch back, HTML export.
9. A skill as `/name`.
10. Sessions view: list, open, touched files, delete a fork, then its parent.
11. A project with its own `.codex/config.toml`: notice, Trust once, the project setting applies for that chat only.
12. Extensions: install and remove one plugin through `codex plugin` (output and exit code are untested).
13. Share setup export from Codex, then import on another harness (and back).
14. One routine on Codex and one hook (toolEnd on `apply_patch`).

## Risks

- `app-server` is labelled experimental: method shapes may move between releases. Mitigation: pin supported
  versions (0.154–0.156 tested, newer "Untested"), regenerate bindings per bump, tolerant decoding.
- Two binaries on this machine (PATH codex and the ChatGPT app's bundled one), possibly different versions. The app
  only uses PATH.
- `config.toml` contains literal secrets: every read path masks, no write path copies them.
