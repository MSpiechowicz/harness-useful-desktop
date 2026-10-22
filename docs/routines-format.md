# Routine file format (version 1)

The contract for routine files in Harness Desktop (Rust). One program reads and writes them in two modes: the app window (user, runs routines) and its MCP server `harness-desktop mcp` (agents, stdio). Shared fixtures: `business/tests/fixtures/routines/`, copied to Harness Memory's `tests/fixtures/routines/`.

A routine is a small JSON file that says when something runs and what runs: an agent chat with a prompt, a shell command or a saved Harness Memory script. The app window reads the files, shows them in the Routines view, schedules them and runs them. The MCP server only reads and writes files on behalf of an agent, it never runs a routine.

Key words: MUST, MUST NOT, MAY as in RFC 2119. Lengths count Unicode code points, sizes count bytes. Invisible characters are the set defined in `docs/hooks-format.md` (the intro: Unicode general category `Cf`, the line and paragraph separators, the fillers, the variation selectors and the other listed code points), the same set Share setup and Harness Memory reject. Control characters are Unicode category `Cc`.

## Folders

Routines root (`<root>`): `HARNESS_ROUTINES_DIR` when set to an absolute path (a leading `~` expanded, a relative value ignored), else a platform base followed by `harness-routines`:

| OS | Base |
| --- | --- |
| Linux | `$XDG_DATA_HOME` when absolute, else `~/.local/share` |
| macOS | `~/Library/Application Support` |
| Windows | `%LOCALAPPDATA%` when absolute. Unset: no routines folder, routines are unavailable |

Linux example: `~/.local/share/harness-routines`. This is `shared_data_dir` in `business/src/app_dirs.rs` with `harness-routines`, the same rule as the hooks root with another folder name.

Routine files live in `<root>/routines/`. Either app creates `<root>` and `<root>/routines` when missing, mode `0700` on Unix. Two more folders belong to the app window alone, and the MCP server never reads them:

- `<root>/team/` holds the copies of team routines (see Team routines).
- `<root>/workspace/` (mode `0700`) holds the working folders of global routines that run an agent or a command: they have no project. Each such routine runs in its own folder `<root>/workspace/<id>` (mode `0700`), so two routines (a local and a team one included) never share a working folder. Before each run the app window creates the folders when missing and checks them: each MUST be a real folder, not a symbolic link, whose canonical path is the canonical `<root>` followed by `workspace/<id>`, and on Unix owned by the user and not writable by group or others (`<root>` too), otherwise the run fails.

File rules (the same as hooks, with `routines` for `hooks`):

- Name is `<id>.json`, `<id>` is 32 lowercase hex characters (`^[0-9a-f]{32}$`). The `id` field inside MUST equal the name stem, otherwise the file is invalid.
- Written mode `0600` on Unix. UTF-8, no BOM, JSON, two-space indent, trailing newline (readers accept any JSON formatting).
- Written by a temp file in the same folder (mode `0600`), flushed, then renamed over the target. Temp names start with `.` and end with `.tmp` or contain `.tmp-` (the desktop uses `.<name>.tmp-<pid>-<nanos>`). Readers MUST ignore every name that is not exactly `<id>.json`.
- Readers ignore anything that is not a regular file (symbolic links included). On Unix, readers ignore the whole folder while `<root>` or `<root>/routines` is owned by another user or writable by group or others, and a file another user owns is never parsed (it is listed as Invalid, "the file could not be read"). The app window applies the same owner and mode checks to `<root>/team` and its copies. A `<root>` the user owns that group or others can write to (an older version or a loose umask created it so) has its group and other write bits cleared by either app before it is checked (its other bits stay), and the change is logged once per folder. A `<root>` that is a symbolic link is never changed: the folder it points to is checked as it is.
- A file larger than 16 KiB (16384 bytes, raw) is Invalid and is not parsed.
- Writers MUST NOT follow a symbolic link at the target path.
- Per scope at most 64 routines (global, and each project separately). The MCP server refuses to add a routine to a scope that holds 64 valid routines, the app window to a scope that holds 64 approved ones. When a folder holds more, approved routines come first, each group ordered by `createdAt` then `id` ascending: the first 64 of a scope are used and the rest are Invalid ("over the limit"). A routine over the limit is still listed, and deleted by "Delete all routines", only with its own scope, while a file that is not a valid routine is listed with every scope.

Per-machine files of the app window (see Approvals and Runs): in `<app_config_dir>` `routine-approvals.json`, `routine-approved/` and `routines-state.json`, in `<app_local_data_dir>` `routine-schedule.json`, `routine-runs/` and `routine-sessions.json`. `<app_local_data_dir>` is Tauri's `app_local_data_dir` for the identifier `com.mspiechowicz.harness-useful-desktop` (`local_data_dir` in `business/src/app_dirs.rs`): `$XDG_DATA_HOME` (else `~/.local/share`) on Linux, `~/Library/Application Support` on macOS and `%LOCALAPPDATA%` on Windows, each followed by the identifier.

## Routine object

Top-level fields. Required means the key MUST be present and valid. Readers ignore unknown top-level, `when` and `action` fields (forward compatibility), writers that rewrite a file MUST keep unknown top-level fields. A `version` other than the number `1` makes the file Invalid.

| Field | Type | Required | Rule |
| --- | --- | --- | --- |
| `version` | number | yes | exactly `1` |
| `id` | string | yes | 32 lowercase hex, equals file name stem |
| `name` | string | yes | 1 to 80 characters, not blank, no control characters, no invisible characters |
| `description` | string or null | no | at most 500 characters, no invisible characters |
| `scope` | string | yes | `"global"` or `"project"` |
| `project` | string or null | for `"project"` | absolute canonical project root (see Project identity). Accepted when it starts at a filesystem root (`/…`, on Windows also `C:\…` or `\\server\share\…`), so a POSIX path still reads on Windows. At most 4096 bytes, no control or invisible characters. Must be `null` or absent when `scope` is `"global"` |
| `when` | object | yes | see When |
| `action` | object | yes | see Actions |
| `enabled` | boolean | yes | whether the schedule is on (see Approvals for when a routine runs) |
| `createdAt` | string | yes | RFC 3339 UTC, `YYYY-MM-DDTHH:MM:SSZ` (a fraction is accepted on read), e.g. `2026-10-17T10:00:00Z` |
| `createdBy` | string | yes | `"desktop"` or `"agent"`. Set once when the file is created and never changed by any writer. Decides who may remove the routine |
| `updatedAt` | string | yes | same format, set on every write |
| `updatedBy` | string | yes | `"desktop"` or `"agent"`: the last writer. Drives pending (see Agent rules) |

`seed` and `requires` are reserved: routines have no Harness family catalogue and no requirements, so a file where either holds a value other than `null` is Invalid. A writer MUST NOT write them.

## When

`when.type` and fields. A routine has exactly one `when`.

| `when.type` | Fields | Fires |
| --- | --- | --- |
| `daily` | `at` required | every day at `at` |
| `weekdays` | `at` required | Monday to Friday at `at` |
| `everyMinutes` | `every` required, whole number 1 to 720 | every `every` minutes, counted from the start of each day |
| `everyHours` | `every` required, whole number 1 to 24 | every `every` hours, counted from the start of each day |
| `cron` | `expression` required, at most 100 characters | at every minute the expression matches |
| `appStart` | none | once each time Harness Desktop starts |

`at` is `HH:MM` on a 24-hour clock, exactly two digits each (`^([01][0-9]|2[0-3]):[0-5][0-9]$`). A value such as `9:00`, `24:00` or `09:00:00` is Invalid. A value of the wrong JSON type (`every` as a string or a fraction) is Invalid.

### Time zone

The file holds no time zone. Every time is wall-clock time in the time zone of the machine, read from the operating system again on every scheduler tick, so travelling or changing the zone moves the routine with the clock. A team routine fires at each machine's own local time.

### Slots

A slot is one instant a routine is due. Equal instants count once.

- `daily`, `weekdays`: the local time `at` of each (matching) local calendar day.
- `everyMinutes`, `everyHours`: slots are `k * every` minutes (hours) of elapsed time after the first instant of the local day, for `k = 0, 1, 2, ...`, while before the first instant of the next local day. The count restarts every day, so `everyMinutes: 7` fires at 00:00, 00:07, ... 23:55 and again at 00:00, and `everyHours: 5` at 00:00, 05:00, 10:00, 15:00, 20:00 and 00:00 (4 hours later). `everyHours: 24` fires at 00:00 only. Elapsed time is used, so a day of 23 or 25 hours has no effect on spacing.
- `cron`: every local minute that matches (see Cron).
- `appStart`: no slots. It runs once at launch, after the routines were loaded and only for a routine that is runnable then (see Approvals). A routine that becomes runnable later in the session does not run for it. A team `appStart` routine runs at the first look that knows whether team routines are shared: when Harness Memory could not be asked at launch, it runs once that answer comes later in the same session.

### Daylight saving time

A local time that does not exist (inside a spring-forward gap) runs once, immediately after the gap, shifted by the length of the gap: `02:30` in a gap from 02:00 to 03:00 runs at 03:30. A local time that happens twice (fall back) runs once, at its first occurrence. For `cron`, every matching minute is resolved the same way and the resulting instants are de-duplicated, so a pattern such as `*/15` never fires twice for the same instant.

### Cron

Five fields separated by exactly one space (no leading or trailing space, no tab): minute, hour, day of month, month, day of week. A field is one or more items separated by `,`. An item is a base, optionally followed by `/` and a step:

```
item  = base [ "/" step ]
base  = "*" / value / value "-" value
step  = 1*2DIGIT            ; 1 up to the field's highest value, only after "*" or a range
value = 1*2DIGIT / name     ; names only in month and day of week
```

| Field | Values | Names |
| --- | --- | --- |
| minute | 0 to 59 | |
| hour | 0 to 23 | |
| day of month | 1 to 31 | |
| month | 1 to 12 | `jan` to `dec` |
| day of week | 0 to 7 (0 and 7 are Sunday) | `sun` to `sat` |

Rules:

- Names are three letters, ASCII case-insensitive, and may form a range (`mon-fri`, `jan-jun`). A range runs from a start to an end that is not smaller (`30-10`, `fri-mon` are Invalid, write `fri-sun,mon`). In day of week, `sun` or `0` at the end of a range that starts after Sunday counts as 7, so `fri-sun` runs Friday to Sunday and `sun-sun` is Sunday only. Numbers are decimal, up to two digits, no sign.
- A step after a single value (`5/10`) is Invalid, so is a step of 0, an empty item or field, `?`, `L`, `W`, `#`, seconds, years and macros such as `@daily`.
- Day of month and day of week: when both fields begin with something other than `*`, a day matches when either field matches (OR). When either field begins with `*` (`*/2` counts), a day must match both (AND). Month, hour and minute always combine with AND.
- A cron that can never match is Invalid: the expression is checked against the calendar, not against today, so it does not change with the date. When day of month is the only restricted day field, at least one of its days must exist in one of the months (February counts 29). `0 0 31 2 *` and `0 0 31 4,6,9,11 *` are Invalid, `0 0 29 2 *` is valid and fires only in leap years.

## Actions

`action.type` and fields:

- `agent`: starts a chat with an agent and sends the prompt.
  - `harness` required, the id of a registered harness (`"omp"`, `"pi"`, `"opencode"` or `"claude"`, the values of `HarnessId::ALL` in `business/src/model/harness.rs`). An unknown value is Invalid. A harness whose `capabilities.chat` is `false` is Invalid too: the app has no chat to start. The tool of the routine MCP server takes the same ids.
  - `prompt` required, 1 to 8192 characters.
  - `model` optional, `null` or absent means the harness's own default model, else an object `{ "provider": string, "id": string }`, each 1 to 200 characters without control or invisible characters. The same pair the app uses to switch a chat's model (`ModelOption.provider` and `ModelOption.id`, sent as `SetModel { provider, modelId }`). A model the agent does not offer at run time fails the run.
  - `approvalMode` optional, `null` or absent means the harness's own setting (OMP `tools.approvalMode`, OpenCode's permission config, Claude Code's `permissions.defaultMode`), else one of `"alwaysAsk"`, `"write"`, `"yolo"`: the serialized names of `ApprovalMode` (`business/src/model/agent.rs`). The app maps them to OMP's `always-ask`, `write` and `yolo`, and those spellings are Invalid in a file. For OpenCode the app turns them into the chat's permission rules. Only with a harness that has a chat and `capabilities.approval_modes` (`omp`, `opencode` and `claude`): a value with `pi` is Invalid. `"yolo"` also needs `capabilities.yoloMode` (`omp` and `opencode`): Claude Code chats never start in its bypass mode, so `"yolo"` with `claude` is Invalid, while `"alwaysAsk"` and `"write"` start it in its `default` mode (Always ask adds the app's ask-all hook). Without `approvalMode` the run uses Claude Code's own `permissions.defaultMode`, which can be `bypassPermissions` or `auto`: tools then run without asking, and the file shows no "Runs tools without asking" chip, because the chip follows only `approvalMode` and `trustOnce` in the file.
  - `trustOnce` optional boolean, `null` or absent means `false`: trust the project's local resources for this launch only (PI project trust, `AgentStartRequest.trustOnce`). `true` only with a harness that has a chat and `capabilities.project_trust` (`pi` and `claude`), with `omp` or `opencode` it is Invalid. For `claude` it lets the run start in a folder Claude Code has not trusted: without `hasTrustDialogAccepted` for the project in Claude Code's global file (and `trustOnce` false) the start is refused as untrusted and the run fails. The app passes no flag to `claude` and writes nothing, and a routine run never uses the "Trust once" the user gave a chat during this app run, so the file must say it.
  - `maxMinutes` optional whole number 1 to 240, `null` or absent means 30: the run is stopped after that long.
- `command`: `command` required, 1 to 2048 characters. `timeoutSecs` optional whole number 1 to 3600, `null` or absent means 300.
- `script`: runs a script saved in Harness Memory (`script_run`).
  - `name` required, the script name, `^[a-z0-9][a-z0-9_-]{0,63}$` (`is_valid_name` in Harness Memory's `src/store/scripts.rs`).
  - `scope` required, `"repo"` or `"global"`, Harness Memory's script scopes (the same values the MCP tools take). Not the routine's `scope`: `"project"` and `"all"` are Invalid here. `"repo"` needs a routine with `scope: "project"`, a global routine has no repository to look in.

`prompt` and `command` MUST NOT contain invisible characters, or control characters other than tab, line feed and carriage return. (Hooks check this for `name` and `description` only, a routine's prompt and command are what a user approves, so what is shown must be what runs.)

Apart from those two harness rules, every combination of `when` and `action` is valid. An agent action with `approvalMode: "yolo"` or `trustOnce: true` runs tools without asking: the app shows it as "Runs tools without asking" in the editor and the review, for a team routine too. That chip is information only, it does not change when a routine may run. With `claude`, `trustOnce` only lets the run start in an untrusted folder and does not switch its approvals off, but the chip is shown for it as well.

## Runs

A run is one execution of a routine. Runs are started by a schedule, by the user, or by launch, only by the app window.

### Triggers and statuses

`trigger`: `schedule` (a slot, on time), `catchUp` (a missed slot, see Catch-up), `appStart` (`when.type: "appStart"` at launch), `manual` ("Run now").

`status`:

| Status | Meaning |
| --- | --- |
| `running` | started and not finished. A run waiting for an answer is still `running` and shown as "Needs you" |
| `ok` | an agent run settled, a command or script exited with 0 |
| `failed` | non-zero exit, a command timeout, a spawn error, the agent run failed, the time limit (`maxMinutes`) passed, the model is not available, the project folder is missing or is no longer the project, a script was not found, Harness Memory is not available, the session limit stayed full for 10 minutes ("Too many chats open") |
| `needsAttention` | the run cannot go on without the user: an unreviewed Harness Memory script ("Trust it in the Scripts view"), or a question the user did not answer in 10 minutes |
| `stopped` | the user stopped it (or deleted its routine), Harness Desktop closed during the run, or a team routine stopped being allowed to run here (turned off here, gone from the team, sharing off, no longer trusted or approved) |

### How a run starts

A routine can start only when it is valid, approved and, for a scheduled trigger, `enabled` (see Approvals). "Run now" needs a valid, approved routine, `enabled` is not required (it only switches the schedule). A pending routine never runs, not even with "Run now". Agents cannot run routines.

- One run per routine at a time. A slot that is due while a run of that routine is still going, or waiting for a free place, is skipped and logged, and counts as handled. "Run now" is refused while the routine runs.
- Deleting a routine first checks the version the user saw, then stops its run and waits for it to end. Until the delete is over no run of that routine starts.
- Once Harness Desktop starts to close, no run starts. A run that was still starting is stopped as soon as it is recorded.
- At most 2 agent runs and 4 command or script runs at the same time. Further runs wait in the order they were due and are recorded when they start.
- A run uses the routine as it is when the run starts.

### Agent runs

The app starts a chat like the Chat view does, in the project root for a project routine (the root is checked again: it MUST still be a folder whose project identity equals `project`, otherwise the run fails) or in `<root>/workspace/<id>` for a global one. It applies `approvalMode` and `trustOnce` as the chat's per-session override, switches to `model` when set, and sends `prompt` as the first user message, so hooks (`sessionStart`, `promptSubmit`, ... see `hooks-format.md`) fire like in any chat. The run ends when the agent settles and the hooks have finished their follow-ups. The prompt is the text of the file, with no wrapper.

When the session manager has no free place the run retries every 30 seconds for up to 10 minutes, then fails. `maxMinutes` counts from the moment the chat started: the wait for a free place does not count. A start that failed while the app's page reloaded is tried once more.

The session is a normal session of the harness, listed in Sessions with a "Routine" tag (see Session tags). The user can open it from the Routines view while it runs and answer its questions: the window gets the chat so far (the first 3 events, then the newest, at most 20 000 events and 32 MiB of them) and the live events. With no window looking at the run, a question or tool approval of the agent (a UI request) is waited for: after 10 minutes without an answer the request is cancelled, the agent is stopped and the run is `needsAttention`. At the first moment a run waits for an answer the app tells the user (see Notifications).

When the run ends well (the agent settled) and a window is attached, the window keeps the agent until the chat tab is closed. Any other end stops the agent, attached or not: the user stopped the run, the time limit passed, a question went unanswered, the model or the prompt was refused. An agent that exits before the run is done fails the run ("The agent stopped before it finished"), also while a window shows it.

### Command runs

Shell and working directory as for hook commands (`hooks-format.md`, Commands): `/bin/sh -c <command>` on Linux and macOS, `cmd.exe /D /S /C <command>` on Windows, in the project root of a project routine and `<root>/workspace/<id>` of a global one. Stdin is closed. Exit code non-zero, a timeout (the process is killed) or a spawn failure is `failed`. A timeout, "Stop", deleting the routine and quitting kill the command's whole process group on Linux and macOS and its process tree on Windows (`taskkill /T`), so what it started in the background ends too. On Windows a process that was started by one that already exited is not part of the tree any more and keeps running. Of stdout and of stderr the last 16 KiB are kept: when a stream was longer, its first word in what is kept (up to the first whitespace) is dropped too, since it was cut off and a secret cut in half could not be recognized. All output is redacted for secrets before it is kept.

Environment (always set, empty when not applicable):

| Variable | Value |
| --- | --- |
| `HARNESS_ROUTINE_ID` | the routine's id |
| `HARNESS_ROUTINE_RUN` | the run id |
| `HARNESS_ROUTINE_TRIGGER` | `schedule`, `catchUp`, `appStart` or `manual` |
| `HARNESS_ROUTINE_PROJECT` | the project root, empty for a global routine |

### Script runs

The app asks Harness Memory (`script_get`) whether this machine may run the script. When it says the user must approve it first, the run is `needsAttention` without running ("Trust it in the Scripts view"). Otherwise it calls `script_run` with no arguments and no stdin, with `project_path` set to the routine's project root for scope `repo`, over its own connection to `harness-memory mcp`, so a long script never blocks other use of Harness Memory. The run's time limit is the script's own timeout plus 30 seconds. A script that is missing, or no Harness Memory with scripts, is `failed`. If the script changes between the two calls, Harness Memory asks the user the normal way. Stopping a script run (the user, a delete, quitting) cancels the request (`notifications/cancelled`) and closes that server's input: Harness Memory then kills the script with everything it started (its own process group) and removes the script file. A server still running 2 seconds later is killed. A Harness Memory from before routines does not stop a script that way: the script then runs until it ends by itself or reaches its timeout.

### Run records

One file per routine, `<app_local_data_dir>/routine-runs/<routineId>.json` (mode `0600`), written by the app window alone, deleted with the routine:

```json
{ "version": 1, "routineId": "<id>", "runs": [ { "id": "<32 hex>", "routineName": "Morning review", "origin": "local", "trigger": "schedule", "startedAt": "2026-10-18T09:00:00Z", "endedAt": "2026-10-18T09:03:12Z", "durationMs": 192000, "status": "ok", "message": null, "excerpt": "...", "errorExcerpt": null, "exitCode": null, "session": { "harness": "omp", "id": "..." }, "project": null } ] }
```

- Newest first, at most 100 per routine, older ones are dropped when a run is added. A run is written when it starts (`running`), rewritten when it ends. Only a start creates the file: a run that ends after its routine's records were deleted writes nothing.
- The records of a team routine are deleted when its team copy leaves `<root>/team/` (removed from the team, or sharing off) and no local routine has its id.
- `origin`: `"local"` or `"team"`. `routineName`: the name when the run started (at most 80 characters, no invisible characters). `message`: a short secret-free reason or null, at most 500 characters. `exitCode`: the exit code of a command or script, else null. `session`: the harness session of an agent run, else null. `project`: the project root the run used, null for a global run.
- `excerpt` (at most 4 KiB, UTF-8 cut on a boundary): the last assistant text of an agent run, or the stdout tail of a command or script. `errorExcerpt` (at most 4 KiB): the stderr tail of a command or script, null for an agent. Both are redacted for secrets.
- A prompt or command is never copied into a record.
- At launch a leftover `running` record is rewritten as `stopped` with the message "Harness Desktop closed during the run". When the app quits, live runs are marked `stopped` the same way. An unreadable or invalid file is set aside as `<routineId>.json.corrupt-<unix-seconds>` and the history starts again.

### Session tags

`<app_local_data_dir>/routine-sessions.json`: `{ "version": 1, "sessions": { "<harness>:<sessionId>": { "routineId": "<id>", "runId": "<id>", "name": "Morning review" } } }`, the newest 2000 entries, written when an agent run's session id is known. Sessions shows these sessions with the tag "Routine" and the routine's name. Sessions of global routines live in their routines' folders `<root>/workspace/<id>` (`<root>/workspace/team-<id>` for a team routine) and are listed together under "Global routines" (the app names `<root>/workspace` for all of them, and opens or resumes each session in the folder that holds it). Only the folders of routines still there count: when a routine is deleted, or a team routine leaves this machine, its folder is deleted (never through a symbolic link, and not while a run of it goes on or waits), its sessions are no longer listed or opened there, and the session files themselves stay in the harness's own session store.

### Notifications

A native notification, or an in-app toast instead when the Harness Desktop window is in front. The body starts with the routine's name (valid by construction: no control or invisible characters):

| Event | Title | Body |
| --- | --- | --- |
| a run waits for an answer (see Agent runs) | Routine needs you | `<name> · Answer it in Harness Desktop` |
| status `failed` | Routine failed | `<name> · See the run in Harness Desktop` |
| status `needsAttention` | Routine needs attention | `<name> · See the run in Harness Desktop` |
| status `ok` | Routine finished | `<name>` |

"Needs you" and failures are always shown, "Routine finished" at most once per routine and hour (the hour counts from an OS notification that was shown: a toast while the window was in front does not use it up). The titles and texts are localised in the five app languages. Clicking any notification focuses the window. Clicking a "needs you" notification also opens the chat of the run it was about, while that run still waits (the notification carries its run id). A platform that reports the click without the notification's data opens nothing.

### Catch-up and the schedule file

`<app_local_data_dir>/routine-schedule.json` (written by the app window, mode `0600`): `{ "version": 1, "routines": { "<id>": { "sha256": "<sha256 of the routine bytes>", "armedAt": "<RFC 3339 UTC>", "lastSlot": "<RFC 3339 UTC>" | null } } }`.

- `armedAt` is when this machine first saw these exact bytes runnable, so a routine never runs for a time before it existed in this form: creating a daily 09:00 routine at 10:00 does not run it until tomorrow. Any byte change, enabling or approval sets it again.
- `lastSlot` is the latest slot handled: run, skipped or caught up.
- A look at the routines whose folder or approvals file could not be read plans nothing, forgets nothing and saves nothing. The launch look is then still to come. While Harness Memory cannot say whether team routines are shared, no entry is forgotten.
- A slot that is due runs as `schedule` when the scheduler handles it within 2 minutes. When it is later (the machine slept, the app was closed, the clock jumped forward) the scheduler takes the latest slot in `(max(lastSlot, armedAt), now]` and runs it once as `catchUp`. Older missed slots are dropped, so a laptop closed over a weekend runs a daily routine once, not three times.
- A clock set backwards never runs a slot at or before `lastSlot` again.
- Without Harness Desktop nothing runs, there are no operating system schedulers. The scheduler runs while the app does (window open, or with the setting "Keep running in the tray when the window is closed" on, in the tray). An invalid or unreadable schedule file is set aside as `.corrupt-<unix-seconds>` and rebuilt: every routine is armed at that time.

## Project identity

For a folder: take its real path (symbolic links resolved, on Windows without a `\\?\` prefix), then the nearest ancestor-or-self that contains `.git` (a directory or a file), else the folder itself. The result is the project root. Both modes use this algorithm, the same one as hooks (`hooks-format.md`, Project identity). `project` in a file holds it as a string with the OS separators, compared by exact equality. The MCP server resolves the root from the folder it is given and MUST reject a `project` that does not exist or is not a folder.

## Approvals

Written only by the app window, so a file an agent wrote never runs unreviewed. Same mechanism as hooks, same `<app_config_dir>` (`hooks-format.md`, Approvals).

Location `<app_config_dir>/routine-approvals.json`:

```json
{ "version": 1, "approvals": { "<id>": "<sha256 hex of the raw file bytes>" } }
```

- The hash is lowercase SHA-256 of the exact bytes of `<id>.json`. Any byte change, including a rewrite by an agent with the same values, makes the routine pending.
- A routine runs on its schedule only when it is valid, `enabled` and its hash equals `approvals[id]`. "Run now" does not need `enabled`.
- The desktop saves a routine file and its approval together, so routines created or edited in the desktop run at once. The user approving a pending routine stores the hash of the current bytes. Deleting a routine removes its approval, its approved copy, its run records and its schedule entry.
- The MCP server (same program, same `<app_config_dir>`, no override) reads the file read-only to report a routine as approved or pending. A missing, unreadable or invalid file means every routine is pending. The MCP server MUST NOT write it.
- The app window never saves over an approvals file it could not read. An invalid file (not JSON, over 4 MiB, not UTF-8, another version) is renamed `routine-approvals.json.corrupt-<unix-seconds>` by the next write, and every routine is pending until approved again.
- Team copies have approvals of their own in the same map, under the key `team:<id>` (see Team routines), so approving a team routine never approves a local file with the same id and bytes, and a local approval never carries over to a team copy.
- The app window drops the approval of an id only when `<root>/routines` has no entry named `<id>.json`, and the approval `team:<id>` only when `<root>/team` has none. An entry that is there but cannot be read, or is not a regular file, keeps its approval.
- The app window saves a routine only over a file whose current bytes are approved: a pending file is approved or rejected first. Each write checks again, right before its rename, that the file still holds the bytes it read, and fails with a conflict otherwise.
- Fixture `approvals/valid-approvals.json` holds real hashes of three fixtures beside it, plus one stale hash.

Approved copies `<app_config_dir>/routine-approved/<id>.json` (`team-<id>.json` for the approval `team:<id>`): the exact bytes of the approved version (mode `0600`, written with the approval, removed when the app deletes the routine), so the Routines view can show what a pending edit changed. Written only by the app window.

Routines state `<app_config_dir>/routines-state.json`: `{ "version": 1, "teamEnabled": { "<id>": "<project root>" | null }, "sharedFrom": { "<id>": "<sha256>" } }`. `teamEnabled` (see Team routines): the team routines this machine turned on, with the folder a team project routine runs in (`null` for a global one). `sharedFrom`: the SHA-256 each local routine had when the user shared it. Both objects may be absent. Written only by the app window. An invalid file is set aside as `routines-state.json.corrupt-<unix-seconds>` and rebuilt empty, so every team routine is off again.

## Team routines

A team that points Harness Memory at a team database (PostgreSQL, libSQL or MongoDB) can share routines, like hooks (`hooks-format.md`, Team hooks). The desktop never connects to that database: it runs `harness-memory routines …` commands, and Harness Memory stores the routines in its `routines` table while its config says `routines = "remote"`. Only the user shares from the app, in the Routines view: neither `harness-desktop mcp` nor Harness Memory has an MCP tool that publishes a routine, and agents never see team routines through the app. That is not a boundary: any process running as the user, an agent with a shell included, can run `harness-memory routines put` directly, and so can anyone with write access to the team database. A routine can run a command, an agent with tools or a script on every machine that turns it on. "Run team routines without asking" covers every team routine turned on here, commands and agents that run tools without asking included: trusting team routines means trusting everyone and everything that can write to that database. Leave it off unless that holds, and read each team routine before you turn it on.

Team routines are a catalogue. They arrive off: nothing runs until the person on that machine turns the routine on there.

Body. The team database stores a routine file (version 1, at most 16 KiB) with these rules on top:

- `seed` and `requires` are absent or `null`, as for every routine file.
- A project routine's `project` is the repository key `git:host/owner/repo` (Harness Memory's identity of the project root, from its `origin` remote, at most 512 bytes, no whitespace, control or invisible characters), never a local path. A project in a repository without an `origin` remote (a `path:` identity) cannot be shared. A global routine's `project` is `null` and its row has no repository.
- A body that holds a credential is refused by Harness Memory.

Sharing a local routine (approved, valid) sends its exact bytes, with only `project` replaced by the repository key for a project routine. The team copy keeps the local routine's `id`. The SHA-256 of the stored body is the hash a machine approves.

The `harness-memory routines` commands have the shapes of the `hooks` commands, with the key `routines`:

```
harness-memory routines config --json                    # {"routines": "remote"|"local", "trustTeamRoutines": bool}
harness-memory routines list --json [--project PATH] [--offset N] [--limit N]
harness-memory routines get <id> --json
harness-memory routines put --json [--project PATH] [--expect-sha SHA256] [--author NAME] < routine.json
harness-memory routines delete <id> [--expect-sha SHA256] --json
harness-memory routines delete --mine [--author NAME] --json   # {"deleted": n, "ids": ["<id>", ...]}
```

Config keys: `routines` (`HARNESS_MEMORY_ROUTINES`, `"remote"` or `"local"`, default `"local"`) and `trust_team_routines` (`HARNESS_MEMORY_TRUST_TEAM_ROUTINES`, default `false`).

`author` is a free-text label (at most 80 characters) that the writer chooses, it is not authenticated. `delete --mine` removes every row whose author equals `--author` or, without it, the user name Harness Memory records for this user (`USER`, else `USERNAME`). Without `--author` and without a usable user name it is refused (`refused`, "set --author"): `put` records such a writer as `unknown`, which may be anyone. It is a convenience for cleaning up after yourself ("Remove all my team routines"), not a permission: anyone can write any author, and anyone with database access can delete anything.

Copies. `<root>/team/` (mode `0700`) holds one envelope per team routine, `<id>.json` (mode `0600`, temp file and rename), as Harness Memory listed it:

```json
{ "version": 1, "id": "<id>", "repo": "git:github.com/o/r", "sha256": "<sha256 of body>", "author": "ana", "createdAt": "2026-10-01T10:00:00Z", "updatedAt": "2026-10-02T08:30:00Z", "body": "<the routine file>" }
```

`<root>/team/projects.json` maps each project root a list was asked for to its identity, as for hooks. Refresh (launch, every 60 seconds, when the Routines view asks, after sharing or removing, page following, scope-by-scope removal of copies no list returned, deleting `<root>/team/` when sharing is off) works exactly as for team hooks, with the routines commands and the `routines-state.json` and `routine-approvals.json` files. A copy that leaves the folder takes its approval (`team:<id>`) and approved copy with it, and its `teamEnabled` entry unless a local routine has the same id.

Turning on. A team routine runs only on the machine that turned it on: `teamEnabled[id]` exists. A run going on stops (`stopped`) when the routine may no longer run here: it was turned off here, left the team, sharing was turned off, or it is no longer trusted or approved. Turning on a team global routine stores `null`. Turning on a team project routine stores one project root, chosen by the user, whose project identity key equals the routine's `repo`: the routine runs in that one folder, not in every clone. Turning on is not approval.

Origin and status. The Routines view lists team routines after the local ones, with `origin: "team"`, the sharer (`author`) and when the team copy changed. A team routine is, in this order:

- Invalid: the envelope cannot be read, its `sha256` is not the hash of `body`, the body breaks the rules above, or its `repo` does not match the body's `project`.
- Duplicate: `<root>/routines/` has a file with the same id. The local routine wins and the team copy never runs.
- Off: not turned on on this machine (the default), or the body says `enabled: false`.
- Pending: "Run team routines without asking" (`trust_team_routines = true` in Harness Memory's config) is off and the exact body bytes are not approved (see the trust rule).
- Paused: a project routine's chosen folder is missing or is no longer the repository.
- Active: it runs.

Trust rule. A team routine runs only while sharing is on, only after the user turned it on here, and only with "Run team routines without asking" on or after the user approved its exact bytes on this machine. Trust covers every team routine, the same way the trust settings of team hooks and scripts do: a command, an agent with `approvalMode: "yolo"` or `trustOnce: true`, any action. With trust off each one needs its exact bytes approved. Approval uses `routine-approvals.json` (key `team:<id>`) and `routine-approved/` (`team-<id>.json`), so a changed team routine shows what changed since the version this machine approved, and any change makes it pending again.

Run start. A team routine that runs on trust alone (its exact bytes not approved here) is checked again when a run starts: `harness-memory routines get <id> --json` MUST answer a row whose `sha256` and body hash equal the copy's, otherwise the run fails ("changed in the team database", "no longer in the team database", or Harness Memory could not confirm it). The copy in `<root>/team/` is a file any process of the user can write, so trust covers what the team database holds, not what the copy says. A routine whose exact bytes are approved needs no such check.

Update and copy. After a share, `sharedFrom[id]` holds the hash of the local file shared. When the local file changes, the team copy offers "Update team copy", which stores the new body over the team copy it names (`--expect-sha`). "Copy to my routines" (only for a team routine that may run: trusted or approved, never a pending one) writes it as a local, approved routine with the same id (a project routine gets the chosen local project root as `project`), which then wins over the team copy. "Delete all routines" never touches team routines.

## Agent rules (MCP server `harness-desktop mcp`)

The MCP server has `routine_list`, `routine_get`, `routine_add`, `routine_update` and `routine_remove`, and works like the hook tools:

- A new routine gets `createdBy: "agent"` and `updatedBy: "agent"`. It is pending until the user approves it in the Routines view, so an agent's routine never runs unreviewed.
- Every edit the MCP server writes on behalf of an agent (any routine, including a user's) sets `updatedBy: "agent"` and a new `updatedAt`. Its bytes change, so it is pending until the user approves that exact version in the app.
- `createdBy` never changes. The MCP server keeps it on edits and rejects an edit that would change it.
- The MCP server removes a routine only when the file has `createdBy: "agent"`. Otherwise it refuses and tells the agent to ask the user.
- The MCP server keeps the `scope` and `project` of a routine with `createdBy: "desktop"`: an edit that would change them is refused. An agent may move a routine an agent created, which stays pending.
- The MCP server shows a routine's `prompt` and `command` to the agent with secrets masked (the same redaction as command output). The file keeps what was written.
- Edits and removals carry the lowercase SHA-256 of the file bytes the agent last read (as `routine_get` returned it). The server refuses when the current bytes hash differently.
- The MCP server keeps `createdAt`, `createdBy` and unknown fields on edits and validates everything above before writing.
- `routine_list` and `routine_get` show only the global routines and those of the project the server works in (its working folder), like a listing for that project. `routine_list` refuses a `project` of another project, and `routine_get` answers "no routine has this id" for another project's routine.
- Agents cannot run a routine, approve one, read run records, or see team routines. The tools only list and read files in `<root>/routines/`.

## Validity summary

Invalid files are listed with a reason and never run. Reasons include: not JSON, over 16 KiB, wrong `version`, id mismatch, a missing or mistyped field, a limit exceeded, a bad `when` (a bad `at`, an `every` out of range, a malformed or never-firing cron), a bad `action` (an unknown harness, an agent harness without a chat, an `approvalMode` with `pi`, a `"yolo"` with `claude`, a `trustOnce` with `omp` or `opencode`, a bad `maxMinutes` or `timeoutSecs`, a bad script name or scope), invisible characters, a `seed` or `requires` value, a scope and project mismatch, over the 64 limit. A fixture file is checked by content only, its name does not follow `<id>.json`. Loader tests copy a fixture to `<id>.json` first.

Fixtures: every `valid-*.json` MUST be accepted and every `invalid-*.json` MUST be rejected. The `approvals/` subfolder is not matched by a non-recursive glob of the folder. The `.gitattributes` file there keeps the bytes exact (the approvals fixture holds their hashes). Harness Memory's copy adds `valid-team-project.json` and `invalid-team-local-path.json` and explains them in its `README.md`.
