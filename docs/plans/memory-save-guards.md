# Memory save guards (issue #39)

Users decide what agents may save to Harness Memory: per chat, per repository, everywhere, and "ask before saving".
Reading and recall are never guarded. The UX is fixed by the approved mockups (Memory button in the chat top bar,
Notes and Scripts cards in its popover, a waiting card in the transcript, Notes and Scripts cards in Settings > Memory).

Guards stop agents that use the harness-memory MCP tools. They are not a sandbox: an agent with a shell can edit files
under the user's home or run CLIs, the same limit as script trust today.

## Decisions

- Off or paused blocks every agent write of that kind: `note_save`, `note_update`, `note_delete`, `entity_link`,
  `entity_delete` (notes) and `script_save`, `script_delete` (scripts). Ask before saving holds only `note_save`,
  `note_update` and `script_save`. Deletes and links pass in Ask mode.
- A repository opt-out applies to everything a chat in that repository writes, global-scope notes and scripts included.
  The chat's repository is the git root of the resolved `project_path`, else of the MCP server's working folder.
- One Pause per chat covers notes and scripts. Global pause is separate for notes and for scripts (Settings).
- The user's own saves (Notes and Scripts views, the waiting card) bypass every guard, through user-only CLI
  subcommands that agents cannot reach through MCP.
- All guard and held state is local to this computer, never in a team database.

## Session identity

The desktop gives every chat a random `chatKey` (lowercase UUID v4) and starts its harness with
`HARNESS_MEMORY_SESSION=<chatKey>`. The harness-memory MCP child inherits it. The server captures the variable
(`CAPTURED_VARS`). A value that is not a lowercase UUID makes every gated call refuse (fail closed).

Harnesses that do not pass the variable to MCP children get `memoryChatGuards: false` (capability): per-chat rules then
only skip the desktop's memory-save hook, repo and global guards still apply, and "Ask before saving" is disabled for
that harness with a one-line reason. Propagation results are recorded at the end of this file.

## Files (harness-memory data folder)

Same folder as `desktop.alive` (see harness-memory `src/mcp/approval.rs`): `<data>/harness-memory/`, overridden by
`HARNESS_MEMORY_DIR`. Folders 0700, files 0600, atomic writes (temp + rename), symbolic links refused, owner checked as
in `approval.rs`. Every file has `"version": 1`; an unknown version counts as unreadable.

### `guards/global.json` (written by the desktop, read by the server on every gated call)

```json
{ "version": 1,
  "notes":   { "paused": false, "offRepos": ["/abs/git/root"] },
  "scripts": { "paused": false, "offRepos": [] } }
```

Missing file = no global guards. Unreadable or invalid file = refuse (fail closed). Repos are absolute, canonical git
roots, compared after canonicalising the chat's repository.

### `guards/chats/<chatKey>.json` (written by the desktop at chat start and on every toggle)

```json
{ "version": 1, "paused": false,
  "notes":   { "on": true, "ask": false },
  "scripts": { "on": true, "ask": true } }
```

Missing file = no chat rules. Unreadable or invalid file = refuse.

### `held/<heldId>.json` (written by the server, read and removed by the user CLIs)

```json
{ "version": 1, "heldId": "uuid", "chatKey": "uuid", "createdAt": "RFC 3339", "client": "omp",
  "kind": "note", "tool": "note_save", "projectPath": "/abs/repo", "repoRoot": "/abs/git/root",
  "arguments": { "...": "the tool arguments as the agent sent them" },
  "current": null }
```

- `kind`: `note` or `script`. `tool`: `note_save`, `note_update` or `script_save`.
- `current`: for `note_update` the note as saved now `{ "id", "title", "body", "kind", "tags", "entities", "scope" }`;
  for a `script_save` that replaces a script `{ "name", "description", "language", "body", "timeoutSecs", "scope" }`;
  otherwise `null`.
- Limits: 50 held items per chat (the oldest is refused, not dropped: reply as refused, reason `held-limit`),
  512 KiB per file, swept after 30 days.
- Holding the same note title, the same note id (`note_update`) or the same script name again in the same chat
  replaces the earlier held item and keeps its `heldId`.
- Held items are not saved: not in the store, search, lists or trust.

## Gate order (server, before any gated tool body runs)

1. Global pause of the kind, then the kind's `offRepos` against the chat's repository.
2. No `HARNESS_MEMORY_SESSION`: allow.
3. Chat file missing: allow. Invalid: refuse.
4. Chat paused, or the kind is off in the chat: refuse.
5. Ask is on and the tool is holdable: hold if the desktop is alive (`desktop.alive` fresh, as for script approvals),
   else refuse with reason `no-desktop`.

## Replies to the agent (not errors, so the agent does not retry)

- Held: `Waiting for the user's review in Harness Desktop. Nothing is saved yet. Do not save it again or retry, the user decides. Continue with your task.`
  Machine-readable: `{"held": true, "heldId": "..."}`.
- Refused: `The user turned off saving notes for this chat. Nothing was saved. Do not retry. Continue without saving.`
  Wording per reason and kind. Machine-readable: `{"saved": false, "reason": "..."}` with reason one of
  `chat-off`, `chat-paused`, `repo-off`, `global-paused`, `no-desktop`, `held-limit`, `guard-unreadable`.

## User-only CLI (harness-memory, all print JSON with `--json`, none reachable through MCP)

| Command | Input (stdin JSON) | Output |
| --- | --- | --- |
| `notes save --json [--project PATH]` | `{title, body, kind, tags, entities, scope: "repo"\|"global"}` | `{"id": 12, "title": "...", "scope": "repo"}` |
| `notes update <id> --json` | `{title, body, kind, tags}` | `{"id": 12, "title": "..."}` |
| `scripts save --json [--project PATH]` | `{name, description, language, body, timeoutSecs, scope}` | `{"name": "...", "scope": "repo"}` (trusted, as the desktop save today) |
| `held list --json [--chat KEY]` | | `{"held": [HeldFile...]}` newest first |
| `held save <heldId> --json` | optional edited draft (note or script shape above) | `{"saved": {"kind": "note", "id": 12}}` or `{"saved": {"kind": "script", "name": "..."}}` |
| `held skip <heldId> --json` | | `{"skipped": "<heldId>"}` |

- `held save` of a script saves it untrusted (needs the user's trust before it runs), matching the card copy.
- `held save` of a `note_update` applies to the note's current version; a deleted note returns error code `gone`.
- Errors follow the existing `--json` error convention of the CLI, exit code non-zero.
- `features` lists `save-guards`.

## As built in harness-memory (deviations from the table above)

- Machine-readable replies are in MCP `structuredContent`; replies are `isError: false`.
- Refused deletes end with `Nothing was deleted. Do not retry. Continue with your task.`
- Repository check looks at the git root of `project_path` AND of the server's working folder: either one off refuses.
- `notes update <id> --json [--project PATH]` (repository notes need `--project`). Unknown id gives `notFound`.
- `scripts save` prints `{"name","scope","id","created","approval"}`. Trust follows `script_save` rules: the desktop
  calls `trust` afterwards as today when `approval` is `needed` and the user saved the code.
- Errors with `--json`: stdout `{"error": "<code>", "message": "..."}`, exit 1, codes `notFound`, `gone`, `invalid`,
  `refused`. Usage errors exit 2.
- `held save` keeps the held file when the note is `gone`. Edited fields replace the agent's; unknown fields give `invalid`.

## Desktop contracts

- `AgentStartRequest.memoryChat: Option<MemoryChatStart { key: String, guards: MemoryChatGuards }>`.
- `MemoryChatGuards { paused: bool, notes: KindGuard, scripts: KindGuard }`, `KindGuard { on: bool, ask: bool }`.
- `MemoryGuards { notes: GlobalKindGuard, scripts: GlobalKindGuard }`, `GlobalKindGuard { paused: bool, offRepos: Vec<String> }`.
- `HeldItem` mirrors `held/<heldId>.json` (camelCase), `arguments` and `current` as `serde_json::Value`.
- IPC: `memory_guards_get() -> MemoryGuards`, `memory_guards_update(patch) -> MemoryGuards` (was `memory_guards_set`, see the repairs below),
  `memory_chat_guards_set(key, guards)`, `memory_held_list(chatKey: Option<String>) -> Vec<HeldItem>`,
  `memory_held_save(heldId, edited: Option<serde_json::Value>) -> HeldSaved`, `memory_held_skip(heldId)`,
  `chat_memory_check_again(agentId)`. Event `memory-held-changed` (no payload) when the held folder changes.
- `MemoryFeatures` gains `saveGuards`. Harness capability `memoryChatGuards`.
- Prefs: `memoryNotesAskDefault`, `memoryScriptsAskDefault` (bool, default false), UI-managed.
- The memory-save hook is skipped when the chat is paused, or when notes and scripts are both off for this chat,
  its repository or globally. "Check again" sends the same hook text through the hook path, disabled in those states.
- Without `save-guards` in `harness-memory features`, the Memory button and the new Settings cards are hidden.

## Telling the agent

The agent learns where saving stands from three places, so it does not try saves that the guards refuse.

- **Memory settings notice (desktop).** After a guard change, the next eligible user message carries one
  `<hook name="Memory settings">` element (id `harness-desktop:memory-settings`), first in its
  `<harness-hook-context>`. It is told once per change, and a change undone before the next message tells nothing. A
  chat where everything may be saved starts with nothing to tell. Hook follow-ups, `/` and `!` commands and utility
  agents never carry it. A new, switched, forked or cloned session starts over, and a message that was not delivered
  tells it again on the next one. Texts, all starting `Harness Desktop: `:
  - `saving notes and scripts is on again for this chat.` (back to everything saved)
  - `saving notes and scripts is off for this project.` (both kinds with the same reason)
  - `saving notes is off for this chat. Scripts are saved.` (one sentence per kind otherwise)
  - Reasons: `is paused everywhere`, `is off for this project`, `is paused for this chat`, `is off for this chat`,
    `is off, the memory settings cannot be read`. With Ask on: `notes you save wait for the user's review.`
- **Save follow-up prefix (desktop).** While only one kind may be saved, "Save what you learned" starts with one
  sentence, then a blank line and the user's hook text. "Check again" sends the same text.
  - Notes off: `Saving notes <reason>: only consider scripts (script_save), save no notes.`
  - Scripts off: `Saving scripts <reason>: only consider notes (note_save, note_update), do not call script_save.`
- **Read notices (harness-memory).** The notes reads (`note_search`, `note_get`, `entity_neighbors`) and the scripts
  reads (`script_list`, `script_get`) end their reply with one line while saving that kind is not plainly allowed for
  the call. The read itself is unchanged. Scripts replies also carry `structuredContent.saving`:
  `{"allowed": false, "reason"}` or `{"ask": true}`. Examples:
  `Saving notes is off for this chat (the user's setting). Reading works. Do not try to save notes.`,
  `Scripts you save in this chat wait for the user's review before they are kept.`

## Propagation results

Linux, 2026-10-21. Each harness was started with `HARNESS_MEMORY_SESSION` set, and the environment of its
harness-memory MCP child was read from `/proc/<pid>/environ`.

| Harness | Version | Child inherits the value | Notes |
| --- | --- | --- | --- |
| OMP | installed `omp` | yes | MCP child started with the RPC session |
| PI | installed `pi` | yes | MCP child started with the RPC session |
| OpenCode | 1.19.3 | yes | MCP children start lazily (first `/mcp` or session use). Two servers at once each had their own child and value |

All three keep `memoryChatGuards: true`. Not tested: OpenCode 2.x, macOS, Windows.

## Review round 1 repairs (contract changes)

- Chat file gains `"repoRoot": "/abs/git/root" | null` (the chat's project root as the desktop resolves it). The
  server's repository check also matches this root, compared as canonical paths, so global-scope writes from an
  opted-out repository are refused whatever the MCP child's working folder is, and a repository rooted at the home
  folder works.
- Agent `script_save` is NOT trusted automatically while any guard applies to the call's chat or repository (chat
  notes/scripts off, Ask, pause, a repo opt-out of either kind, global pause of either kind): `script_run` then needs the
  user's approval that shows the code. Without guards, today's trust rules stay.
- Held files gain `"revision": "<sha256 hex of the canonical held content>"`. Re-holding the same key replaces content
  and changes the revision. `held save <heldId> --json --expect <revision>` refuses with error `changed` when the file's
  revision differs. `held save` claims the file first (atomic rename to a claim name), so two saves of one id cannot both
  write, and a re-hold during a save is kept as a new item.
- Held arguments are stored in parsed, normal form (trimmed scope and kind, canonical language, known fields only).
  Unknown fields are dropped. Note tags and entities and script descriptions are masked like the store masks them.
- A held `note_update` with `append` keeps it, and the card shows the resulting body (current body + append) in the
  diff. Save with no edit sends `--expect`, so the user saves exactly what was shown.
- A held script saved by the user is always saved as needing trust, even with trusted remote scripts on.
- `held list --json --chat KEY` is used per open chat by the desktop. The desktop's output cap for `held list` is 64 MiB.
- Scripts run by `script_run` do not inherit `HARNESS_MEMORY_SESSION`.
- Desktop global guard writes go through `memory_guards_update(patch)` (read-modify-write under a lock):
  `{ kind: "notes" | "scripts", paused?: bool, addRepo?: string, removeRepo?: string }` → `MemoryGuards`.
  `memory_guards_set` is removed.
- `memory_chat_guards_set(key, guards, revision: u64)`: the desktop ignores a write whose revision is not higher than the
  last one written for that key, so late writes cannot overwrite newer state.
- `dialog_pick_folder() -> Option<String>`: a folder picker that changes no project state, used by "Add repository…".
- The desktop deletes chat files untouched for 30 days at startup. A chat without a written chat file gets no session
  variable.
- "Check again" sends the text of the user's configured memory-save hook (falls back to the seed text).

## Review round 2 repairs (contract changes)

- Held-back scripts: a local-only table (outside the shared schema, like the hidden list), never pruned, records
  (place, script id or name, version/hash) for every agent `script_save` made while a guard applies and for every
  script saved by `held save`. `approval_of` returns Needed for a held-back version before any team trust, so
  `trust_team_scripts` cannot trust it. Only the user's `trust` clears it. Replaces the `script_seen_unapproved` marker.
- Repository opt-out also matches by project key: the server resolves the project key (`git:<remote>` etc.) of each
  `offRepos` entry and of the chat's repositories, and refuses when either the canonical root or the key matches, so
  worktrees and other clones of an opted-out repository are covered.
- `HARNESS_MEMORY_SESSION` set and chat file missing: refuse (`guard-unreadable`).
- Held layout: `held/<chatKey>/<heldId>.json` (one folder per chat, 0700). Per-chat byte cap 4 MiB besides the 50-item
  limit (reason `held-limit`). `held list --chat KEY` reads only that folder. Files in the old flat layout are ignored.
- `held save` of a `note_update` refuses `changed` when the live note's title, body, kind or tags differ from `current`.
- `held skip <heldId> --json --expect <revision>` refuses `changed` on mismatch (same claim logic as save).
- Desktop `memory_held_skip(heldId, revision)` passes `--expect`. The held watcher scans the per-chat folders and
  debounces change events (at most one per 500 ms).
- `memory_chat_guards_set` returns `{ applied: bool, revision: u64 }` (`revision` = the last written one for the key). The
  UI, on `applied: false`, moves its counter above `revision` and writes again once.
- The UI captures an item's revision when Edit opens and sends that one, closes the editor when the item's revision
  changes ("The agent changed it. Review it again."), and sends the revision on Skip.
- A toggle never drops the chat file's `repoRoot`: when the file is unreadable it is rebuilt from the chat's project.
- A data folder that other users can write to fails the chat start with a clear message naming the folder and the fix
  (`chmod 700`).
