# Hook file format (version 1)

The contract for hook files in Harness Desktop (Rust). One program reads and writes them in two modes: the app window (user, runs hooks) and its MCP server `harness-desktop mcp` (agents, stdio). Shared fixtures: `business/tests/fixtures/hooks/`.

A hook is a small JSON file. The app window reads the files, shows them in the Hooks view and runs them. The MCP server only reads and writes files on behalf of an agent, it never runs a hook.

Key words: MUST, MUST NOT, MAY as in RFC 2119. Lengths count Unicode code points, sizes count bytes. Invisible characters are Unicode general category `Cf` (bidirectional controls, zero-width characters, the soft hyphen, tags), the line and paragraph separators (`Zl`, `Zp`: U+2028, U+2029), the combining grapheme joiner (U+034F), the Hangul fillers (U+115F, U+1160, U+3164, U+FFA0), the Khmer inherent vowels (U+17B4, U+17B5), the Mongolian variation selectors (U+180B to U+180F), the blank Braille pattern (U+2800) and the variation selectors (U+FE00 to U+FE0F, U+E0100 to U+E01EF), the same set Share setup and Harness Memory reject: invisible, breaking or reordering, so what the user reviews could differ from what a file holds.

## Folders

Hooks root (`<root>`): `HARNESS_HOOKS_DIR` when set to an absolute path (a leading `~` expanded, a relative value ignored), else a platform base followed by `harness-hooks`:

| OS | Base |
| --- | --- |
| Linux | `$XDG_DATA_HOME` when absolute, else `~/.local/share` |
| macOS | `~/Library/Application Support` |
| Windows | `%LOCALAPPDATA%` when absolute. Unset: no hooks folder, hooks are unavailable |

Linux example: `~/.local/share/harness-hooks`. This mirrors `memory_data_dir` in `business/src/ecosystem/memory_requests.rs` with `harness-hooks` instead of `harness-memory`.

Hook files live in `<root>/hooks/`. Either app creates `<root>` and `<root>/hooks` when missing, mode `0700` on Unix. `<root>/team/` holds the app window's copies of team hooks (see Team hooks): no other writer touches it, and the MCP server never reads it.

File rules:

- Name is `<id>.json`, `<id>` is 32 lowercase hex characters (`^[0-9a-f]{32}$`). The `id` field inside MUST equal the name stem, otherwise the file is invalid.
- Written mode `0600` on Unix. UTF-8, no BOM, JSON, two-space indent, trailing newline (readers accept any JSON formatting).
- Written by a temp file in the same folder (mode `0600`), flushed, then renamed over the target. Temp names start with `.` and end with `.tmp` or contain `.tmp-` (the desktop uses `.<name>.tmp-<pid>-<nanos>`). Readers MUST ignore every name that is not exactly `<id>.json`, which covers temp files, leftovers and editor files.
- Readers ignore anything that is not a regular file (symbolic links included). On Unix, readers ignore the whole folder while `<root>` or `<root>/hooks` is owned by another user or writable by group or others, and a file another user owns is never parsed (it is listed as Invalid, "the file could not be read"). The app window applies the same owner and mode checks to `<root>/team` and its copies. A `<root>` the user owns that group or others can write to (an older version or a loose umask created it so) has its group and other write bits cleared by either app before it is checked (its other bits stay), and the change is logged once per folder. A `<root>` that is a symbolic link is never changed: the folder it points to is checked as it is.
- A file larger than 16 KiB (16384 bytes, raw) is Invalid and is not parsed.
- Writers MUST NOT follow a symbolic link at the target path.
- Per scope at most 64 hooks (global, and each project separately). The MCP server refuses to add a hook to a scope that holds 64 valid hooks, the app window to a scope that holds 64 approved ones. When a folder holds more, approved hooks come first, each group ordered by `createdAt` then `id` ascending: the first 64 of a scope are used and the rest are Invalid ("over the limit"). So files waiting for review never push an approved hook out. A hook over the limit is still listed, and deleted by "Delete all hooks", only with its own scope (global, or its project), while a file that is not a valid hook is listed with every scope.

Order of hooks when several apply: global before project, then `createdAt` ascending, then `id` ascending. Hooks of one event firing run one after another in that order, never concurrently.

## Hook object

Top-level fields. Required means the key MUST be present and valid. Readers ignore unknown top-level and `action` fields (forward compatibility), writers that rewrite a file MUST keep unknown top-level fields. A `version` other than the number `1` makes the file Invalid.

| Field | Type | Required | Rule |
| --- | --- | --- | --- |
| `version` | number | yes | exactly `1` |
| `id` | string | yes | 32 lowercase hex, equals file name stem |
| `name` | string | yes | 1 to 80 characters, not blank, no control characters, no invisible characters |
| `description` | string or null | no | at most 500 characters, no invisible characters |
| `scope` | string | yes | `"global"` or `"project"` |
| `project` | string or null | for `"project"` | absolute canonical project root (see Project identity). Accepted when it starts at a filesystem root (`/…`, on Windows also `C:\…` or `\\server\share\…`), so a POSIX path still reads on Windows. At most 4096 bytes, no control or invisible characters. Must be `null` or absent when `scope` is `"global"` |
| `event` | string | yes | `sessionStart`, `promptSubmit`, `toolEnd`, `turnEnd`, `sessionEnd` |
| `matcher` | string or null | no | `toolEnd` only, see Matcher. `null` or absent means all tools |
| `action` | object | yes | see Actions |
| `enabled` | boolean | yes | |
| `requires` | array of string or null | no | unique, `"harness-memory"` (Harness Memory installed and its memory setting on) or `"harness-dashboard"` (Harness Dashboard installed). A hook whose requirement is not met does not run and shows as waiting |
| `seed` | string or null | no | `"memory-search"`, `"memory-save"`, `"dashboard-limits"` or `"useful-skills"`: marks a Harness family hook, set only by the app window. Other writers keep it as is |
| `createdAt` | string | yes | RFC 3339 UTC, `YYYY-MM-DDTHH:MM:SSZ` (a fraction is accepted on read), e.g. `2026-10-17T10:00:00Z` |
| `createdBy` | string | yes | `"desktop"` or `"agent"`. Set once when the file is created and never changed by any writer. Decides who may remove the hook |
| `updatedAt` | string | yes | same format, set on every write |
| `updatedBy` | string | yes | `"desktop"` or `"agent"`: the last writer. Drives pending (see Agent rules) |

### Actions

`action.type` and fields:

- `addContext`: `text` required, 1 to 4096 characters.
- `followUp`: `text` required, 1 to 4096 characters.
- `command`: `command` required, 1 to 2048 characters. `timeoutSecs` optional integer 1 to 30, default 10.

Valid event and action combinations (everything else is Invalid):

| Event | addContext | followUp | command |
| --- | --- | --- | --- |
| `sessionStart` | yes | no | yes |
| `promptSubmit` | yes | no | yes |
| `toolEnd` | no | yes | yes |
| `turnEnd` | no | yes | yes |
| `sessionEnd` | no | no | yes |

A `command` hook's stdout is used like the text of the matching action: context on `sessionStart` and `promptSubmit`, a follow-up on `toolEnd` and `turnEnd`, discarded on `sessionEnd`.

### Matcher

Only on `toolEnd`. Grammar: `matcher = name *( "|" name )`, `name = 1*[A-Za-z0-9_.:-]`, at most 200 characters, no spaces, no empty part. A hook matches a tool when the tool name equals one of the names, ASCII case-insensitive. Exact names only, no regex or wildcards. A matcher on any other event is Invalid.

The tool name is the one the harness reports. OpenCode reports the native id of the generation the app detected (`opencode --version`: `1.19.3` is OpenCode 1, `opencode v2.1.2` is OpenCode 2), so the same action is `bash` on one and `shell` on the other. OpenCode 1 (1.19) uses these tool ids:

| Tool | What it does | Touched paths |
| --- | --- | --- |
| `bash` | runs a shell command | none |
| `read`, `glob`, `grep` | read a file, find files, search file contents | none |
| `edit`, `write` | change or write the file in `filePath` | that file |
| `apply_patch` | applies a patch | the files of its `*** Add File:`, `*** Update File:`, `*** Delete File:` and `*** Move to:` headers |
| `task` | runs a subagent | none |
| `webfetch`, `websearch` | fetch a URL, search the web | none |
| `todowrite`, `skill`, `question` | update the todo list, load a skill, ask the user | none |
| `<server>_<tool>` | tool `<tool>` of MCP server `<server>` | none |

So `edit|write|apply_patch` matches every OpenCode 1 file change. Only paths inside the project count. Tool calls of a subagent (a `task`) are not reported, as in OMP: they fire no `toolEnd` and add no touched paths.

OpenCode 2 (2.1) renamed some tools and the argument that names the file:

| Tool | What it does | Touched paths |
| --- | --- | --- |
| `shell` | runs a shell command | none |
| `read`, `glob`, `grep` | read a file, find files, search file contents | none |
| `edit` | replaces text in the file in `path` | that file |
| `write` | writes the file in `path` | that file |
| `patch` | applies a patch | the files of the `*** Add File:`, `*** Update File:`, `*** Delete File:` and `*** Move to:` headers of its `patchText` |
| `subagent` | runs a subagent | none |
| `webfetch`, `websearch` | fetch a URL, search the web | none |
| `todowrite`, `skill`, `question` | update the todo list, load a skill, ask the user | none |
| `<server>_<tool>` | tool `<tool>` of MCP server `<server>` | none |

A relative path resolves against the working directory of the OpenCode session. One hook can match the file changes of both generations by naming every id: `edit|write|apply_patch|patch`. Likewise `bash|shell` for the shell tool and `task|subagent` for subagents. The same rules apply as on OpenCode 1: only paths inside the project count, and tool calls made inside a subagent are not reported. A session that OpenCode 2 migrated from OpenCode 1 may still hold `apply_patch` calls and a `filePath` argument, and both count as file changes.

## Events

- `sessionStart`: the first eligible prompt of a new desktop chat (fresh or after a new session, including `/new`). Eligible means a user prompt that is not a steer or follow-up and does not start with `/` or `!`. When its context does not fit in the prompt (see Prompt text), it fires again on the next eligible prompt. A `sessionStart` whose hooks produced nothing (a command without output) does not.
- `promptSubmit`: every eligible prompt.
- `toolEnd`: after a tool call of a user run ends and matches.
- `turnEnd`: when the agent settles after a user run (not after every model turn, and not after the run of a hook's own follow-up). Its commands run on every such settle: a run without a tool call, an aborted one or a failed one included. Their output (and `followUp` text) is sent back only under the follow-up loop rule.
- `sessionEnd`: when the chat exits or is stopped. Commands only. When the app quits it waits at most 3 s for these commands (within its shutdown budget), then stops the ones still running. A page reload does not end the app, so they finish.

A user run starts with a user message and ends at the next settle the agent reports. A message sent after the agent reported a settle belongs to the next run, even when the app is still busy with commands of the run before.

Follow-up loop rule: `followUp` text (and command stdout on `toolEnd` and `turnEnd`) is sent after the agent settles, and only when a user message was delivered since the last settle, at least one tool call ended without error, the run was not aborted and the prompt did not fail. All matching follow-ups of one run are joined into one prompt, so a user run gets at most one hook follow-up. Each `toolEnd` hook contributes at most once per run. Prompts sent by hooks never count as user messages. A user message sent before the follow-up goes out cancels it.

## Prompt text

Context for `sessionStart` and `promptSubmit` is put before the user text:

```
<harness-hook-context>
<hook name="NAME">TEXT</hook>
</harness-hook-context>

USER TEXT
```

One `<hook>` element per contributing hook, in hook order, inside one wrapper. A follow-up prompt is the elements joined with a blank line, so it always starts with `<` and a harness never runs it as a `/` command or `!` shell line, whatever the hook's text starts with:

```
<harness-hook-followup name="NAME">TEXT</harness-hook-followup>
```

Escaping: in `NAME` replace `&` `<` `>` `"` with `&amp;` `&lt;` `&gt;` `&quot;`. In `TEXT` write a `<` that begins `<hook`, `</hook`, `<harness-hook` or `</harness-hook` (ASCII case-insensitive) as `&lt;`. The desktop UI recognises these markers in transcripts (context chip, "Sent by hook").

Cap: each composed context block (the `<harness-hook-context>` wrapper's elements) and each composed follow-up prompt is at most 8 KiB (8192 bytes) of elements. Elements are taken in hook order, one that does not fit is dropped and logged, later smaller ones may still fit. When the user's text with the block would exceed the prompt limit (512 KiB), the last elements are dropped until it fits, and the text is sent alone when none fits.

## Commands

Run by the desktop only, after approval. Shell: `/bin/sh -c <command>` on Linux and macOS, `cmd.exe /D /S /C <command>` on Windows. Working directory: the project root for project hooks, the chat's folder for global ones. Stdin is closed. Exit code non-zero, a timeout (the process is killed) or a spawn failure means no output. Stdout is capped at 16 KiB (truncated), stderr is dropped, and stdout is redacted for secrets before use.

Environment (always set, empty when not applicable):

| Variable | Value |
| --- | --- |
| `HARNESS_HOOK_EVENT` | event name, e.g. `toolEnd` |
| `HARNESS_HOOK_PROJECT` | project root of the chat, empty without one or when it is not a plain path (the `HARNESS_HOOK_TOUCHED` rule below) |
| `HARNESS_HOOK_SESSION` | desktop session id |
| `HARNESS_HOOK_TOOL` | tool name, `toolEnd` only. Empty unless it is 1 to 200 characters of `[A-Za-z0-9_.:-]` |
| `HARNESS_HOOK_TOOL_ERROR` | `1` or `0`, `toolEnd` only |
| `HARNESS_HOOK_TOUCHED` | newline-separated paths the tool call (`toolEnd`), the run (`turnEnd`) or the chat (`sessionEnd`, the latest 256) changed, at most 16 KiB. A path with a control character (a newline included), one of `& \| < > ^ % ! " ( )`, a character Windows may turn into one of them, `/` or `\` when it converts the environment to a legacy code page ("best fit", see below), or a leading `-` or character best fit may turn into `-` (U+00AD, U+02D7, U+2010 to U+2015, U+2043, U+207B, U+208B, U+2212, U+2500, U+FE58, U+FE63, U+FF0D) is left out, and so is a path longer than 16 KiB on its own |

The best-fit characters refused anywhere in a path, on every platform: U+FE10 to U+FE1F, U+FE30 to U+FE6F, U+FF00 to U+FFEF (vertical, compatibility, small, fullwidth and halfwidth forms), and `"` U+02BA, U+02DD, U+02EE, U+030B, U+030E, U+05F4, U+201F, U+2033, U+2036, U+3003, U+301D to U+301F; `|` U+00A6, U+01C0, U+05C0, U+2016, U+2223, U+2225, U+23D0, U+2502, U+2758; `<` `>` U+00AB, U+00BB, U+02C2, U+02C3, U+2039, U+203A, U+2190, U+2192, U+2329, U+232A, U+276E, U+276F, U+27E8, U+27E9, U+3008, U+3009; `^` U+02C4, U+02C6, U+0302, U+2038, U+2191, U+2303; `%` `!` U+066A, U+2052, U+00A1, U+01C3, U+203C; `(` `)` U+207D, U+207E, U+208D, U+208E, U+2768 to U+276B, U+2E28, U+2E29; `/` `\` U+00A5, U+20A9, U+2044, U+2215, U+2216, U+2571, U+2572, U+27CB, U+27CD, U+29F5, U+29F8, U+29F9. The list follows Microsoft's best-fit tables (`bestfit*.txt`, among them code pages 1252, 932 and 936) as far as they could be checked, widened with look-alikes of the same characters. It is not exhaustive for every code page.

The agent reports the tool name and the touched paths, and may have named the project folder, so `HARNESS_HOOK_TOOL`, `HARNESS_HOOK_TOUCHED` and `HARNESS_HOOK_PROJECT` are untrusted input even after the filtering above (a path may hold spaces, quotes, `$` or `;`). A command MUST NOT paste them into its own command line unquoted. On Linux and macOS quote them and end the options first, for example `git diff -- "$HARNESS_HOOK_TOUCHED"` or `printf '%s\n' "$HARNESS_HOOK_TOUCHED" | while IFS= read -r path; do …; done`. On Windows `cmd.exe` expands `%HARNESS_HOOK_TOUCHED%` before it parses the line, so read the variables inside a script (PowerShell `$env:HARNESS_HOOK_TOUCHED`) rather than on the command line.

Budget: command hooks of one event firing run sequentially in hook order and share a total budget of 30 s. Once the time spent reaches 30 s, the remaining command hooks of that firing are skipped and logged. Each command also keeps its own `timeoutSecs`, cut short to the budget left.

## Project identity

For a folder: take its real path (symbolic links resolved, on Windows without a `\\?\` prefix), then the nearest ancestor-or-self that contains `.git` (a directory or a file), else the folder itself. The result is the project root. Both modes use this algorithm. `project` in a file holds it as a string with the OS separators, compared by exact equality. The MCP server resolves the root from the folder it is given and MUST reject a `project` that does not exist or is not a folder.

## Approvals

Written only by the app window, so a file an agent wrote never runs unreviewed.

Location `<app_config_dir>/hook-approvals.json`, where `<app_config_dir>` is Tauri's `app_config_dir` for the identifier `com.mspiechowicz.harness-useful-desktop`:

| OS | `<app_config_dir>` |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/com.mspiechowicz.harness-useful-desktop`, else `~/.config/com.mspiechowicz.harness-useful-desktop` |
| macOS | `~/Library/Application Support/com.mspiechowicz.harness-useful-desktop` |
| Windows | `%APPDATA%\com.mspiechowicz.harness-useful-desktop` (roaming, not `%LOCALAPPDATA%`) |

Content:

```json
{ "version": 1, "approvals": { "<id>": "<sha256 hex of the raw file bytes>" } }
```

- The hash is lowercase SHA-256 of the exact bytes of `<id>.json`. Any byte change, including a rewrite by an agent with the same values, makes the hook pending.
- A hook runs only when it is valid, `enabled`, its hash equals `approvals[id]` and its `requires` are met.
- The desktop saves a hook file and its approval together, so hooks created or edited in the desktop run at once. The user approving a pending hook stores the hash of the current bytes. Deleting a hook removes its approval.
- The MCP server (same program, same `<app_config_dir>`, no override) reads the file read-only to report a hook as approved (hash equals the entry) or pending (no entry or a different hash). A missing, unreadable or invalid file means every hook is pending. The MCP server MUST NOT write it.
- The app window never saves over an approvals file it could not read: the write fails before anything changes. An invalid file (not JSON, over 4 MiB, not UTF-8, another version) is renamed `hook-approvals.json.corrupt-<unix-seconds>` by the next write, and every hook is pending until approved again. The seed record is handled the same way, except that a rebuilt record lists every seed, so launch seeding creates nothing the user deleted.
- The app window drops the approval of an id only when neither `<root>/hooks` nor `<root>/team` has an entry named `<id>.json`. An entry that is there but cannot be read, or is not a regular file, keeps its approval.
- The app window saves a hook only over a file whose current bytes are approved: a pending file is approved or rejected first. Each write checks again, right before its rename, that the file still holds the bytes it read, and fails with a conflict otherwise.
- Fixture `approvals/valid-approvals.json` holds real hashes of the fixtures beside it, plus one stale hash.

Approved copies `<app_config_dir>/hook-approved/<id>.json`: the exact bytes of the approved version (mode `0600`, written with the approval, removed when the app deletes the hook), so the Hooks view can show what a pending edit changed. Written only by the app window.

Hooks state `<app_config_dir>/hooks-state.json`: `{ "version": 1, "seeded": ["memory-search", "memory-save", "dashboard-limits", "useful-skills"], "teamDisabled": ["<id>"], "sharedFrom": { "<id>": "<sha256>" } }`. `seeded`: the seeds launch seeding created (or found) once. `teamDisabled` and `sharedFrom` (both optional, see Team hooks): the team hooks turned off on this machine, and the SHA-256 each local hook had when the user shared it. Written only by the app window. An invalid file is set aside and rebuilt with every seed listed and no team entries.

## Harness family hooks

A catalog of hooks for the Harness family members agents use over MCP. Each is global, `requires` its member, has `createdBy: "desktop"` and `updatedBy: "desktop"` and is approved when the app writes it.

| Seed | Member (`requires`) | Name | Event | Action |
| --- | --- | --- | --- | --- |
| `memory-search` | `harness-memory` | `Search Harness Memory first` | `sessionStart` | `addContext` |
| `memory-save` | `harness-memory` | `Save what you learned` | `turnEnd` | `followUp` |
| `dashboard-limits` | `harness-dashboard` | `Check limits before long work` | `sessionStart` | `addContext` |
| `useful-skills` | `harness-memory` | `Check Useful Skills first` | `sessionStart` | `addContext` |

Exact texts:

- `memory-search`: `Before you start, search Harness Memory for notes about this task (note_search) and check the saved scripts (script_list). Reuse what fits.`
- `memory-save`: `If this turn taught you something worth keeping, save it to Harness Memory (note_save or note_update). Save any command sequence you would run again as a script (script_save). Reply briefly, or say nothing needs saving.`
- `dashboard-limits`: `Before a long or expensive task, check the plan limits and budgets with Harness Dashboard (limits, budget_status). Say so if a limit is close.`
- `useful-skills`: `If Useful Skills are installed, read the ones that apply before you act: decide the workflow first (plan, implement, review), then read any other skill that fits the task and follow its limits (for example the 500-line file maximum). Do not skip a matching skill.`

When the app writes them:

- Launch seeding: at launch (and after a member is enabled), the hooks of each available member are created once and listed in `hooks-state.json`. A listed seed is never created at launch again, even after the user deletes it. A file with that seed already in the folder is listed, not duplicated.
- Harness family hooks dialog: the user adds a missing hook or updates an outdated one (an approved valid file with that seed whose name, event or action differs from the catalog, including the user's own edit). An update keeps the id, `createdAt`, `createdBy`, scope and `enabled`.
- Automatic updates (Settings, Updates, "automatic extension updates" on): at launch the app updates every outdated family hook that is stale, one whose name and event match the catalog and whose text is word for word an earlier catalog version of that seed, and tells the user in a toast. A hook the user edited is never stale and is updated only from the dialog.
- Only an approved file counts as a seed's hook. When the app adds a seed that has only a pending file (an edit the user has not reviewed, or a file someone else wrote), it rewrites that file with the catalog version and approves it, keeping the id, `createdAt`, `createdBy`, scope and `enabled`.
- After a member is installed through the app, its missing hooks are added again, even ones deleted before. Existing ones are never overwritten.
- Uninstalling a member deletes nothing: its hooks wait for it.

Status of a catalog entry: `installed` (an approved valid file with the seed matches the catalog), `outdated` (it differs), `missing` (no approved valid file with the seed), `needsMember` (the member is not installed).

## Team hooks

A team that points Harness Memory at a team database (PostgreSQL, libSQL or MongoDB) can share hooks. The desktop never connects to that database: it runs `harness-memory hooks …` commands, and Harness Memory stores the hooks in its `hooks` table while its config says `hooks = "remote"`. Only the user shares from the app, in the Hooks view: neither `harness-desktop mcp` nor Harness Memory has an MCP tool that publishes a hook, and agents never see team hooks through the app. That is not a boundary: any process running as the user, an agent with a shell included, can run `harness-memory hooks put` directly, and so can anyone with write access to the team database. Trusting team hooks means trusting everyone and everything that can write to that database. Leave "Trust team hooks" off unless that holds, and approve each team hook after reading it.

Body. The team database stores a hook file (version 1, at most 16 KiB) with two more rules:

- `seed` absent or `null`: Harness family hooks are never shared, every machine seeds its own.
- A project hook's `project` is the repository key `git:host/owner/repo` (Harness Memory's identity of the project root, from its `origin` remote), never a local path. A project in a repository without an `origin` remote (a `path:` identity) cannot be shared. A global hook's `project` is `null` and its row has no repository.

Sharing a local hook (approved, valid, not a family hook) sends its exact bytes, with only `project` replaced by the repository key for a project hook. The team copy keeps the local hook's `id`. The SHA-256 of the stored body is the hash a machine approves.

Copies. `<root>/team/` (mode `0700`) holds one envelope per team hook, `<id>.json` (mode `0600`, written by a temp file and a rename), as Harness Memory listed it:

```json
{ "version": 1, "id": "<id>", "repo": "git:github.com/o/r", "sha256": "<sha256 of body>", "author": "ana", "createdAt": "2026-10-01T10:00:00Z", "updatedAt": "2026-10-02T08:30:00Z", "body": "<the hook file>" }
```

`<root>/team/projects.json` maps each project root a list was asked for to its identity: `{ "<project root>": { "key": "git:…" | "path:…", "shareable": true } }`. A chat in that root gets the team hooks whose `repo` is that key, run in the root like its own project hooks. So a team project hook applies in every folder whose `origin` resolves to the key (every clone or worktree of the repository), and one approval covers all of them.

Refresh. While sharing is on, the app window lists the team hooks (`hooks list --json`, all pages: it follows `nextOffset`, the offset to ask for next with skipped rows counted, and an older answer without it advances by the page size; a list longer than the pages the app reads, or a page it cannot read, makes the round incomplete) for the global scope and for each project root in use (the 32 most recent), at launch, every 60 seconds, when the Hooks view asks, and after the user shares or removes one. It writes what was listed and removes copies no list returned, scope by scope: global copies when the global list read every page, copies of repository key `R` when the list of some root with key `R` read every page. A failing list keeps the copies of its own scope only. A root that is no longer a folder (an unmounted drive) is skipped and stays in `projects.json`, so its repository's copies and their approvals stay until it is back. Copies of a repository key no root in `projects.json` has any more are removed. The filesystem root and the home folder are never asked for, and a root Harness Memory refuses as a project is forgotten. A root's mapping in `projects.json` is used (its team project hooks shown and run) only after a round of this session listed it. A copy that leaves the folder takes its approval and approved copy with it, unless a local hook has the same id. Chats never wait for a refresh: they use the copies on disk. Sharing turned off, a Harness Memory without team hooks, or no Harness Memory deletes `<root>/team/` at once, so team hooks stop.

Origin and status. The Hooks view lists team hooks after the local ones, with `origin: "team"`, the sharer (`author`) and when the team copy changed. A team hook is, in this order:

- Invalid: the envelope cannot be read, its `sha256` is not the hash of `body`, the body breaks the rules above, or its `repo` does not match the body's `project`.
- Duplicate: `<root>/hooks/` has a file with the same id. The local hook wins and the team copy never runs (on the sharer's machine the team copy of a shared hook is always a duplicate).
- Pending: "Trust team hooks" (`trust_team_hooks = true` in Harness Memory's config) is off and the exact body bytes are not approved. Approval uses `hook-approvals.json` and `hook-approved/` as for local hooks, so a changed team hook shows what changed since the version this machine approved.
- Off: the body says `enabled: false`, or the user turned it off on this machine (`teamDisabled` in `hooks-state.json`, never written to the team).
- Paused: a requirement is not met.
- Active: it runs.

Trust rule. A team hook runs only while sharing is on, and only with "Trust team hooks" on or after the user approved its exact bytes on this machine.

Run order. Local global hooks, local project hooks, then team global hooks, then team project hooks, each group by `createdAt` then `id`.

Update and copy. After a share, `sharedFrom[id]` holds the hash of the local file shared; when the local file changes, the team copy offers "Update team copy", which stores the new body over the team copy it names (`--expect-sha`). "Copy to my hooks" (only for a team hook that may run: trusted or approved, never a pending one) writes the team hook as a local, approved hook with the same id (a project hook gets the local project root as `project`), which then wins over the team copy. "Delete all hooks" never touches team hooks.

## Agent rules (MCP server `harness-desktop mcp`)

- A new hook gets `createdBy: "agent"` and `updatedBy: "agent"`.
- Every edit the MCP server writes on behalf of an agent (any hook, including a user's) sets `updatedBy: "agent"` and a new `updatedAt`. Its bytes change, so it is pending until the user approves that exact version in the app.
- `createdBy` never changes. The MCP server keeps it on edits and rejects an edit that would change it. Hooks created in the app (user hooks, Harness family hooks) have `createdBy: "desktop"`.
- The MCP server removes a hook only when the file has `createdBy: "agent"`. Otherwise it refuses and tells the agent to ask the user. An agent edit of a user hook does not make it removable by the agent.
- The MCP server keeps the `scope` and `project` of a hook with `createdBy: "desktop"`: an edit that would change them is refused (ask the user). An agent may move a hook an agent created, which stays pending.
- The MCP server shows a hook's `text` and `command` to the agent with secrets masked (the same redaction as command output). The file keeps what was written.
- Edits and removals carry the lowercase SHA-256 of the file bytes the agent last read (as `hook_get` returned it). The server refuses when the current bytes hash differently.
- The MCP server keeps `createdAt`, `createdBy`, `seed`, `requires` and unknown fields on edits and validates everything above before writing.
- `hook_get`, `hook_update` and `hook_remove` see only the global hooks and those of the project the server works in (its working folder): for another project's hook they answer "no hook has this id".

## Validity summary

Invalid files are listed with a reason and never run. Reasons include: not JSON, over 16 KiB, wrong `version`, id mismatch, a missing or mistyped field, a limit exceeded, a bad event and action combination, a bad matcher, a scope and project mismatch, over the 64 limit. A fixture file is checked by content only, its name does not follow `<id>.json`. Loader tests copy a fixture to `<id>.json` first.

Fixtures: every `valid-*.json` MUST be accepted and every `invalid-*.json` MUST be rejected. The `approvals/` subfolder is not matched by a non-recursive glob of the folder.
