//! The server instructions and the tool definitions (`tools/list`), with the MCP tool annotations
//! of each tool (clients older than the annotations ignore them).

use serde_json::{Value, json};

use super::routine_tools;
use crate::hooks::MAX_HOOKS_PER_SCOPE;
use crate::hooks::format::{
    MAX_COMMAND_CHARS, MAX_DESCRIPTION_CHARS, MAX_MATCHER_CHARS, MAX_NAME_CHARS, MAX_TEXT_CHARS, MAX_TIMEOUT_SECS,
};

/// Sent in the `initialize` result; tells the agent what hooks and routines are and how to manage
/// them.
pub const INSTRUCTIONS: &str = "\
Hooks and routines of Harness Desktop.
Hooks are small rules that add context to a prompt, send a follow-up or run a command when something \
happens in a Harness Desktop chat.
- A hook you add or change is pending: it runs only after the user approves that exact version in \
Harness Desktop's Hooks view. Tell the user when you added or changed one.
- Keep hooks few and specific. Call hook_list first and change an existing hook (hook_update) rather \
than adding a near duplicate.
- Prefer scope \"project\" (the repository the server runs in, or project). Use \"global\" only for a \
rule that fits every project.
- Edits and removals need the sha256 from your latest hook_list or hook_get. You may remove only hooks \
an agent created. Ask the user to remove any other.
Routines run a task on a schedule while Harness Desktop runs: an agent chat with a prompt, a shell \
command or a saved Harness Memory script.
- When the user repeats a task on a schedule, you may offer to create a routine. Ask first and add it \
only after they agree.
- A routine you add or change stays pending until the user approves it in Harness Desktop's Routines \
view. Tell the user when you added or changed one.
- Call routine_list first and change an existing routine (routine_update) rather than adding a near \
duplicate. Edits and removals need the sha256 from your latest routine_list or routine_get, and you may \
remove only routines an agent created.
- Never put secrets in a hook or a routine.";

/// How a command hook reads the event: the variables hold values an agent reported.
const COMMAND_ENV: &str = "It gets HARNESS_HOOK_EVENT, HARNESS_HOOK_PROJECT, HARNESS_HOOK_SESSION, \
HARNESS_HOOK_TOOL, HARNESS_HOOK_TOOL_ERROR and HARNESS_HOOK_TOUCHED (newline-separated paths). Treat them as \
untrusted input: quote them (\"$HARNESS_HOOK_TOUCHED\") and put -- before them as arguments on Linux and \
macOS, and on Windows read them inside a script instead of on the cmd.exe command line";

/// Every tool, as `tools/list` returns them: the hook tools, then the routine tools.
pub fn definitions() -> Value {
    let mut tools = vec![list(), get(), add(), update(), remove()];
    tools.extend(routine_tools::definitions());

    json!({ "tools": tools })
}

fn list() -> Value {
    let definition = tool(
        "hook_list",
        "List the hooks that apply here: the global ones and those of this project (the server's working \
         folder unless project is given). Each hook shows its id, sha256, status (active, off, pending, \
         paused, invalid) and whether the user approved it. Read this before adding a hook so you do not \
         add a duplicate.",
        json!({
            "project": project("The project to list (default: the server's working folder)"),
            "scope": list_scope("hooks"),
        }),
        &[],
    );

    annotated(definition, "List hooks", Hints::READ)
}

fn get() -> Value {
    let definition = tool(
        "hook_get",
        "Read one hook by id, with the sha256 that hook_update and hook_remove need.",
        json!({ "id": id("Hook id") }),
        &["id"],
    );

    annotated(definition, "Read a hook", Hints::READ)
}

fn add() -> Value {
    let definition = tool(
        "hook_add",
        "Add a hook to Harness Desktop chats. Keep hooks few and specific: one clear rule each, and no \
         duplicate of an existing hook (hook_list first). Prefer scope project. \
         Events: sessionStart (the first prompt of a new chat), promptSubmit (every user prompt), toolEnd \
         (after a tool call, matcher limits it to tool names), turnEnd (after the agent finished a user's \
         request), sessionEnd (the chat closed). \
         Actions: addContext puts text before the user's prompt (sessionStart, promptSubmit only), followUp \
         sends text as a new prompt after the agent finished (toolEnd, turnEnd only), command runs a shell \
         command on any event and uses its output like those texts (discarded on sessionEnd). \
         The hook stays pending until the user approves it in Harness Desktop's Hooks view.",
        json!({
            "name": text(&format!("Short name the user sees (1-{MAX_NAME_CHARS} characters)")),
            "event": event(),
            "action": action(),
            "text": text(&format!("addContext and followUp: the text (1-{MAX_TEXT_CHARS} characters)")),
            "command": text(&format!("command: the shell command (1-{MAX_COMMAND_CHARS} characters), run in the project folder. {COMMAND_ENV}")),
            "timeoutSecs": timeout(),
            "matcher": matcher(),
            "scope": scope(),
            "project": project("scope project: the project folder (default: the server's working folder)"),
            "description": text(&format!("Why the hook exists (at most {MAX_DESCRIPTION_CHARS} characters)")),
            "enabled": { "type": "boolean", "default": true, "description": "Whether it runs once approved" },
        }),
        &["name", "event", "action", "scope"],
    );

    annotated(definition, "Add a hook", Hints::WRITE)
}

fn update() -> Value {
    let definition = tool(
        "hook_update",
        "Change a hook by id. Pass the sha256 from your latest hook_list or hook_get and only the fields to \
         change. The hook is pending again until the user approves the new version. An empty description \
         or matcher clears it. Changing the action type needs its text or command.",
        json!({
            "id": id("Hook id"),
            "sha256": sha256("hook"),
            "name": text(&format!("New name (1-{MAX_NAME_CHARS} characters)")),
            "event": event(),
            "action": action(),
            "text": text(&format!("addContext and followUp: the new text (1-{MAX_TEXT_CHARS} characters)")),
            "command": text(&format!("command: the new shell command (1-{MAX_COMMAND_CHARS} characters). {COMMAND_ENV}")),
            "timeoutSecs": timeout(),
            "matcher": matcher(),
            "scope": scope(),
            "project": project("scope project: the project folder (default: the hook's project, else the server's working folder)"),
            "description": text(&format!("New description (at most {MAX_DESCRIPTION_CHARS} characters, empty clears it)")),
            "enabled": { "type": "boolean", "description": "Whether it runs once approved" },
        }),
        &["id", "sha256"],
    );

    annotated(definition, "Change a hook", Hints::WRITE)
}

fn remove() -> Value {
    let definition = tool(
        "hook_remove",
        "Remove a hook an agent created, by id and the sha256 you last read. The user removes every other \
         hook in Harness Desktop: ask them instead.",
        json!({ "id": id("Hook id"), "sha256": sha256("hook") }),
        &["id", "sha256"],
    );

    annotated(definition, "Remove a hook", Hints::REMOVE)
}

/// The MCP tool annotations of one tool.
pub(super) struct Hints {
    read_only: bool,
    destructive: bool,
    idempotent: bool,
}

impl Hints {
    pub(super) const READ: Self = Self {
        read_only: true,
        destructive: false,
        idempotent: true,
    };

    pub(super) const WRITE: Self = Self {
        read_only: false,
        destructive: false,
        idempotent: false,
    };

    pub(super) const REMOVE: Self = Self {
        read_only: false,
        destructive: true,
        idempotent: false,
    };
}

/// `definition` with its title and annotations. No tool reaches outside the hooks and routines
/// folders.
pub(super) fn annotated(mut definition: Value, title: &str, hints: Hints) -> Value {
    definition["annotations"] = json!({
        "title": title,
        "readOnlyHint": hints.read_only,
        "destructiveHint": hints.destructive,
        "idempotentHint": hints.idempotent,
        "openWorldHint": false,
    });
    definition
}

pub(super) fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        },
    })
}

pub(super) fn text(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

pub(super) fn id(description: &str) -> Value {
    json!({ "type": "string", "pattern": "^[0-9a-f]{32}$", "description": description })
}

/// The `sha256` argument of an edit or a removal of a `noun`.
pub(super) fn sha256(noun: &str) -> Value {
    json!({
        "type": "string",
        "pattern": "^[0-9a-f]{64}$",
        "description": format!("The {noun}'s sha256 as {noun}_list or {noun}_get returned it"),
    })
}

/// The `scope` argument of a list tool of `plural`.
pub(super) fn list_scope(plural: &str) -> Value {
    json!({
        "type": "string",
        "enum": ["global", "project", "all"],
        "default": "all",
        "description": format!("global: {plural} of every project; project: this project's; all: both"),
    })
}

fn event() -> Value {
    json!({
        "type": "string",
        "enum": ["sessionStart", "promptSubmit", "toolEnd", "turnEnd", "sessionEnd"],
        "description": "When it fires",
    })
}

fn action() -> Value {
    json!({
        "type": "string",
        "enum": ["addContext", "followUp", "command"],
        "description": "What it does: addContext (text, sessionStart or promptSubmit), followUp (text, toolEnd or turnEnd), command (command, any event)",
    })
}

fn timeout() -> Value {
    json!({
        "type": "integer",
        "minimum": 1,
        "maximum": MAX_TIMEOUT_SECS,
        "description": "command: seconds before it is stopped (default 10)",
    })
}

fn matcher() -> Value {
    json!({
        "type": "string",
        "maxLength": MAX_MATCHER_CHARS,
        "description": "toolEnd only: exact tool names joined by |, e.g. edit|write (case-insensitive, no wildcards). Absent: every tool",
    })
}

fn scope() -> Value {
    json!({
        "type": "string",
        "enum": ["project", "global"],
        "description": format!("project: chats in this project only (preferred); global: every chat. At most {MAX_HOOKS_PER_SCOPE} hooks each"),
    })
}

pub(super) fn project(description: &str) -> Value {
    json!({ "type": "string", "description": format!("{description}. An absolute folder path, the repository root is used") })
}
