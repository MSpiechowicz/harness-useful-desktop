# OpenCode 2 support — plan (resumable)

Branch `feat/opencode-v2` (desktop) and `feat/opencode-v2` (harness-useful-skills). Plan approved 2026-10-21, including
real-traffic recording with the free `opencode/big-pickle` model. Builds on
[opencode-native-chat.md](opencode-native-chat.md) and [opencode-support.md](opencode-support.md). Each step below ends
in one commit; the **Status** column says where to resume.

## Goal

OpenCode 2 (2.1.x, npm `@opencode/cli`) gets the same parity as OpenCode 1 (1.18 / 1.19): detection, wizard, Config and
Extensions views, Sessions (list, open, export, delete, touched files), plugin install and remove, Share setup, Harness
Skills, MCP registration, native chat (streaming, tool cards, approvals and approval modes, model / thinking / agent
pickers, slash commands, steering and follow-up queue, abort, new / resume / fork / clone / rename / compact, HTML
export, stats and context meter, copy last reply, `!command`, images, AI naming, question dialogs), hooks (#16), team
hooks (#26) and routines (#14). Nothing changes for OpenCode 1.18 / 1.19.

Not in scope: importing OpenCode 1 sessions, the v2 background service, remote or stdio servers, live subagent cards,
login, per-session MCP.

## Acceptance

- `opencode --version` printing `opencode v2.1.2` is detected as 2.1.2, Supported. Wizard, Config and Extensions views
  show what v2 reads: `plugins` (string or `{package, options}`) plus legacy `plugin`, `mcp.servers.*` plus legacy
  `mcp.*`, `agents` / `agent`, `commands` / `command`; no `config.json` layer on v2; `OPENCODE_CONFIG_DIR` replaces
  the global directory on v2.
- Sessions view lists, opens (transcript, touched files), exports and deletes v2 sessions; every v2 CLI call has
  `--standalone`; no `opencode serve --service` is left running.
- Plugins: v2 installs with `opencode plugin add <pkg>`; remove keeps its preview and edits `plugins` (and legacy
  `plugin`); Share setup exports and imports v2 plugins and MCP servers; Recommended setup registers the dashboard,
  memory and desktop MCP servers as `mcp.servers.<name>` and recognises existing entries in either shape.
- Chat on v2 reaches v1 parity with one private `serve --hostname 127.0.0.1 --port 0` per chat and a random
  `OPENCODE_SERVER_PASSWORD`.
- Hooks and routines work on v2; `toolEnd` reports v2 tool ids (`shell`, `patch`, `subagent`, ...); touched paths come
  from edit / write input paths and `patch` patchText headers.
- All existing OpenCode 1 tests stay green.

## Decisions (user, 2026-10-21)

1. **One "OpenCode" harness.** v1 or v2 is chosen from the parsed `opencode --version` (v1 prints `1.19.3`, v2 prints
   `opencode v2.1.2`). v1 support is unchanged.
2. **`2.1.` is a supported version prefix.**
3. **One private `opencode serve` per chat**, as on v1. The user's v2 background service is never started:
   `--standalone` on every CLI call that accepts it (session list / export / delete, run). v2 CLI commands that call
   the service (`debug agents`, `debug config`, `plugin list`, `mcp list`) are avoided; config files are read instead,
   as v1 already does.
4. **Hooks report v2 native tool ids** (`shell`, `patch`, `subagent`, ...). Templates and docs match both
   (`edit|write|apply_patch|patch`); `docs/hooks-format.md` gets a v2 table.
5. **Harness Skills get a V2 plugin entry** in harness-useful-skills (branch `feat/opencode-v2`), no release without
   the user's go.
6. **One commit per step** on `feat/opencode-v2`; parallel tracks work in git worktrees with `CARGO_TARGET_DIR` on
   `/home`; no push.

Standing v1 rules carry over: random per-launch password, 127.0.0.1 only, `OPENCODE_DISABLE_AUTOUPDATE=1`, never change
OpenCode config files from chat, never pass `--auto`, never call share / auth / upgrade / config-write routes.

Install hint: OpenCode 1 keeps its install script (npm `opencode-ai` on Windows). OpenCode 2 is
`npm install -g @opencode/cli` everywhere: the install script at opencode.ai installs OpenCode 1 releases only and
refuses 2.x (checked 2026-10-21).

## Architecture

- **Generation:** `OpenCodeGeneration { V1, V2 }` in `business/src/harness/opencode_generation.rs` (prelude
  re-export). `from_version(&str)`: major 1 → V1, major ≥ 2 → V2, unknown → V1.
- **Version hook:** `Harness::note_version(&self, Option<&str>)` (default no-op), called by `detect::detect` after the
  version probe (with `None` when the binary is missing). `OpenCode` is `OpenCode { generation: Arc<AtomicU8> }`
  with `with_generation()` for tests; every trait method that differs reads `self.generation()`. `SPAWN_ENV` is a
  const so helpers no longer need an `OpenCode` value.
- **Desktop shell:** `ensure_detected(state, harness)` in `src-tauri/src/commands/harness.rs` detects a harness without
  a cached detection before the Config and Extensions views and the Harness Skills install. Plugin actions, sync,
  sessions, chat and MCP registration already detect through `installed_binary` / `detected_binary` / `summaries`.
- **Chat:** `ChatProtocol::OpenCodeServerV2`; `OpenCode::chat_protocol()` picks it for V2;
  `AgentSession::OpenCodeV2(OpenCodeV2Session)` in `agent/engine.rs` (until S9 the arm fails with "OpenCode 2 chat is
  not available yet").
- **Shared with v1:** `opencode_server::http` becomes route-agnostic through an `ApiRoute` trait (method, target,
  blocks, is_stream) implemented by `Route` (v1) and `RouteV2`; `ServerPassword`, the SSE reader, `export_html`,
  `stats::usage`, `agent::process` / `agent::io`; the `mode_config` rule tables (v2 normalizes legacy
  `agent.<n>.permission`, verified: bash → shell).
- **New modules** (each file under 500 lines): `business/src/opencode_server_v2/` (`routes.rs`,
  `wire/{mod,envelope,session,catalog,prompt}.rs`, `events.rs`, `messages.rs`, `tools.rs`,
  `mapper/{mod,turns,tools,asks,quiet}.rs`, `modes.rs`, `stats.rs`); `business/src/agent/opencode_v2/` (`mod`,
  `startup`, `stream`, `prompts`, `history`, `settings`, `dialogs`, `approvals`, `probe`, `ops`, `runs`, `reports`);
  fake server `business/src/bin/fake_agent/opencode2{,_http,_events}.rs`. S1 declares every module as a doc-only
  stub so each later track fills only its own files.

## Steps

Order: S1 → (Track A: S3 → S4 ∥ Track B: S5 ∥ Track C: S2 → S6 → (S7 ∥ S8) → S9 → S10 → S11 → S12) → S13 → S14,
S15 in the skills repo. S7 also needs S5.

- **S1 version plumbing and skeleton** (backend, first, serial): `harness/mod.rs` (`ChatProtocol::OpenCodeServerV2`,
  `note_version`), `harness/detect.rs`, `harness/opencode_generation.rs`, `harness/registry.rs`,
  `harnesses/opencode/mod.rs` (supported `1.18.`, `1.19.`, `2.1.`; install hint per generation; `SPAWN_ENV` const used
  by `merged.rs`, `sessions.rs`, `ecosystem/installer.rs`); `agent/engine.rs` V2 arm returns `Unsupported` until S9;
  doc-only stubs for `opencode_server_v2/*` and `agent/opencode_v2/*`; src-tauri `ensure_detected`; this plan. Tests:
  `business/tests/opencode_generation.rs`.
- **S2 route-agnostic HTTP** (Track C): `opencode_server/http/client.rs`, `routes.rs` implements `ApiRoute` for
  `Route`. Pure refactor, existing tests prove it.
- **S3 config and extensions read** (Track A): `harnesses/opencode/paths.rs`, `config.rs`, `config/layers.rs` (no
  `config.json` on V2), `extensions.rs` split into `extensions/plugins.rs`, `extensions/mcp.rs`,
  `extensions/agents.rs`, `share.rs`, `sync/collect_opencode.rs`. V1 reads legacy keys only. Tests with fixtures
  `business/tests/fixtures/opencode2-home/...`, `opencode2_views.rs`, `sync_export_opencode2.rs`.
- **S4 writes** (Track A): `cli.rs` V2 `["plugin", "add", module]`; `plugin_remove.rs` V2 edits `plugins` plus legacy
  `plugin`; `McpTarget::OpenCodeConfig(OpenCodeGeneration)`; `ecosystem/mcp_merge.rs` `Shape::OpenCodeV2` (nested
  `mcp.servers`, entry `{type: "local", command: [bin, "mcp"]}`, no `enabled`; split to `mcp_merge/shape.rs` past 500
  lines); existing-entry lookup in `memory_register.rs`, `desktop_register.rs`, `mcp_register.rs` checks both shapes;
  `ecosystem/setup.rs::opencode_file`; `sync/plan_rows.rs` v2 command line; `merged.rs` V2 reads merged MCP names through `opencode api --standalone GET /api/config` (no background service)
  instead of `debug config` (which spawns the service). Tests `opencode2_plugin_actions.rs`,
  `opencode2_plugin_remove.rs`, `setup_mcp_register_opencode2.rs`, `sync_import_opencode2.rs`.
- **S5 sessions v2** (Track B): `sessions/v2.rs` and `sessions/transcript_v2.rs` (`session list --standalone --format
  json -n N`, `export --standalone`, `delete --standalone`, ms times, optional title, `Session not found:` →
  `NotFound`); `opencode_server_v2/messages.rs` (`SessionMessage` union → transcript and history model) and
  `tools.rs` (v2 tool ids, touched paths). Fixtures `business/tests/fixtures/sessions/opencode2/*.json`; tests
  `sessions_opencode2.rs`, `sessions_opencode2_transcript.rs`.
- **S6 wire, routes, events** (Track C): `RouteV2` allowlist (info, event, session create / get / patch / delete, fork
  `{before?}`, agent and model switch, prompt, interrupt, command, shell, compact, message list, permission list /
  reply, form reply / cancel, agent / command / config / model / provider lists); `location[directory]` encoding; id
  checks; wire envelope `{data}` / cursor, session object, catalogs, prompt body `{text, files, delivery: steer|queue}`;
  `events.rs` typed SSE envelope, unknown types ignored. Tests `opencode2_routes.rs`, `opencode2_wire.rs` against
  `business/tests/fixtures/opencode2/recorded/`.
- **S7 mapper** (Track C, after S5 and S6): text and reasoning deltas, step / execution, status idle / busy / retry,
  tool input / called / progress / success / failed → tool cards and `ToolEnd` with v2 ids and touched paths,
  `permission.asked` / `replied`, `form.*` → dialogs, compaction and shell as quiet runs. Tests `opencode2_mapper.rs`,
  `opencode2_asks_mapper.rs`.
- **S8 fake v2 server** (Track C, parallel with S7): `bin/fake_agent/opencode2*.rs`, `--dialect opencode2`, Basic auth,
  `{data}` envelope, SSE heartbeat comments, scripted events; `business/tests/opencode2_engine_support/mod.rs`,
  `fake_opencode2.rs`; fixtures from real recordings.
- **S9 engine core** (Track C): `agent/opencode_v2/{mod,startup,stream,prompts,history,settings}.rs` — listening line,
  `/api/info` readiness (503 retry), event stream, session create / resume (checks the directory), catalogs, prompt,
  steer / queue, interrupt, model / agent / variant pickers, SSE overflow → reconnect and refetch;
  `harnesses/opencode/launch.rs` V2 args without `--mdns`, `OPENCODE_PASSWORD` added to the stripped env prefixes;
  `engine.rs` arm. Fixtures `opencode2_{smoke,startup,turn,tools,steer,follow_up,abort,resume,crash,no_listen}.jsonl`;
  tests `opencode2_engine*.rs`.
- **S10 approvals and modes** (Track C): `opencode_server_v2/modes.rs`, `agent/opencode_v2/{approvals,dialogs,probe}.rs`;
  mode rules in the server's `OPENCODE_CONFIG_CONTENT` as `agents.<n>.permissions` per agent; replies
  `{decision: once|always|reject}`; forms answered or cancelled; helper chat session with deny-all permissions,
  deleted at stop.
- **S11 ops** (Track C): `agent/opencode_v2/{ops,runs,reports}.rs`, `opencode_server_v2/stats.rs` — new, resume, fork
  (`before`), clone (fork without `before`), rename (PATCH title), compact, HTML export through the shared
  `export_html`, stats from cost and tokens, context meter, last reply, `!command`, slash commands, images, AI naming.
- **S12 hooks and routines tests** (Track C): `src-tauri/src/hooks/tests_opencode2.rs`,
  `src-tauri/src/routines/tests_agent_opencode2.rs`, fixtures `opencode2_hooks.jsonl`,
  `opencode2_routine_{write,ask}.jsonl`.
- **S13 UI** (frontend, only if needed): mock fixture `ui/src/lib/ipc/mock/fixtures/harness_list.json` v2 entry; copy
  rules (no semicolons, "Harness family").
- **S14 docs:** `hooks-format.md` v2 tool table and matcher example `edit|write|apply_patch|patch`;
  `opencode-support.md` v2 column; `architecture.md` / `adding-a-harness.md` on `note_version` and `ChatProtocol`;
  finalise the status table.
- **S15 Harness Skills V2 entry** (harness-useful-skills, branch `feat/opencode-v2`): `opencode/v2.js` default export
  `{id: "useful-skills", setup(ctx)}`; v1 `experimental.chat.system.transform` → `ctx.session.hook("context", ...)`;
  `chat.message` → `session.hook("prompt")`; tool hooks → `ctx.tool.hook("execute.before" | "execute.after")`; events
  `ctx.event.subscribe`; `command.execute.before` → `ctx.command.transform` or a prompt rewrite; no toast. Check
  whether v2's legacy auto-discovery of `plugins/` breaks on the v1 entry and whether v1 tolerates a default export
  (one dual entry, or the installer picks the entry). Build script, installers, `test/opencode-v2.test.js`. No
  release.

## Status

| # | Step | Owner | Status |
| --- | --- | --- | --- |
| S1 | Version plumbing and skeleton | backend | done |
| S2 | Route-agnostic HTTP | backend | done (46e6559) |
| S3 | Config and extensions read | backend | done |
| S4 | Config writes, plugins, MCP registration | backend | done (merged 306c96e) |
| S5 | Sessions v2 | backend | done (00afe63) |
| S6 | Wire, routes, events | backend | done |
| S7 | Event mapper | backend | done (merged de6dab8) |
| S8 | Fake v2 server | backend | done (merged 0a2dd27) |
| S9 | Engine core | backend | done (merged 6d4195b) |
| S10 | Approvals and approval modes | backend | done (merged 25a8e72) |
| S11 | Session ops, export, stats | backend | done (merged; S11b skills, background commands, summed stats 6900116) |
| S12 | Hooks and routines tests | backend | done (merged d04a536) |
| S13 | UI (only if needed) | frontend | done (merged e4573f7) |
| S14 | Docs | general | done (bbab074, final wording after S12) |
| S16 | Orphaned chat servers killed on next start (user-approved addition) | backend | done (merged 1cb9454) |
| S15 | Harness Skills V2 entry | general | done (harness-useful-skills b285e6f, branch feat/opencode-v2, unreleased; dual entry checked on real 1.19.3 and 2.1.2) |

## Verified facts

From the real OpenCode 2.1.2 binary in a sandbox (`unshare`, no provider) and recordings of real traffic with the free
`opencode/big-pickle` model (2026-10-21). The replayable recordings are under `business/tests/fixtures/opencode2/recorded/`.

**Server and wire**

- `serve --hostname 127.0.0.1 --port 0` with `OPENCODE_SERVER_PASSWORD` prints `server listening on http://127.0.0.1:<port>`
  on stdout. `--mdns=false` is rejected.
- `GET /api/info` answers 401 without auth and `{version, pid, urls, paths, capabilities}` with Basic `opencode:<password>`
  (503 while starting).
- Responses are wrapped in `{"data": ...}`, lists add `{"cursor": {previous, next}}`, catalogs add `{"location":
  {directory}}`. A directory scopes a request as `?location%5Bdirectory%5D=<absolute dir>`.
- Session object: `{id: "ses_...", projectID, cost, tokens, time: {created, updated}, location: {directory}, title?}`.
  Times are millisecond numbers, `title` is absent until set.
- `GET /api/command` lists `init` and `review`, `GET /api/model` lists models (`opencode/big-pickle` with the public key),
  `GET /api/agent` starts empty for a directory and fills 150–300 ms after the first request there. `GET /api/skill`
  lists `{id, name, description, path, content}` (built-ins `opencode` and `report` always present) and fills the same way.
- Legacy `agent.<n>.permission` in `OPENCODE_CONFIG_CONTENT` is normalized by `GET /api/config` to
  `agents.<n>.permissions` rules (`bash` becomes `shell`). Native `agents.<n>.permissions` lists are accepted.
  Without config, shell runs without asking.

**Events and runs**

- No `session.status` events. A run is `session.inbox.enqueued` → `session.execution.started` → `session.inbox.delivered` →
  `session.step.started` → text / reasoning / tool events → `session.step.ended` → `session.usage.updated` →
  `session.execution.succeeded`, `.failed` or `.interrupted {reason: "user"}`. `session.renamed` arrives early
  (automatic title).
- Tool events: `tool.input.started|delta|ended`, `tool.called {id, input}`, an optional `permission.asked`,
  `tool.progress`, then `tool.success` or `tool.failed {error: {type, message}}`. Ids seen: `read`, `glob`, `edit`,
  `write`, `shell`, `question`. Inputs: `edit {path, oldString, newString}`, `write {path, content}`, `shell {command}`,
  `read {path}`.
- `permission.asked {id: "per_...", action, resources, save, metadata, source}`. `GET /api/session/:id/permission` lists
  the pending ones, `POST .../permission/:id/reply {decision: once|always|reject, message?}` answers 204 and emits
  `permission.replied`.
- The `question` tool emits `form.created` (fields with `key`, `title`, `options`). `POST /api/session/:id/form/:formID/reply
  {answer: {q0: "blue"}}` answers 204, then `form.replied`.
- Steering and queueing: `POST .../prompt` with `delivery` `steer` or `queue` while running answers 200, delivered at
  boundaries. No 409 was seen while busy (`shell` answers 204, `compact` 200).
- `interrupt` answers 200 `{"interrupted": true}`, the tool fails with error type `aborted`.
- `shell` answers 204 (events `session.shell.started|ended`, then a synthetic item with the output). `command {name: "review",
  text: ""}` answers 204 and enqueues the template. `PATCH` title answers 204 and emits `session.renamed`.
- `fork {before: <messageID>}` answers 200 with `fork.boundary.type` `before`, `fork {}` with `through`. `compact` answers
  200 with a compaction item, then `session.compaction.started|ended`.
- Images go in `files: [{uri: "file:///abs/path.png", name}]`, the server inlines the data.

**CLI**

- `session list --standalone --format json` prints `[{id, updated, created, projectId, directory, title?}]`, `[]` when empty.
  `session export --standalone <id>` prints `{info, messages}`. `session delete --standalone <missing>` prints `Session
  not found: <id>` with exit code 1.
- Without `--standalone`, `debug config`, `plugin list`, `mcp list` and `debug agents` start a persistent `opencode serve
  --service`. With it no daemon is left.
- `api --standalone GET /api/config` returns the config sources the private server merged: `{type: "directory", path}`
  and `{type: "document", info}`.

**Source (2.1.2)**

- `cli/src/server-process.ts` reads `OPENCODE_CONFIG_PROJECT_DISABLE ?? OPENCODE_DISABLE_PROJECT_CONFIG` and
  `OPENCODE_FILEWATCHER_DISABLE ?? OPENCODE_DISABLE_FILEWATCHER`: the newer name wins when both are set.
- `core/src/config/discovery.ts` skips the project walk only for the system and the global config directory. `~/.claude`
  and `~/.agents` are global roots for skills, not a reason to skip the walk.

## Engine decisions (S9–S11b)

- Follow-ups are held by the app (mapper queue), steering goes as `delivery: steer` with an app-made `msg_` id.
- Always allow lasts for the chat: OpenCode only ever gets `once` or `reject`, the chat answers matching asks itself.
- Skills run as a prompt with `skills: [{id}]`, never through `/api/experimental/session/:id/skill`.
- Commands and skills start their turn in the background, a refused one shows an error result (as v1).
- Stats sum the session's replies and compactions (as v1), so a fork counts its copied messages.
- Images go as `data:` URLs in `files`.

## Verification

- Each Rust step: `cargo test -p harness_business --test <new tests>` plus the existing `opencode_*` tests,
  `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test -p harness_desktop` when src-tauri changed.
  UI: `bun run check`, `bun run test`. Skills: `node --test` with coverage, `node scripts/build-opencode-plugin.js
  --check`.
- Sandboxed real v2 end-to-end run (under `unshare`, no provider): detection, empty-project session list, create /
  list / export / delete through app code, chat start → ready, pickers, rename and fork, approval-mode config through
  `GET /api/config`, plugin add / remove in a temporary HOME, MCP register → `GET /api/config` shows `mcp.servers`;
  `pgrep -f "serve --service"` empty after each.
- At most 3 review and 3 security passes, then the user's real-provider checklist.

## Risks

- Harness Skills entry loader compatibility between v1 and v2.
- `/api/agent` (and `/api/skill`) start empty for each directory and fill 150–300 ms after its first request: startup asks again
  for up to 3 s, the build/plan fallback only covers a list that stays empty.
- A project's saved OpenCode allow rules (from the TUI's "always") apply after every agent's rules, so Always ask and
  Write ask first are refused in such a project (Yolo still starts).
- Subagent permission inheritance; PATCH permissions semantics (not used).
- Serve exposure through the user's config (check the cli serve handler; the client refuses non-loopback).
- Verified by recordings (business/tests/fixtures/opencode2/recorded): no 409 while busy (shell 204, compact 200),
  images as `files[{uri: file://…}]`, form reply `{answer:{key:value}}`, no `session.status` events (a run ends with
  `session.execution.succeeded|failed|interrupted`).
- `--standalone` spawn cost; `opencode2`-only installs are ignored (the binary name stays `opencode`).
- The generation is stale until the next detection (an upgrade from 1 to 2 needs a re-detect).
- Harness Skills under `OPENCODE_CONFIG_DIR`: the skills installer writes to `~/.config/opencode` only, which OpenCode 2
  does not read when the variable is set, while the app's record check still says installed. Documented in
  [opencode-support.md](opencode-support.md#opencode-2-v2-column), the fix belongs in the installer.
- A downgrade to OpenCode 1 with a shared config file: `mcp.servers.<name>` entries written for OpenCode 2 are not an
  OpenCode 1 shape and must move back to `mcp.<name>`.
