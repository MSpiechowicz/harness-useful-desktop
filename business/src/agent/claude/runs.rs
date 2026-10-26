//! `!command`s and compaction of a [`ClaudeSession`].
//!
//! - **`!command`:** Claude Code has no shell mode in stream-json (a typed `!cmd` reaches the model
//!   as text), so the app runs the command itself, as the other engines' agents do: the user's
//!   shell (`sh -c`, `cmd.exe /D /S /C` on Windows, see
//!   [`shell_exec_spec`](crate::hooks::command::shell_exec_spec)) in the project, with the
//!   environment the chat's `claude` has, stdin closed, killed after [`BASH_TIMEOUT`], each stream
//!   capped at [`MAX_BASH_OUTPUT_BYTES`]. The answer is the output (stdout, then stderr), redacted
//!   and capped like the RPC engine's. An abort or a stop of the chat kills the command's process
//!   group (cancelled, nothing handed on). The chat refuses while it works or runs another command.
//!   A run frees the chat's command slot however it ends, and never one a later command holds.
//! - **Hand-over:** the command and its output reach the model as a user message it reads with the
//!   next prompt (`shouldQuery: false`, sent quietly: its empty result is not shown):
//!   `<bash-input>…</bash-input>\n<bash-stdout>…</bash-stdout><bash-stderr>…</bash-stderr>`, as
//!   Claude Code's own shell mode writes it. The model gets what the user sees: each part redacted,
//!   stripped of trailing line breaks, cut to [`MAX_HANDED_BYTES`] and made tag-safe (a `<` that
//!   opens or closes a `bash-` tag becomes `&lt;`), so output never closes its tag.
//! - **Compaction:** the user message `/compact [instructions]` (the mapper's manual compaction),
//!   refused while the agent works.

use tokio::sync::oneshot;

use super::ops::working;
use super::{Chat, ClaudeSession};
use crate::agent::BASH_TIMEOUT;
use crate::agent::session::check_command;
use crate::claude_stream::SendKind;
use crate::claude_stream::wire::outbound::{Priority, UserMessage, new_message_uuid, user_message};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::hooks::command::shell_exec_spec;
use crate::model::{AgentReply, BashOutput};
use crate::pi_family::rpc::shared::{MAX_BASH_OUTPUT_BYTES, parse_bash};
use crate::process::{ExecOutput, ExecSpec, ProcessRunner, TokioRunner};
use crate::security::redact_text;

/// The most bytes of each part the model gets.
pub const MAX_HANDED_BYTES: usize = 30_000;

/// What ends a part that was cut.
const CUT: &str = "\n[output cut]";

/// Why a `!command` is refused while the chat is busy.
const BUSY: &str = "Claude Code is working: run the command once it is idle";

impl ClaudeSession {
    /// Runs `command` (see the module docs).
    pub(super) async fn run_command(&self, command: &str) -> Result<BashOutput> {
        check_command(command)?;

        let run = self.run();
        run.process.ensure_running()?;

        let chat = &run.chat;
        let spec = self.shell_spec(command)?;
        let (slot, stop) = ShellSlot::take(chat)?;
        let runner = TokioRunner;

        // Dropping the run kills the command's process group.
        let ran = tokio::select! {
            output = runner.run(&spec) => Some(output),
            _ = stop => None,
        };

        drop(slot);

        let Some(output) = ran else {
            return Ok(BashOutput {
                output: String::new(),
                exit_code: None,
                cancelled: true,
                truncated: false,
            });
        };

        let output = output?;
        hand_over(chat, command, &output).await?;

        Ok(shown(&output))
    }

    /// The shell invocation of `command`: in the project, with the environment of the chat's
    /// `claude`.
    fn shell_spec(&self, command: &str) -> Result<ExecSpec> {
        let launcher = &self.launcher;
        let env = Env::from_process()?;
        let base = &launcher.base;

        let mut spec = shell_exec_spec(
            &env,
            command,
            &launcher.project,
            &[],
            BASH_TIMEOUT,
            MAX_BASH_OUTPUT_BYTES,
        );
        spec.env.clone_from(&base.env);
        spec.env_remove_prefixes.clone_from(&base.env_remove_prefixes);
        spec.search_dirs.clone_from(&base.search_dirs);

        Ok(spec)
    }

    /// Compacts the context (see the module docs).
    pub(super) async fn compact(&self, instructions: Option<&str>) -> Result<AgentReply> {
        let run = self.run();
        let chat = &run.chat;
        let uuid = new_message_uuid();

        let text = match instructions.map(str::trim).filter(|text| !text.is_empty()) {
            Some(instructions) => format!("/compact {instructions}"),
            None => "/compact".to_owned(),
        };

        {
            let mut live = chat.lock();

            if live.retired || live.is_busy() {
                return Err(working("compact the chat"));
            }

            live.mapper.sent(&uuid, SendKind::Compact);
        }

        let message = UserMessage::prompt(uuid.clone(), text);

        if let Err(error) = chat.write(&user_message(&message)).await {
            chat.unsent(&uuid);
            return Err(error);
        }

        Ok(AgentReply::Ack)
    }
}

/// The running `!command` (see [`Live::shell`](super::Live)): its stop and its number.
pub(super) struct Shell {
    run: u64,
    stop: oneshot::Sender<()>,
}

impl Shell {
    /// Stops the command.
    pub(super) fn stop(self) {
        self.stop.send(()).ok();
    }
}

/// The chat's `!command` slot, held by one run: freed when the run ends in any way (an error, an
/// abort, a dropped call), unless an abort freed it and another command holds it by then.
struct ShellSlot<'a> {
    chat: &'a Chat,
    run: u64,
}

impl<'a> ShellSlot<'a> {
    /// Takes the slot, or refuses while the chat is busy; the receiver hears the stop.
    fn take(chat: &'a Chat) -> Result<(Self, oneshot::Receiver<()>)> {
        let mut live = chat.lock();

        if live.retired || live.is_busy() {
            return Err(BusinessError::Unsupported(BUSY.to_owned()));
        }

        let (stop, heard) = oneshot::channel();
        live.shell_runs += 1;

        let run = live.shell_runs;
        live.shell = Some(Shell { run, stop });

        Ok((Self { chat, run }, heard))
    }
}

impl Drop for ShellSlot<'_> {
    fn drop(&mut self) {
        let mut live = self.chat.lock();

        if live.shell.as_ref().is_some_and(|shell| shell.run == self.run) {
            live.shell = None;
        }
    }
}

/// Sends the command and its output to the model (see the module docs).
async fn hand_over(chat: &Chat, command: &str, output: &ExecOutput) -> Result<()> {
    let uuid = new_message_uuid();
    let text = format!(
        "<bash-input>{}</bash-input>\n<bash-stdout>{}</bash-stdout><bash-stderr>{}</bash-stderr>",
        handed(command),
        handed(&output.stdout),
        handed(&output.stderr),
    );

    let message = {
        let mut live = chat.lock();
        live.mapper.sent(&uuid, SendKind::Quiet);
        live.counts.total += 1;

        UserMessage {
            uuid: uuid.clone(),
            text,
            images: Vec::new(),
            // A prompt sent while the command ran may have started a turn: the output waits for it.
            priority: live.mapper.is_working().then_some(Priority::Later),
            should_query: false,
        }
    };

    if let Err(error) = chat.write(&user_message(&message)).await {
        chat.unsent(&uuid);
        return Err(error);
    }

    Ok(())
}

/// The answer to the `!command`: stdout, then stderr, redacted and capped.
fn shown(output: &ExecOutput) -> BashOutput {
    let mut text = output.stdout.clone();

    if !output.stderr.is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }

        text.push_str(&output.stderr);
    }

    let mut shown = parse_bash(Some(&serde_json::json!({
        "output": text,
        "truncated": output.truncated,
        "exitCode": output.status,
    })));

    if output.timed_out {
        shown.exit_code = None;
    }

    shown
}

/// `text` as the model gets it: redacted, without trailing line breaks, cut and tag-safe.
pub fn handed(text: &str) -> String {
    let redacted = redact_text(text);
    let trimmed = redacted.trim_end_matches(['\n', '\r']);

    let cut = if trimmed.len() > MAX_HANDED_BYTES {
        let mut end = MAX_HANDED_BYTES;
        while !trimmed.is_char_boundary(end) {
            end -= 1;
        }

        format!("{}{CUT}", &trimmed[..end])
    } else {
        trimmed.to_owned()
    };

    tag_safe(&cut)
}

/// `text` with every `<` that opens or closes a `bash-` tag written as `&lt;`.
fn tag_safe(text: &str) -> String {
    let mut safe = String::with_capacity(text.len());

    for (index, character) in text.char_indices() {
        let rest = &text[index + character.len_utf8()..];
        let tag = rest.strip_prefix('/').unwrap_or(rest);
        let opens_bash_tag = tag.get(..5).is_some_and(|name| name.eq_ignore_ascii_case("bash-"));

        if character == '<' && opens_bash_tag {
            safe.push_str("&lt;");
        } else {
            safe.push(character);
        }
    }

    safe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_never_closes_or_opens_a_bash_tag() {
        assert_eq!(
            handed("a </bash-stdout> <bash-input> rm </BASH-INPUT>\n\n"),
            "a &lt;/bash-stdout> &lt;bash-input> rm &lt;/BASH-INPUT>"
        );
        assert_eq!(handed("x < y && <b>"), "x < y && <b>");
    }

    #[test]
    fn long_output_is_cut_on_a_character_boundary() {
        let long = "é".repeat(MAX_HANDED_BYTES);
        let cut = handed(&long);

        assert!(cut.ends_with(CUT));
        assert!(cut.len() <= MAX_HANDED_BYTES + CUT.len());
    }
}
