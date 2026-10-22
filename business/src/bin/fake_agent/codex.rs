//! `fake_agent --dialect codex --scenario <file.jsonl>`: a scripted `codex app-server` (Codex
//! 0.154.0, recordings in `tests/fixtures/codex/app_server`) for the Codex chat tests. It reads
//! JSON-RPC frames without the `jsonrpc` field on stdin and writes them on stdout, like Codex.
//!
//! # Scenario format
//!
//! JSONL, one step per line; blank lines and `//` comments are skipped and `{"include":
//! "other.jsonl"}` inlines a file (relative to the scenario).
//!
//! - `{"expect": "turn/start", "as": "name", "match": {...}, "reply": {...}}` — wait for the app's
//!   next request or notification with that `method` (frames of other methods are kept for later
//!   expects), capture its `id` as `name` (default: the method), check that it carries every
//!   `match` field (keys starting with `/` are JSON pointers, `"/params/threadId"`), then answer
//!   `{"id": …, "result": reply}` (`{}` without a `reply`; `/work/project` moved as in `emit`). `"error": "text"` answers `{"id": …,
//!   "error": {"code": -32600, "message": "text"}}` instead (an object: that error), and
//!   `"hold": true` nothing until `{"respond": "name", "reply": {...}}` (or `"error"`). A
//!   notification (`initialized`) gets no answer.
//! - `{"expect": "answer", "as": "name", "match": {...}}` — wait for the app's next answer to one
//!   of Codex's requests (a frame with an `id` and no `method`) and check `match`
//!   (`"/result/decision"`, …). It gets no answer.
//! - `{"emit": {...}}` — write a frame; string values `"$name"` become the value captured as
//!   `name`, and the recorded project directory `/work/project` at the start of a string becomes
//!   the working directory. With `"times": n` it is written n times, `"$index"` being 0, 1, … in
//!   turn (a request id). `"$pad:<bytes>"` becomes that many `x` (a frame over the app's limit).
//! - `{"replay": "../codex/app_server/02_turn_approvals_accept.jsonl", "from": 3, "to": 40,
//!   "except": ["response", "mcpServer/startupStatus/updated"], "replace": {"<recorded value>":
//!   "$name"}}` — write Codex's frames (rows with `dir: in`) of that recording (relative to the
//!   scenario) from line `from` to line `to` (1-based, both included; the whole file without
//!   them), leaving out frames whose `method` is in `except` (`response`: the answers to the
//!   recorded app's requests, whose ids the app does not know). A value equal to a `replace` key
//!   becomes its value (`"$name"` the captured one), and the recorded project directory becomes
//!   the working directory.
//! - `{"sleep": ms}`, `{"stderr": "line"}`, `{"stderrRepeat": …}` as for the other dialects;
//!   `{"exit": code}`, `{"hang": true}` (ignore stdin EOF, run until killed).
//!
//! After the last step the fake waits for stdin EOF and exits 0, as Codex does. An `expect` that
//! sees EOF exits 0 too; a failed `match` exits 4 with a message on stderr.

use std::path::Path;

use serde_json::{Value, json};

use crate::check_match;
use crate::recorded::{Dialect, Player, write};

/// The project directory of the recordings.
const RECORDED_PROJECT: &str = "/work/project";

/// The `expect` of the app's answer to one of Codex's requests.
const ANSWER: &str = "answer";

/// The code of an `error` step given as text (Codex's invalid request).
const INVALID_REQUEST: i64 = -32600;

/// Plays `steps` (see the module docs) and exits.
pub fn serve(steps: &[Value], scenario: &Path) -> ! {
    Player::play::<Codex>(steps, scenario, RECORDED_PROJECT, "in")
}

struct Codex;

impl Dialect for Codex {
    /// A frame's method, or `response` for a response.
    fn kind(frame: &Value) -> Option<String> {
        match frame["method"].as_str() {
            Some(method) => Some(method.to_owned()),
            None => frame.get("id").map(|_| "response".to_owned()),
        }
    }

    fn index(index: u64) -> Value {
        json!(index)
    }

    fn expect(player: &mut Player, method: &str, step: &Value) {
        let frame = player.inbox.next(|frame: &Value| match method {
            ANSWER => frame.get("method").is_none() && frame.get("id").is_some(),
            _ => frame["method"] == method,
        });
        let name = step.get("as").and_then(Value::as_str).unwrap_or(method).to_owned();

        check_match(method, &frame, step);

        let id = frame.get("id").cloned().unwrap_or(Value::Null);
        player.captured.insert(name, id.clone());

        let held = step.get("hold").and_then(Value::as_bool).unwrap_or(false);

        if method != ANSWER && !id.is_null() && !held {
            Self::answer(player, &id, step);
        }
    }

    fn answer(player: &Player, id: &Value, step: &Value) {
        let frame = match step.get("error") {
            Some(Value::String(message)) => {
                json!({ "id": id, "error": { "code": INVALID_REQUEST, "message": message } })
            }
            Some(error) => json!({ "id": id, "error": error }),
            None => {
                let reply = step
                    .get("reply")
                    .map_or_else(|| json!({}), |reply| player.substitute(reply));
                json!({ "id": id, "result": player.recordings.recorded(&reply, &[]) })
            }
        };

        write(&frame);
    }
}
