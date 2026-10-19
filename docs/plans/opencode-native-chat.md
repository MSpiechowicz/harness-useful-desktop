# OpenCode chat over the native server API — plan (resumable)

Issue #22, branch `feat/opencode-chat`. Level 3 of [opencode-support.md](opencode-support.md), replacing its ACP
proposal. Plan approved 2026-10-19. Each step below ends in one commit; the **Status** column says where to resume.

## Goal

Chat with OpenCode in the app with **1:1 parity with OMP and PI chat** as the desktop sees it: streaming, tool cards,
approval cards and approval modes, model / thinking / agent pickers, slash commands, steering and follow-up queue,
abort, new chat, resume, fork, clone, rename, compact, HTML export, stats and context meter, copy last reply,
`!command`, images, AI chat naming, question dialogs, hooks (#16), team hooks (#26) and routines (#14).

Not in scope, matching OMP and PI: login (the app has none, "sign in from a terminal") and per-session MCP (the app
registers its MCP servers in the user's config, `McpTarget::OpenCodeConfig`). Not possible in OpenCode: moving a chat to
another project (capability off, like PI). Live subagent cards are a backlog item; a `task` shows as a tool card and its
asks still reach the chat.

## Why not ACP

ACP v1 (`opencode acp`) is a thin shim over OpenCode's HTTP server and lacks steering and queueing, `!command`, fork at
a message, export and a shared meaning of approval modes, and it gives tools only as title and kind (hooks match on tool
names). The generic ACP engine moved to #27 (Gemini CLI and other ACP-only agents). OMP and PI stay on their RPC engine.

## Decisions (user, 2026-10-19)

1. **One `opencode serve` per chat.** Fits the existing lifecycle (`Exited`, kill on drop, restart with an override,
   `MAX_SESSIONS`), keeps "always allow" scoped to the chat, a crash ends one chat. Cost: one Bun process (≈200–400 MB)
   and a 1–3 s cold start per chat.
2. **Subagents in Always ask / Write ask first** (`task` rule `ask`). Since review round 2 every agent, subagents
   included, also gets the mode's rules through the server's configuration (below), so a subagent's tools ask too.
3. **Question tool on** (OpenCode's own default under `serve`, `tool/registry.ts:207`), answered through the existing
   dialogs. Unattended routines stall on it like on any ask.
4. **"Always allow" is not saved by the app.** It lives as long as the chat's server, like OMP's.
5. **"Preview" badge** on OpenCode chat (`Capabilities.chat_preview`).
6. **AI chat naming** through a utility session with `*:*:deny` rules, deleted at stop.
7. **Team OpenCode routines** need a harness-useful-memory commit (its routine format accepts only `omp|pi`).

Standing rules: random per-launch `OPENCODE_SERVER_PASSWORD` (Basic auth, user `opencode`), `--hostname 127.0.0.1
--port 0 --mdns=false` always (a user config with `server.mdns: true` would otherwise listen on 0.0.0.0,
`cli/network.ts:62-79`), `OPENCODE_DISABLE_AUTOUPDATE=1`. Never change OpenCode config files. Never call `/share`,
`/global/dispose`, `/global/upgrade`, config writes, `/auth/*`, OAuth, `/tui/*`, `/pty/*`, `/experimental/*`. Never pass
`--auto`, `--yolo`, `--dangerously-skip-permissions`; never `auth login` / `providers login` / `mcp auth` / `db` /
`account`. One `cargo clean` before the final commit only.

## OpenCode 1.19.3 server surface

Paths under `packages/opencode/src` of the OpenCode source.

- **Start:** `opencode serve --hostname 127.0.0.1 --port 0 --mdns=false` prints
  `opencode server listening on http://127.0.0.1:<port>` (`cli/cmd/serve.ts:20`). Health `GET /global/health`.
  Auth Basic (`OPENCODE_SERVER_USERNAME`, default `opencode`), or `?auth_token=<base64 user:pass>`.
- **Directory routing:** `?directory=` or `x-opencode-directory`. `/session/{id}/…` routes by the stored
  `session.directory` (`workspace-routing.ts:182`). `/permission/*` and `/question/*` are not session-scoped: replies
  carry the session's directory.
- **Events:** `GET /global/event` → SSE `data:` `{directory, project, workspace, payload:{id,type,properties}}`
  (durable events repeat as `payload.type=="sync"`: ignore). `server.connected` first, `server.heartbeat` every 10 s.
  Types: `session.created|updated|deleted`, `session.status {sessionID, status:{type: idle|busy|retry}}`,
  `session.idle`, `session.error`, `message.updated`, `message.removed`, `message.part.updated {part}`,
  `message.part.delta {messageID, partID, field, delta}`, `message.part.removed`, `permission.asked {id, sessionID,
  permission, patterns, metadata, always, tool?}`, `permission.replied`, `question.asked {id, sessionID,
  questions[{question, header, options[{label, description}], multiple?, custom?}]}`, `question.replied|rejected`.
- **Parts:** text, reasoning, tool (`state.status` pending | running | completed | error, `input`, `output`/`error`,
  `title`, `metadata`), file, step-start, step-finish (cost, tokens), compaction, retry, subtask, patch, snapshot,
  agent. Each loop step is a new assistant message whose `parentID` is the last user message.
- **Tool ids:** `bash read glob grep edit write apply_patch task webfetch websearch todowrite skill question invalid`
  (+ `lsp`, `plan_exit` behind flags), MCP tools `<server>_<tool>`. Permission key `edit` covers edit, write and
  apply_patch.
- **Routes:** `POST /session/{id}/prompt_async {parts, agent?, model?:{providerID,modelID}, variant?}` → 204 (never
  send the deprecated `tools`, it replaces session permissions, `prompt.ts:1060-1067`); `POST /session/{id}/command
  {command, arguments, agent?, model:"prov/model", variant?}` (blocks); `POST /session/{id}/shell {agent, command,
  model?}` (blocks, 409 when busy, no permission check, no exit code); `POST /session`; `GET|PATCH|DELETE
  /session/{id}` (PATCH `{title?, permission?}` *appends* `permission` to the rules stored with the session,
  `Permission.merge` concatenates, nothing removes them); `POST /session/{id}/fork {messageID?}`
  (copies messages before `messageID`, no `parentID`); `POST /session/{id}/abort`; `POST /session/{id}/summarize
  {providerID, modelID}` (blocks); `GET /session/{id}/message?limit&before`; `GET /session/status`;
  `GET /config/providers` → `{providers, default}`; `GET /agent`; `GET /command` → `{name, description,
  source: command|mcp|skill, hints}`; `POST /permission/{id}/reply {reply: once|always|reject, message?}`;
  `POST /question/{id}/reply {answers:[[label…]]}`; `POST /question/{id}/reject`.
- **Behaviour:** reject cascades to every pending ask of the session. "Always" is in memory per directory instance.
  Rules evaluate `findLast` over agent rules, then session rules, then approved rules. The loop reads
  `session.permission` once per run (`session/prompt.ts:1086`), so agent and approval-mode changes happen only while
  idle. A prompt sent while busy joins the running loop at the next step boundary; an end-of-turn race can strand it
  (`effect/runner.ts:115-122`), so on idle the engine checks the newest message and re-sends an unanswered user message.
- **Security note:** `OPENCODE_SERVER_PASSWORD` reaches tool shells, MCP servers and plugins OpenCode starts. A command
  the agent runs could call the server API. Documented, cannot be stripped from the app side.

## Architecture

- **Seam:** `Harness::chat_protocol() -> ChatProtocol { Rpc(&'static dyn RpcDialect) | OpenCodeServer }`, default
  `Rpc(self.dialect())`. `AgentSession` becomes `enum { Rpc(RpcSession), OpenCode(OpenCodeSession) }` forwarding
  `send, respond, bash, emit, on_prompt_taken, move_project, stop, exit_info, protocol_version, is_running`.
  `SessionLaunch` carries the protocol plus `resume`, `approval_override`, `utility`.
- **Shared plumbing:** `business/src/agent/process.rs` (`AgentProcess`): spawn, emitter, redacted rate-limited
  `stderr_loop`, `wait_loop` with `Exited` last, stop (close stdin, grace, kill the group), kill on drop, startup
  deadline / abort race. Both engines use it (jscpd threshold 0).
- **HTTP/SSE client:** hand-written HTTP/1.1 on tokio `TcpStream`, loopback addresses only, `Connection: close`,
  Content-Length and chunked bodies, SSE line parser with size caps. No new crates.
- **Code layout:** `business/src/opencode_server/` (`http/{client,chunked,sse}.rs`, `routes.rs` closed `Route`
  allowlist, `wire.rs`, `mapper/`, `modes.rs`, `export_html.rs`) and `business/src/agent/opencode/{mod,startup,
  prompts,dialogs,ops}.rs`. The converter in `harnesses/opencode/sessions/transcript.rs` becomes a shared module
  (history, stats, last reply, export, touched paths).
- **Startup:** (approval mode only: a probe `opencode serve` → `GET /agent` → stop, then the mode as
  `OPENCODE_CONFIG_CONTENT`) → spawn → parse the listening line within `STARTUP_TIMEOUT` → `GET /global/health` →
  subscribe SSE → `POST /session` or (resume) `GET /session/{id}` with the directory check → load `/agent` (checked
  against the probe's), `/config/providers`, `/command` → model / agent / variant from the last user message → a helper
  chat's deny-all PATCH → emit `Ready`, `State`, `CommandsUpdated`, the probe included all within one
  `STARTUP_DEADLINE`. Events filtered to the chat's session plus its child
  sessions (asks from children must reach the UI).
- **Sessions view:** stays on the CLI. Enabling chat turns on Resume in chat and Fork there. `session_tree` stays false.

## Parity mapping

| Feature | OpenCode | App |
| --- | --- | --- |
| Text, thinking | `message.part.updated` + `message.part.delta` | `MessageStart`, `TextDelta`, `ThinkingDelta`, `MessageEnd` on `time.completed` |
| Tool cards | tool part state | `ToolStart` / `ToolUpdate` / `ToolEnd{name: tool id, is_error, touched_paths}` |
| Approvals | `permission.asked` → reply route; `permission.replied` | `UiRequest::Select` "Allow once" / "Always allow" / "Reject"; `UiCancel` |
| Approval modes | server configuration (below) | `approval_modes: true`, badge source "OpenCode settings" |
| Model, thinking | `/config/providers` (`limit.context`, `variants`), `variant` per prompt | `ModelOption`, levels = variant keys |
| Agent picker | `GET /agent` (primary, not hidden), `agent` per prompt | new `SetAgent`, `GetAvailableAgents`, `SessionState.agent`, `agent_picker` |
| Slash commands | `GET /command`; `/name args` → `/session/{id}/command` | `parse_live` by `source` |
| Steer | `prompt_async` while busy | live until a reply with that parent appears |
| Follow-up, remove | held by the app, sent at idle | `QueueUpdate`, `RemoveQueued`, `remove_queued: true` |
| Abort | `/abort` | `Settled{aborted}` |
| New, resume | `POST /session`, `GET /session/{id}` | — |
| Fork, clone | `/fork {messageID}`, `/fork` | fork id `msg_…` |
| Rename | `PATCH {title}` | `session.updated` → `State` |
| Compact | `/summarize` | instructions → Unsupported |
| HTML export | app renders all messages, escaped | `Exported{path}` |
| Stats, context | message `cost`, `tokens`, model `limit.context` | `SessionStats`, `ContextUsage` |
| Copy last reply | last assistant text parts | `Text` |
| `!command` | `/shell` (409 → "agent is working") | `BashOutput{exit_code: None}` |
| Images | file part `data:<mime>;base64,…` | — |
| AI naming | utility session, `*:*:deny`, deleted at stop | same `ai-name.ts` flow |
| Questions | `question.asked` → Select (plus "Type your own answer…" unless `custom: false`) or Input; dismiss → reject | — |
| Move project | not possible | `move_session: false` |

**Approval modes** (`modes.rs`, `mode_config.rs`, engine `probe.rs`): per agent, a leading catch-all + the agent's own
rules transformed, so `findLast` keeps denies and order and the list decides alone. Always ask `[*:*:ask]` +
allow→ask (except `question`, `todowrite`, `plan_enter`, `plan_exit`). Write `[*:*:ask]` + allow→ask except a
read-only allowlist (`read glob grep list lsp skill todowrite question plan_enter plan_exit`), the agent's
`external_directory` rules kept. Yolo `[*:*:allow]` + ask→allow, agent denies kept (plan stays read-only), an ask
inside an earlier deny stays an ask. No override: nothing set, the user's config applies.

Delivery (review round 2): PATCHed session rules are appended and stored in OpenCode's database for good, so the mode
is never set on a session. The chat starts `opencode serve` once without the mode, reads `GET /agent`, stops it, and
starts its server with every agent's mode rules as `agent.<name>.permission` in `OPENCODE_CONFIG_CONTENT` (read last,
`config/config.ts`, appended after each agent's rules, `agent/agent.ts`). Keys end in ` *` (OpenCode's wildcard reads
`read *` as exactly `read` for space-free names), so the user's own agent permissions never share a key with ours and
keep their place before them. An inherited `OPENCODE_CONFIG_CONTENT` is merged (ours wins), unparsable content refuses
the mode. After the start every agent must end with the mode's rules (only OpenCode's tool-output `allow` after
them), otherwise the chat is refused. Agent switches, new chats, forks and clones set nothing. Only a helper chat's
throwaway session gets a PATCH (deny all), and it is deleted at stop.

## Hooks and routines

- `sessionStart` / `promptSubmit` go through `chat_hooks::send` unchanged.
- `toolEnd`: `name` = OpenCode tool id; `touched_paths` from `edit`/`write` `input.filePath` and `apply_patch`
  `input.patchText` headers, project-contained (shared with the CLI transcript, which keeps old names for old exports).
- `turnEnd` / `Settled` from `session.status idle` of the chat's own session after flushing `MessageEnd`s, plus
  `AgentEnd` and `PromptResult` (errors from `session.error` / assistant `error`). `AgentStart` from `busy`.
- `sessionEnd` from `Exited`. Prompt taken on the `prompt_async` 204, in event order. OpenCode commands always start a
  turn (`disposition` `started` / `queued`). Failed send → `never_delivered`.
- Routines: `has_chat` adds OpenCode, `approvalMode` allowed for `omp|opencode`, `trustOnce` stays PI-only, docs and
  UI form updated. Team routines need harness-useful-memory accepting `opencode` (new minor version); older memory keeps
  showing "update Harness Memory".
- Hooks MCP: `McpTarget::OpenCodeConfig` registers globally; step 1 confirms `serve` loads it.

## Steps

| # | Step | Owner | Commit | Status |
| --- | --- | --- | --- | --- |
| 0 | This plan | main | `docs(opencode): plan native server chat (level 3)` | done |
| 1 | Record real `opencode serve` responses (scratch HOME, `unshare -rn`) → `business/tests/fixtures/opencode/server/` | main | `test(opencode): record opencode serve responses` | done |
| 2 | Shared process plumbing (`agent/process.rs`), no behaviour change | backend | `refactor(agent): share the process lifecycle between chat engines` | done |
| 3 | Engine seam, OpenCode still "coming soon" | backend | `refactor(agent): engine seam for non-RPC chat protocols` | done |
| 4 | Loopback HTTP + SSE client, closed route allowlist | backend | `feat(opencode): loopback HTTP and SSE client` | done |
| 5 | Fake OpenCode server (`fake_agent --dialect opencode`) + scenarios | backend | `test(opencode): scripted fake opencode server` | done |
| 6 | Contract: `agent_picker`, `chat_preview`, `SessionState.agent`, `SetAgent`, `GetAvailableAgents`, bindings | backend | `feat(model): agent picker and preview capability in the chat contract` | done |
| 7 | Engine core: lifecycle, SSE mapper, prompts, steer, queue, idle race, abort, state, history | backend | `feat(opencode): native chat engine over opencode serve` (7a f5062c4) + `feat(opencode): steering, queue, abort, slash commands and history` (7b) | done |
| 8 | Approvals, questions, approval modes | backend | `feat(opencode): approvals, questions and per-chat approval modes` | done |
| 9 | Session ops: new, fork, clone, rename, compact, export, stats, last text, shell, images | backend | `feat(opencode): session actions, export, stats and !commands` | done |
| 10 | Harness wiring: launch spec, `chat_protocol`, capabilities, catalog | backend | `feat(opencode): enable chat for the OpenCode harness` | done |
| 11 | Hooks and routines (memory side done: harness-useful-memory `feat/opencode-routines` 37b53d7, unpushed). Desktop: mirror its 3 routine fixtures (`valid-opencode-agent.json` renamed from `invalid-agent-harness-opencode.json`, `valid-opencode-approval-write.json`, `invalid-agent-trust-once-with-opencode.json`), fix `hooks_mcp/routine_tools.rs` approvalMode text | backend | `feat(routines): OpenCode agent routines and hooks` | done |
| 12 | UI: agent picker, Preview badge, approval badge, routine form, onboarding, 5 locales | frontend | `feat(ui): OpenCode chat with agent picker and Preview badge` | done (406007a) |
| 13 | Guard rules and docs | general | `docs(opencode): native chat architecture, security and formats` | done |
| 14 | Full verification, reviews, `cargo clean`, final commit | main | — | done: review passes 3/3 and the final repair round, verified, `cargo clean` run. Next: the manual check with a real provider (see Verification) |

Order: 0 → 1 → 2 → 3 → (4 ∥ 5 ∥ 6) → 7 → (8 ∥ 9) → 10 → (11 ∥ 12) → 13 → 14.

Final repair round (after review pass 3): the probe reads the agents of the directory the chat works in (a resumed
session's folder), `check_applied` requires each agent's exact probe rules then the mode's, and the mode is refused for
a resumed session with its own non-deny rules, duplicate or missing agent names, colliding or number-like keys, a
config over the environment limit (100 KiB, 30 KiB on Windows) or a server that cannot spawn with it. A mode start gets
twice the startup deadline for its two starts. The "Always allow" detail says it overrides denies for the rest of the
chat. Probe failures (no address, early exit, health, stop, deadline) are reaped and tested.

## Verification

Rust (prefix `rustup run stable` locally): `cargo fmt --all --check`; `cargo clippy -p harness_business --all-targets --
-D warnings`; `cargo nextest run -p harness_business --no-fail-fast`; `cargo test -p harness_business export_` then
`git diff --exit-code -- ui/src/lib/ipc/bindings ui/src/lib/ipc/mock/fixtures`; `cargo deny check advisories bans
licenses sources`; `cargo build -p harness_business --bin fake_agent && cargo test -p harness_desktop`;
`bun run build:ui && cargo clippy -p harness_desktop -- -D warnings`. Bun: `bun run check`, `bun run test`,
`bun run guard`, `bun run dup`, `bun run audit`.

Manual check with a real provider: streaming and live tool cards; Always ask → approval card, once / always / reject
(reject cascades); plan agent refuses edits in Yolo; agent, model, variant switches; steer mid-turn; follow-up queue and
remove; abort; new, resume from Sessions, fork, clone, rename, compact, export and Reveal; stats, context, cost; copy
last reply; `!command`; image; AI naming; question dialog; a subagent's ask; hooks `toolEnd edit|write` with paths,
`turnEnd`, `sessionStart`; unattended routine with `approvalMode` write; no `opencode` process left after closing;
`ss -ltnp` shows only 127.0.0.1; OpenCode config files unchanged; no share URL.

## Risks

- The server API is marked experimental: chat is limited to 1.19.x, drift is caught by contract tests on recordings.
- Memory and cold start per chat; a first-run plugin install may need an OpenCode-specific startup deadline.
- The password reaches tools (see above).
- Hand-written HTTP edge cases: tested against recorded Bun byte streams.
- Cross-repo change for team routines.

## Recording results (step 1)

See `business/tests/fixtures/opencode/server/README.md`. Confirmed: listening line after 0.9 s; `--port 0` may pick
4096, so read the port from the line; **no exit on stdin EOF**, stop sends SIGTERM before the kill; global MCP config
loads; steering lands after the running step; abort gives `MessageAbortedError`; an unknown model gets `204` then
`session.error` twice; `data:` images work; question tool on; message pages via `x-next-cursor` / `before`; SSE is
chunked, JSON replies use `Content-Length`; `sync` duplicates on the global stream.

## Open checks (during implementation)

- Where OMP/PI write HTML exports; mirror it, otherwise the app data dir, never the project.
- Whether PI's fork puts the message text back in the composer.
- How `BashOutput.exit_code: None` renders.
- [x] Questions with Yes/No-like option labels were numbered by the backend ("1. Yes") so `approval.ts` did not render them as approval cards. `UiRequest.purpose` (`approval` / `question`) now marks them, the labels stay as asked, and permission asks carry `toolCallId` so the approval card shows the call that asks.
- `/compact` no longer asks for instructions on OpenCode (`Capabilities.compactInstructions`, step 11).
- A 4xx on `prompt_async` maps to `Protocol`, so hooks treat it as possibly delivered and do not re-arm `sessionStart` context (review item).
- [x] Some engine error messages joined sentences with ";" (e.g. "the agent is working; …", compact refusal). Split into two sentences (step 13), including the shared move messages in `agent/session.rs` and the mock handlers.
- [x] Mock `agent_start` refusal text in `ui/src/lib/ipc/mock/agent-handlers.ts` named OpenCode as coming soon. It now uses the harness name (step 13).
- [x] `docs/adding-a-harness.md` used OpenCode as the `chat: false` example. It documents non-RPC engines instead (step 13).
