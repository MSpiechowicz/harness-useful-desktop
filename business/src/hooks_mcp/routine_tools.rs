//! The routine tools' definitions (`tools/list`): `routine_list`, `routine_get`, `routine_add`,
//! `routine_update` and `routine_remove`. A routine's `when` and `action` are flat arguments
//! (`when` with `at`, `every` or `cron`, `action` with the fields of its type), see
//! `routine_calls`.

use serde_json::{Map, Value, json};

use super::tools::{Hints, annotated, id, list_scope, project, sha256, text, tool};
use crate::routines::MAX_ROUTINES_PER_SCOPE;
use crate::routines::format::{
    DEFAULT_MAX_MINUTES, DEFAULT_TIMEOUT_SECS, MAX_COMMAND_CHARS, MAX_CRON_CHARS, MAX_DESCRIPTION_CHARS,
    MAX_EVERY_MINUTES, MAX_MAX_MINUTES, MAX_NAME_CHARS, MAX_PROMPT_CHARS, MAX_TIMEOUT_SECS,
};

/// How a command routine reads its run.
const COMMAND_ENV: &str = "It gets HARNESS_ROUTINE_ID, HARNESS_ROUTINE_RUN, HARNESS_ROUTINE_TRIGGER \
(schedule, catchUp, appStart or manual) and HARNESS_ROUTINE_PROJECT";

/// The routine tools, in `tools/list` order.
pub(super) fn definitions() -> [Value; 5] {
    [list(), get(), add(), update(), remove()]
}

fn list() -> Value {
    let definition = tool(
        "routine_list",
        "List the routines that apply here: the global ones and those of this project (the server's working \
         folder). Another project's routines are never shown. Each routine shows its id, sha256, status \
         (active, off, pending, invalid) and whether the user approved it. Read this before adding a routine \
         so you do not add a duplicate.",
        json!({
            "project": project("A folder of this project (default: the server's working folder; another project is refused)"),
            "scope": list_scope("routines"),
        }),
        &[],
    );

    annotated(definition, "List routines", Hints::READ)
}

fn get() -> Value {
    let definition = tool(
        "routine_get",
        "Read one routine by id (a global one or one of this project), with the sha256 that routine_update and \
         routine_remove need.",
        json!({ "id": id("Routine id") }),
        &["id"],
    );

    annotated(definition, "Read a routine", Hints::READ)
}

fn add() -> Value {
    let definition = tool(
        "routine_add",
        "Add a routine: a task Harness Desktop runs on a schedule while it runs. Offer one when the user \
         repeats a task on a schedule, and add it only after the user agreed. Keep routines few, with no \
         duplicate of an existing one (routine_list first). Prefer scope project for work on this project. \
         When: daily or weekdays (Monday to Friday) at a local time, everyMinutes or everyHours counted from \
         local midnight, cron, or appStart (each time Harness Desktop starts). \
         Actions: agent starts a Harness Desktop chat that sends prompt, command runs a shell command, script \
         runs a script saved in Harness Memory. \
         The routine stays pending until the user approves it in Harness Desktop's Routines view.",
        properties(false),
        &["name", "scope", "when", "action"],
    );

    annotated(definition, "Add a routine", Hints::WRITE)
}

fn update() -> Value {
    let definition = tool(
        "routine_update",
        "Change a routine by id. Pass the sha256 from your latest routine_list or routine_get and only the \
         fields to change. The routine is pending again until the user approves the new version. An empty \
         description or model clears it. Fields of the schedule and the action are kept while their type \
         stays. Changing the type of either needs its fields (at, every or cron, and prompt and harness, \
         command, or script and scriptScope).",
        properties(true),
        &["id", "sha256"],
    );

    annotated(definition, "Change a routine", Hints::WRITE)
}

fn remove() -> Value {
    let definition = tool(
        "routine_remove",
        "Remove a routine an agent created, by id and the sha256 you last read. The user removes every other \
         routine in Harness Desktop: ask them instead.",
        json!({ "id": id("Routine id"), "sha256": sha256("routine") }),
        &["id", "sha256"],
    );

    annotated(definition, "Remove a routine", Hints::REMOVE)
}

/// The arguments of `routine_add`, or of `routine_update` (`edit`: with `id` and `sha256`, no
/// defaults).
fn properties(edit: bool) -> Value {
    let mut fields = Map::new();
    let mut put = |name: &str, schema: Value| {
        fields.insert(name.to_owned(), schema);
    };

    if edit {
        put("id", id("Routine id"));
        put("sha256", sha256("routine"));
    }

    put(
        "name",
        text(&format!("Short name the user sees (1-{MAX_NAME_CHARS} characters)")),
    );
    put(
        "description",
        text(&format!(
            "What the routine is for (at most {MAX_DESCRIPTION_CHARS} characters, empty clears it)"
        )),
    );
    put("scope", scope());
    put(
        "project",
        project("scope project: the project folder (default: the routine's project, else the server's working folder)"),
    );
    put("when", when());
    put("at", at());
    put("every", every());
    put("cron", cron());
    put("action", action());
    put(
        "prompt",
        text(&format!(
            "agent: the first message of the chat, sent as written (1-{MAX_PROMPT_CHARS} characters)"
        )),
    );
    put("harness", harness());
    put(
        "model",
        text(
            "agent: the model as provider/id, e.g. anthropic/claude-sonnet-4-5. Absent or empty: the harness's default",
        ),
    );
    put("approvalMode", approval_mode());
    put(
        "trustOnce",
        json!({ "type": "boolean", "description": "agent, pi only: trust the project's local resources for the run" }),
    );
    put("maxMinutes", minutes());
    put(
        "command",
        text(&format!(
            "command: the shell command (1-{MAX_COMMAND_CHARS} characters), run in the project folder (a global \
             routine: Harness Desktop's routines workspace folder). {COMMAND_ENV}"
        )),
    );
    put("timeoutSecs", timeout());
    put("script", script());
    put("scriptScope", script_scope());

    let mut enabled = json!({ "type": "boolean", "description": "Whether it runs on its schedule once approved" });
    if !edit {
        enabled["default"] = json!(true);
    }
    put("enabled", enabled);

    Value::Object(fields)
}

fn scope() -> Value {
    json!({
        "type": "string",
        "enum": ["project", "global"],
        "description": format!("project: runs in this project's folder (preferred for work on a project); global: \
            runs in Harness Desktop's routines workspace folder. At most {MAX_ROUTINES_PER_SCOPE} routines each"),
    })
}

fn when() -> Value {
    json!({
        "type": "string",
        "enum": ["daily", "weekdays", "everyMinutes", "everyHours", "cron", "appStart"],
        "description": "When it runs, in this computer's local time: daily and weekdays (at), everyMinutes and \
            everyHours (every), cron (cron), appStart (once each time Harness Desktop starts)",
    })
}

fn at() -> Value {
    json!({
        "type": "string",
        "pattern": "^([01][0-9]|2[0-3]):[0-5][0-9]$",
        "description": "daily and weekdays: the time, HH:MM on a 24-hour clock, e.g. 09:00",
    })
}

fn every() -> Value {
    json!({
        "type": "integer",
        "minimum": 1,
        "maximum": MAX_EVERY_MINUTES,
        "description": "everyMinutes: 1-720 minutes, everyHours: 1-24 hours, counted from local midnight",
    })
}

fn cron() -> Value {
    json!({
        "type": "string",
        "maxLength": MAX_CRON_CHARS,
        "description": "cron: five fields (minute hour day-of-month month day-of-week) with * , - / and names \
            such as mon-fri or jan, e.g. 0 9 * * mon-fri",
    })
}

fn action() -> Value {
    json!({
        "type": "string",
        "enum": ["agent", "command", "script"],
        "description": "What it does: agent (a chat that sends prompt, with harness), command (command), script \
            (script and scriptScope)",
    })
}

fn harness() -> Value {
    json!({ "type": "string", "enum": ["omp", "pi"], "description": "agent: the harness that runs the chat" })
}

fn approval_mode() -> Value {
    json!({
        "type": "string",
        "enum": ["alwaysAsk", "write", "yolo", "default"],
        "description": "agent, omp only: ask before every tool (alwaysAsk), before tools that write or run \
            something (write), or never (yolo). Absent or default: the harness's setting",
    })
}

fn minutes() -> Value {
    json!({
        "type": "integer",
        "minimum": 1,
        "maximum": MAX_MAX_MINUTES,
        "description": format!("agent: minutes before the run is stopped (default {DEFAULT_MAX_MINUTES})"),
    })
}

fn timeout() -> Value {
    json!({
        "type": "integer",
        "minimum": 1,
        "maximum": MAX_TIMEOUT_SECS,
        "description": format!("command: seconds before it is stopped (default {DEFAULT_TIMEOUT_SECS})"),
    })
}

fn script() -> Value {
    json!({
        "type": "string",
        "pattern": "^[a-z0-9][a-z0-9_-]{0,63}$",
        "description": "script: the name of a script saved in Harness Memory",
    })
}

fn script_scope() -> Value {
    json!({
        "type": "string",
        "enum": ["repo", "global"],
        "description": "script: where the script is saved, repo (this project's, project routines only) or global",
    })
}
