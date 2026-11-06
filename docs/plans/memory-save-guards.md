# Memory save guards (issue #39)

Saving became opt-in in issue #55, see [Opt-in (issue #55)](#opt-in-issue-55). Lines below that still say a repository
is "turned off" or list `offRepos` describe version 1 of `guards/global.json` and are superseded by that section.

Users decide what agents may save to Harness Memory: per chat, per repository, everywhere, and "ask before saving".
Reading and recall are never guarded. The UX is fixed by the approved mockups (Memory button in the chat top bar,
Notes and Scripts cards in its popover, a waiting card in the transcript, Notes and Scripts cards in Settings > Memory).

Guards stop agents that use the harness-memory MCP tools. They are not a sandbox: an agent with a shell can edit files
under the user's home or run CLIs, the same limit as script trust today.

## Decisions

- Off or paused blocks every agent write of that kind: `note_save`, `note_update`, `note_delete`, `entity_link`,
  `entity_delete` (notes) and `script_save`, `script_delete` (scripts). Ask before saving holds only `note_save`,
  `note_update` and `script_save`. Deletes and links pass in Ask mode.
- A repository switch (opt-in: turned on, formerly opt-out: turned off) applies to everything a chat in that repository writes, global-scope notes and scripts included.
  The chat's repository is the git root of the resolved `project_path`, else of the MCP server's working folder (and,
  for version 2 with no chat root, the working folder counts besides `project_path`).
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
in `approval.rs`. Chat and held files have `"version": 1`, `guards/global.json` has `"version": 2`; an unknown version counts as unreadable.

### `guards/global.json` (written by the desktop, read by the server on every gated call)

```json
{ "version": 2,
  "notes":   { "paused": false, "everyProject": false, "onRepos": ["/abs/saving/root"],
               "repoKeys": { "/abs/saving/root": "git:host/owner/repo" } },
  "scripts": { "paused": false, "everyProject": false, "onRepos": [], "repoKeys": {} } }
```

Missing file = saving off for the desktop, which writes it (off) before any agent starts, and no rules for the server only
while the marker `guards/opt-in` does not exist (an older desktop has chats and no file and no marker). The desktop writes
that marker (`{"version":1}`, `0600`, atomic, never through a symbolic link) first, whatever the state of `global.json`,
so once it exists a missing file is `guard-unreadable` for the server. Unreadable or invalid file = refuse (fail closed). Repos are absolute saving roots
(see below), compared after mapping the chat's repository the same way. Version 1 (`offRepos`, saving on) is described
under Opt-in.

### `guards/chats/<chatKey>.json` (written by the desktop at chat start and on every toggle)

```json
{ "version": 1, "paused": false,
  "notes":   { "on": true, "ask": false },
  "scripts": { "on": true, "ask": true } }
```

A missing file refuses when the server has a `HARNESS_MEMORY_SESSION` (the desktop writes the file before the chat starts, so a missing one is a chat it did not set up), and means no chat rules when there is no session variable. Unreadable or invalid file = refuse.

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

1. Global pause of the kind, then saving on for the project (`everyProject`, else `onRepos` holds the chat's saving
   root); otherwise refused as `repo-off`.
2. No `HARNESS_MEMORY_SESSION`: allow.
3. Chat file missing: refuse (the session variable is set, so the desktop should have written it). Invalid: refuse.
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
- Repository check, version 1 (`offRepos`): the git root of `project_path`, else that of the server's working folder
  (and the chat's `repoRoot`): any one off refuses. Version 2 (`onRepos`): the same, and with no chat root the working
  folder also counts when `project_path` names another project, so every one of them must be on.
- `notes update <id> --json [--project PATH]` (repository notes need `--project`). Unknown id gives `notFound`.
- `scripts save` prints `{"name","scope","id","created","approval"}`. Trust follows `script_save` rules: the desktop
  calls `trust` afterwards as today when `approval` is `needed` and the user saved the code.
- Errors with `--json`: stdout `{"error": "<code>", "message": "..."}`, exit 1, codes `notFound`, `gone`, `invalid`,
  `refused`. Usage errors exit 2.
- `held save` keeps the held file when the note is `gone`. Edited fields replace the agent's; unknown fields give `invalid`.

## Desktop contracts

- `AgentStartRequest.memoryChat: Option<MemoryChatStart { key: String, guards: MemoryChatGuards }>`.
- `MemoryChatGuards { paused: bool, notes: KindGuard, scripts: KindGuard }`, `KindGuard { on: bool, ask: bool }`.
- `MemoryGuards { notes: GlobalKindGuard, scripts: GlobalKindGuard }`, `GlobalKindGuard { paused: bool, everyProject: bool, onRepos: Vec<String> }`
  (was `offRepos`, see Opt-in).
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
  chat where everything may be saved starts with nothing to tell, as does one where Harness Memory takes no part (Project memory off, not installed, or a server without `save-guards`). Hook follow-ups, `/` and `!` commands and utility
  agents never carry it. A new, switched, forked or cloned session starts over, and a message that was not delivered
  tells it again on the next one. Texts, all starting `Harness Desktop: `:
  - `saving notes and scripts is on again for this chat.` (back to everything saved)
  - `saving notes and scripts is off for this project.` (both kinds with the same reason)
  - `saving notes is off for this chat. Scripts are saved.` (one sentence per kind otherwise)
  - Reasons: `is paused everywhere`, `is off for this project`, `is paused for this chat`, `is off for this chat`,
    `is off, the memory settings cannot be read`. With Ask on: `notes you save wait for the user's review.`
- **Save follow-up prefix (desktop).** While only one kind may be saved, "Harness Memory - save" starts with one
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
| Codex | 0.154.0 | only when named | Codex starts stdio MCP servers with a cleared environment plus the server's `env_vars`/`env`. After the handshake a chat with a memory identity asks Codex for the project's effective servers (`config/read{cwd}`: every layer Codex loads there, trust, profiles and managed settings included) and sends `"mcp_servers.<name>.env_vars": [<its own…>, "HARNESS_MEMORY_SESSION"]` in the `config` of every `thread/start`, `thread/resume` and `thread/fork` for each enabled harness-memory stdio server it listed (read again before each new, resumed or forked thread). Real CLI: the server started for that thread sees the variable. Nothing goes on the command line (so `codex.cmd` works too) and nothing is written to `config.toml`. A server Codex did not list is never named (Codex fails such a thread with `invalid transport`). A server with a dotted name or a non-list `env_vars` gets nothing: the chat then reports `memoryGuards: false`, hides the Memory button and shows a notice. Plan `codex.md` decision 11 |

All four keep `memoryChatGuards: true`. Per chat, `AgentSessionInfo.memoryGuards` says whether the key reached Harness Memory (false for a chat without one, and for a Codex chat whose servers could not be told), and the Memory button shows only then. Not tested: OpenCode 2.x, macOS, Windows.

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
- Repository opt-out also matched by project key (version 1 only): the server resolved the project key (`git:<remote>`
  etc.) of each `offRepos` entry and of the chat's repositories, so worktrees and other clones of an opted-out repository
  were covered. Opt-in replaces this: see the saving root below, a separate clone is not covered any more.
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

## Opt-in (issue #55)

Saving notes and scripts is off until the user turns it on. This flips version 1, where it was on until the user turned
a repository off.

- **`guards/global.json` version 2.** Per kind `paused` (stops the kind everywhere first), `everyProject` (on for every
  project) and `onRepos` (the saving roots where it is on). `offRepos` is removed. Chat files stay version 1.
- **Order.** Gate order is unchanged: global pause, project (`everyProject` or the project's saving root in `onRepos`
  with its pinned key, else refused as `repo-off`), then the chat's pause, the chat's switch, Ask. The chat's own switches default to on, so a
  project that is on saves unless the chat turned a kind off. A missing `global.json` means saving is off for the desktop
  (it writes the marker `guards/opt-in` and then the file before any agent starts). For the server it is no rules while
  the marker does not exist, and unreadable (refuse) once it does.
- **Saving root.** A git worktree counts as its main checkout, or as its git folder when that belongs to no checkout, a
  separate clone does not. Starting from the project root: when `<root>/.git` is a file `gitdir: X` (`X` absolute or
  relative to the root) and `X/commondir` exists (a path relative to `X` or absolute), the common directory is canonical
  `X/commondir`. The worktree must point back: `X/gitdir`, read and canonicalised, has to equal the canonical
  `<root>/.git`, and canonical `X` has to be a child of `<common>/worktrees`. Together this stops a folder's own files
  (a forged admin folder inside it, or elsewhere with a good back-link) from claiming another repository. The mapping
  (`map(root)` in `saving_root.rs`, the same in `src/mcp/worktree.rs` of Harness Memory) is one of three: `Itself(root)`
  (anything that fails a check above, a submodule without `commondir`, a file missing, oversized or unreadable), `Checkout(main)`
  (a verified worktree whose common directory is named `.git`: its parent, matched to entries by path) and
  `GitFolder(common)` (a verified worktree whose common directory is not named `.git` AND is shaped like a git folder:
  `HEAD` a regular file, `objects` and `refs` folders, no `.git` inside; a separate git dir made with
  `git init --separate-git-dir`, or a bare repository, has no back-link to a checkout). A common directory of any other
  shape (a work tree that holds a forged `worktrees/<name>` folder with both files) leaves the root `Itself`. Reads are
  small and bounded, trailing newlines are trimmed. The `.git` file is exactly one line `gitdir: X` and each file is
  read up to 16 KiB on both sides. A `GitFolder` root is on only by an `onRepos` entry that is that git folder, or by
  the entry of the main checkout that keeps its git folder there (a `.git` file of one line `gitdir: X`, `X` shaped like
  a git folder, no `commondir`); neither matches a root that maps to `Itself` or `Checkout`, so a listed folder cannot
  vouch for another repository by naming it. An entry that is itself a verified worktree turns on its `Checkout` main
  and its own path. So with `main` listed (a checkout with a separate git dir), its worktree `wt` is on (it maps to the
  git folder) and `main` by its path, while a forged folder stays off. Adding from `wt` stores the git folder, which
  turns on `wt` and the repository's other worktrees but not `main`. `repo_is_on(root, on_repos)` (`saving_root.rs`)
  takes the project root and maps it, and the server does the same in `all_on` (`src/mcp/guard_repos.rs`). Not stopped:
  someone who can already write inside a repository's own git folder, or a `.git` file in a listed checkout that names
  the git folder of another repository (that checkout then turns on that git folder's worktrees). `addRepo` stores the
  saving root (a folder shaped like a git folder is stored as given, not as the repository whose work tree holds it),
  `removeRepo` accepts the listed string or any folder inside it, `memory_saving_root(folder)` returns a folder's
  saving root for the UI to compare with `onRepos`, or the listed entry that turns it on, so it can change after a
  guards update. The chat file's `repoRoot` stays the chat's project root, the server maps worktrees itself.
- **An entry names itself.** An entry names something only when `dunce::canonicalize(entry)` equals it exactly, as text
  (`names_itself` in `saving_root.rs`, `Allowed::of` in `src/mcp/worktree.rs`), checked before any expansion (`map`,
  `separate_git_dir`, `is_git_folder`). One with a link in it, or whose folder is gone, names nothing: it allows no path,
  no `Checkout` and no `GitFolder`, so a listed link no longer follows its target, and `memory_saving_root` for the
  target does not answer it (the UI says off). Why: a default `git pull` can replace a listed submodule or nested clone
  `U/sub` with a tracked link `sub -> ../O` (also `libs -> ../Op` above an entry `U/libs/sub`, an absolute target, a
  fresh clone), and the entry would then turn on `O`. `addRepo` stores canonical paths on every branch; an entry stored
  while its folder is gone keeps the raw string and names something only once the folder exists as itself. The canonical
  form is `dunce::canonicalize`'s output and is compared exactly, so on a case-insensitive file system or with another
  Windows drive-letter spelling a different spelling names nothing (a mismatch only turns saving off). Cost: a user who
  moves a project and leaves a link at the old path turns it on again. `removeRepo` of the literal entry always works;
  an entry that names nothing is removed alone, and removing a checkout also removes the git folder its
  `separate_git_dir` names. The cases are `saving_root_links.rs` here and `tests/mcp_guard_links.rs` in Harness
  Memory (`LAYOUTS` in step).
- **Pinned keys (`repoKeys`).** Per kind, a required object: entry path to the project key the repository had when the
  user turned it on (`null` for a folder with no project, such as home). It is part of version 2 (no version bump: 2 had
  not shipped). The server (`all_on` in `src/mcp/guard_repos.rs`) compares each repository of the call as a pair
  (root, key), the chat root and the working folder included: `Allowed` must match the root as before AND the pin of an
  entry that allows it must equal that repository's key (two keys made from a path match each other, since a linked
  worktree of a repository with no remote has a path key of its own). `everyProject` skips pins, version 1 is unchanged,
  and a refusal stays `repo-off`. An entry with no pin names nothing, and the server reads a version 2 file with no
  `repoKeys` as having none (the file stays usable). Why: an upstream change that retargets a remote (`git remote
  set-url`, a fresh clone at the same path, an edit of `.git/config`) gives a listed folder the key, and so the notes, of
  a project that is off. A legitimate remote change stops saving the same way until the project is turned on again.
  The desktop cannot compute keys, so its own rule (`repo_is_on`, `SaveRule`) stays path-only and an entry with no pin
  counts as dead (`deadRepos`); the hook notice can therefore say saving is on while the server refuses after a key
  change, and the refusal tells the agent while Settings tells the user (`changedRepos`).
  `harness-memory project-key --json` (user-only, listed under `save-opt-in`, unreachable through MCP) reads a JSON array
  of folder paths on stdin (at most 512, 4096 bytes each) and prints `[{path, root, key}]` (`key` `null` for no
  project, `{path, error}` for a path it cannot use). `memory_guards_update` with `addRepo` asks it for the folder the
  user turned on (not the stored saving root: a worktree of a separate git folder or a bare repository has a `git:` key
  while the git folder alone has a `path:` key) through the existing process runner, then writes the entry and its pin
  under the lock; a failed ask fails the add with a readable message and nothing is written. Turning a listed repository
  on again replaces its pin (how a changed remote is accepted). `removeRepo` and every write drop the pins of entries
  that are not listed. `memory_guards_get` and `memory_guards_update` also fill `GlobalKindGuard.changedRepos` (entries
  whose current key differs from the pin, from one batched `project-key` call for both kinds; empty and logged when the
  server cannot be asked; git folders are left out because the folder alone has a different key than the worktree it
  was pinned from), never written to the file. A version 2 file with no `repoKeys` is rewritten with `repoKeys: {}` and
  no entries by `ensure_global`, so nothing shows as on that the server refuses, and does not take the quick way of `is_current`.
- **Submodules.** A work tree that is a submodule (git reports a superproject, or its git data sits in a `.git/modules`
  folder) is keyed by its path (`path:<root>`) and its remote is never read
  (`identify` in Harness Memory's `src/project.rs`: `git rev-parse --show-superproject-working-tree` prints a
  superproject, or the module's main work tree has one for a linked worktree of a submodule), because an upstream change
  of `.gitmodules` brought in by `git submodule sync` or `clone --recurse-submodules` would otherwise give it another
  repository's key. Nothing is migrated: notes saved under the old `git:` key stay there and are no longer read from the
  submodule, and a submodule's notes are machine-local from now on.
- **Patch.** `MemoryGuardsPatch` gains `everyProject?: bool`. `addRepo` turns a project on, `removeRepo` turns it off.
  At most 256 `onRepos` per kind (formerly `offRepos`).
- **Migration.** `ensure_global` runs in the app's `setup` right after the update switch, at the start of every
  agent (`keep_guards_current`, `business/src/agent/launch.rs`: chats and routine runs) and first when
  `memory_guards_get` reads, but only on a computer that uses Harness Memory: when its data folder exists or the
  `harness-memory` binary resolves (`guards_data_dir`). Otherwise nothing is created and nothing can fail a start.
  A start with a memory chat counts as using it (its chat file goes to that folder). It runs in `spawn_blocking`.
  The start fails only when it has a memory chat, a readable version 1 file remains after the failure (the server
  would keep applying the old deny list with saving on), or neither `global.json` nor the marker exists (`rules_missing`:
  the failure came before the marker was written, so the server would apply no rules). Any other failure logs a warning
  and the agent starts, and a utility agent never fails. While Project memory is off the app removed its entry, so only the
  `rules_missing` case fails, and only when this harness's MCP files (`also_read` files included) still list a
  `harness-memory` server (`memory_mcp_listed`; a file that cannot be read counts as listed): an OpenCode config with
  comments, an entry the app does not own, a file change that failed or a harness not detected at switch-off keep it
  configured, and with no rules it would save anywhere. The failure is an `io` error "Harness Memory settings could not be updated: <reason>" (the
  UI shows `io` as it is, `forbidden` would be replaced by a generic text, `protocol` and `timeout` get an
  "agent is up to date" hint). It holds the lock of `memory_guards_update`: a mutex and an exclusive lock on
  `guards/global.lock`, tried under one 5 s deadline, then a `protocol` error "settings are busy". It writes the marker
  `guards/opt-in` before anything else (a failure is an error). A missing file
  or a version 1 file is written as version 2 with `everyProject: false`, `onRepos: []` and `repoKeys: {}`, keeping the
  pauses of a readable version 1 file and dropping its `offRepos`. A version 2 file is not touched, unless a kind has no
  `repoKeys` (written before the pins): it is written again with `repoKeys: {}` and no entries, so those projects are
  turned on again. A file that cannot be read (bad
  JSON, a symbolic link, a foreign owner, another version) is left alone with a logged warning, the server refuses it.
  After uninstalling the desktop the marker and the version 2 file stay, so saving stays off with no UI to turn it on
  until `guards/` is removed, and an older desktop treats version 2 as unreadable. Not fixed: another local user with
  write access to a data folder that is not private can remove both files, and the server does not check the folders
  before accepting "no rules", because a standalone user's existing folder could break.
  Until the move ran, `memory_guards_get` converts a version 1 file in memory the same way, never exposing `offRepos`,
  with `MemoryGuards.migrationPending: true` (computed on read, never written, `GlobalFile` stays `{version, notes,
  scripts}`). `memory_guards_get` tries the move first and logs a failure, so a file that became writable heals on the
  next read. A write over 256 KiB (many long paths) is refused with `invalidInput` and the file stays as it is.
- **Feature.** `save-opt-in` in `harness-memory features` (`MemoryRemote.saveOptInSupported`). `saveGuardsSupported`
  keeps its meaning.
- **Telling the agent.** The first message of a chat whose project is not turned on carries
  `Harness Desktop: saving notes and scripts is off for this project.`, a chat where saving is on says nothing. The
  memory-save hook is skipped while both kinds are off.
- **One-time notice.** Pref `memoryOptInNoticeDone` (server-managed, `#[serde(default)]`): a new prefs file starts `true`,
  an existing file reads `false`, and unreadable prefs load `false` so the notice is not lost. The UI reads it from
  `prefs_get` and marks it with `memory_opt_in_notice_done()`.
- **Known limits.** See "Known limits of opt-in" in `docs/security-privacy.md` for the full list, routines and clones included. (1) A Harness Memory without `save-opt-in` refuses every save against a version 2 file. (2) A server
  from before save guards ignores the file, agents save as before. (3) Scripts on with notes off leaves agent-saved
  scripts untrusted until the user approves them.
