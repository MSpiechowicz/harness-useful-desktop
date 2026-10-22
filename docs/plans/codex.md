# Codex support — research and plan (resumable)

Issue #21. Research 2026-10-22 against Codex CLI **0.154.0** (`~/.local/bin/codex`; `~/.codex/version.json` already
announces 0.156.0). Protocol reference: the bindings the CLI generates itself, `codex app-server generate-ts` and
`generate-json-schema` (regenerate on every version bump, they carry no version header). Status: **approved 2026-10-22, all 10 decisions as
proposed** (user: "go"). Implementation on branch `feat/codex`.

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
| Question dialogs | `item/tool/requestUserInput`, `mcpServer/elicitation/request` (form only, url → open in browser after confirm) | bindings, experimental |
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
6. **Harness Skills:** Harness Skills has no Codex adapter yet (its README says so). Proposed: the Codex row in
   Recommended setup installs the dashboard + memory MCP only and shows no skills row; a Codex adapter (skills into
   `~/.codex/skills`, models.yml roles) is a separate issue in harness-useful-skills, added to the desktop afterwards.
7. **MCP registration:** new `McpTarget::CodexCli`: read `config.toml`, write only via `codex mcp add / remove`
   (keeps the user's TOML formatting and comments). Entries with literal secrets are never rewritten.
8. **Spawn environment:** every spawn passes `-c check_for_update_on_startup=false` (S0: confirm the key) and
   disables `in_app_updates`; analytics left as the user configured it. The user's own Codex hooks run as in the
   terminal, app hooks stay app-side.
9. **AI naming:** utility process on the user's default model, no model flag, no tools, ephemeral (not saved).
10. **Naming:** harness id `codex`, name "Codex", short "CX", colour from the dashboard palette's existing Codex entry
    (`ui/src/lib/dashboard/palette.ts:48`), contrast checked in both themes.

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
| S4 sessions | done (merged 6ca290a). Real CLI check 2026-10-22: `thread/list` leaves `forkedFromId` out (only `thread/read` has it), so no session tree / fork count; a fork is listed only after its own first turn (preview = its own prompt); an unknown id answers `-32600 "thread not loaded: <id>"` (→ NotFound); unloaded saved threads read fine; parent delete with forks → `-32600 "… forked history still references it"`. Cwd filter is exact (subfolder threads not listed) |
| S8 engine core | done (merged dbf2185). Spawn `-c check_for_update_on_startup=false -c features.in_app_updates=false` (both accepted by the real CLI). `summary:"auto"` accepted but gpt-5.6-terra returns no reasoning text either way. Approval overrides refused until S9. Desktop passes `attachments_dir` since S10 |
| S9 approvals + modes + trust | done (merged 21f2769). Mode sent on thread/start and thread/resume only (thread-level `sandbox` keeps the user's writable roots), a mode change resumes the thread. Yolo offered like OMP/OpenCode (`yolo_mode` capability), Auto refused. Trust once = `-c projects={"<abs>"={trust_level="trusted"}}`: real CLI check 2026-10-22, the project layer applies only with it (config/read). Open for S12: `TrustOnce.svelte` copy is PI-specific and shows without a `.codex/` folder; Windows `.cmd` launcher refuses the trust-once argument |
| S10 ops + chat on | done (7f3eb12, merged with S9 in 30d562a). Chat on as Preview. New/switch/fork/clone stay in one process and carry the chat's mode. Real CLI check 2026-10-22: `thread/fork` honours approvalPolicy + sandbox, `thread/resume` of another thread in the same process works, a `/<skill>` text + `skill` input item runs the skill. Fork keeps the chosen message's whole turn (OpenCode/PI fork before it). Helper (naming) chat keeps Codex's tools: never-ask + read-only sandbox. `!command` is not a tool card (`Hint::Shell`). Desktop passes `attachments_dir` |
| S13 docs | done (7dec72d, 6b2af82) |
| S11 hooks, routines | done (merged 81791b0). Hooks and routines need no Codex-specific source. **Notes guard off:** Codex starts stdio MCP servers with a cleared environment (HOME, PATH, … plus the server's `env_vars`/`env`), so `HARNESS_MEMORY_SESSION` never reaches harness-memory. Open decision for the user |
| S12 UI polish | done (merged ff762b1). Trust once harness-aware (Codex: only with an untrusted `.codex/` layer), TOML source view, harness-named refusals, sessions helper update switches. Windows `.cmd` launcher still refuses the trust-once argument |
| Reviews | pass 1 running (3 correctness + 2 security), read-only on feat/codex |

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

## Risks

- `app-server` is labelled experimental: method shapes may move between releases. Mitigation: pin supported
  versions (0.154–0.156 tested, newer "Untested"), regenerate bindings per bump, tolerant decoding.
- Two binaries on this machine (PATH codex and the ChatGPT app's bundled one), possibly different versions. The app
  only uses PATH.
- `config.toml` contains literal secrets: every read path masks, no write path copies them.
