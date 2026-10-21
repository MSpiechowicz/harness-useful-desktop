//! Live output of the chat's running `shell` tool calls, streamed into their cards as OpenCode 1
//! streamed `bash` output.
//!
//! - OpenCode 2 reports only the shell a call runs its command in (`session.tool.progress`
//!   `{shellID}`), and keeps the command's combined output in that shell. Each such call gets a
//!   task that reads `GET /api/shell/{id}/output` from where it left off, at once and then every
//!   [`POLL`] (as OpenCode's own clients do), and hands what it has to the mapper, which sends it
//!   as `toolUpdate` (redacted) while the call runs.
//! - Bounded: a card shows the newest [`MAX_OUTPUT`] bytes, led by [`CUT`] once older output was
//!   left out, and a read skips output too old to show.
//! - The task ends once the call ended (or the chat no longer has it) or a read fails: the call's
//!   result still comes with its end.

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;

use super::{Chat, Live};
use crate::opencode_server::Response;
use crate::opencode_server_v2::mapper::ShellCall;
use crate::opencode_server_v2::wire::Envelope;
use crate::opencode_server_v2::{RouteV2, ShellId};

/// How often a running call's output is read.
const POLL: Duration = Duration::from_secs(1);

/// Deadline of one read.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// The most output a card shows, in bytes: the newest (OpenCode 1 kept 30,000 characters).
const MAX_OUTPUT: usize = 30_000;

/// Reads at most per poll while more output waits.
const MAX_READS: u32 = 4;

/// Leads output whose start was left out.
const CUT: &str = "...\n";

/// One page of a shell's output (`Shell.Output`).
#[derive(Debug, Deserialize)]
struct Page {
    /// The output from the requested byte on.
    #[serde(default)]
    output: String,
    /// The byte after this page.
    cursor: u64,
    /// The bytes captured so far.
    size: u64,
}

/// Starts reading the shells of the `shell` calls the mapper has just seen (see the module docs).
pub(super) fn follow(chat: &Arc<Chat>, live: &mut Live) {
    for ShellCall { call, shell } in live.mapper.take_shells() {
        match ShellId::parse(&shell) {
            Ok(shell) => {
                tokio::spawn(read_output(chat.clone(), call, shell));
            }
            Err(error) => tracing::debug!(%error, "the output of a shell call is not read"),
        }
    }
}

/// Reads shell `shell`'s output into call `call`'s card while the call runs.
async fn read_output(chat: Arc<Chat>, call: String, shell: ShellId) {
    let mut output = Output::default();

    loop {
        for _ in 0..MAX_READS {
            if !chat.lock().mapper.shell_wanted(&call) {
                return;
            }

            let Some(page) = read(&chat, &shell, output.cursor).await else {
                return;
            };

            let more = output.add(page);
            let mut live = chat.lock();

            for event in live.mapper.shell_output(&call, &output.shown()) {
                chat.shared.emit(event);
            }

            if !more {
                break;
            }
        }

        tokio::time::sleep(POLL).await;
    }
}

/// The page of `shell`'s output from byte `cursor` on; `None` when it cannot be read.
async fn read(chat: &Chat, shell: &ShellId, cursor: u64) -> Option<Page> {
    let route = RouteV2::ShellOutput {
        shell,
        directory: &chat.directory,
        cursor,
        limit: MAX_OUTPUT as u64,
    };

    let page = chat
        .client
        .send_with_timeout(route, None, READ_TIMEOUT)
        .await
        .and_then(Response::error_for_status)
        .and_then(|response| response.json::<Envelope<Page>>());

    match page {
        Ok(envelope) => Some(envelope.data),
        Err(error) => {
            tracing::debug!(%error, "the output of a running shell call could not be read");
            None
        }
    }
}

/// What a call's card shows of its shell's output, and where reading goes on.
#[derive(Debug, Default)]
struct Output {
    /// The newest output, at most [`MAX_OUTPUT`] bytes.
    text: String,
    /// The byte to read next.
    cursor: u64,
    /// Older output was left out.
    cut: bool,
}

impl Output {
    /// Adds `page`; true when more output waits and the read moved on.
    fn add(&mut self, page: Page) -> bool {
        let moved = page.cursor > self.cursor;

        if moved {
            self.text.push_str(&page.output);
            self.cursor = page.cursor;
        }

        let limit = MAX_OUTPUT as u64;
        let waiting = page.size.saturating_sub(self.cursor);

        // More waits than a card shows: only the newest output is read.
        if waiting > limit {
            self.text.clear();
            self.cursor = page.size - limit;
            self.cut = true;
        }

        self.keep_newest();

        moved && waiting > 0
    }

    /// Drops the oldest output past [`MAX_OUTPUT`] bytes, at a character boundary.
    fn keep_newest(&mut self) {
        if self.text.len() <= MAX_OUTPUT {
            return;
        }

        let mut start = self.text.len() - MAX_OUTPUT;

        while !self.text.is_char_boundary(start) {
            start += 1;
        }

        self.text.drain(..start);
        self.cut = true;
    }

    /// The output as the card shows it.
    fn shown(&self) -> String {
        if self.cut {
            return format!("{CUT}{}", self.text);
        }

        self.text.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{CUT, MAX_OUTPUT, Output, Page};

    fn page(output: &str, cursor: u64, size: u64) -> Page {
        Page {
            output: output.to_owned(),
            cursor,
            size,
        }
    }

    #[test]
    fn pages_add_up_and_keep_only_the_newest_output() {
        let mut output = Output::default();

        assert!(!output.add(page("one\n", 4, 4)));
        assert!(output.add(page("two\n", 8, 12)), "more output waits");
        assert!(!output.add(page("two\n", 8, 12)), "a read that did not move on stops");
        assert_eq!(output.shown(), "one\ntwo\n");

        let long = "é".repeat(MAX_OUTPUT);
        output.add(page(&long, 8 + long.len() as u64, 8 + long.len() as u64));
        let shown = output.shown();

        assert!(shown.starts_with(CUT));
        assert!(shown.len() <= MAX_OUTPUT + CUT.len());
        assert!(shown.ends_with('é'));
    }

    #[test]
    fn output_too_old_to_show_is_skipped() {
        let mut output = Output::default();
        let size = 10 * MAX_OUTPUT as u64;

        assert!(output.add(page("start\n", 6, size)));
        assert_eq!(output.cursor, size - MAX_OUTPUT as u64);
        assert_eq!(output.shown(), CUT);
    }
}
