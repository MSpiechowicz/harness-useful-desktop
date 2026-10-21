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
//! - Whole lines after a cut: the line the cut fell in shows from the next line break on (`\n` or
//!   `\r`), so a cut within the server password (masked only where it shows whole) shows none of
//!   it. Output without a line break after the cut shows only [`CUT`] until one comes.
//! - Whole characters: OpenCode decodes each page on its own (`toString("utf8")`), so a character
//!   split by a page's end comes as one U+FFFD, and each byte of one split by its start (a skip
//!   lands anywhere) as one more. A trailing U+FFFD that stands for at most three of the page's
//!   bytes (its cursor tells) is read again with the next page, so a U+FFFD of the output itself
//!   waits for the output that follows; leading ones after a skip are dropped.
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

/// What OpenCode decodes a partial character as.
const REPLACEMENT: char = '\u{FFFD}';

/// The most bytes of a partial character (a whole one has at most four).
const MAX_PARTIAL: u64 = 3;

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
    /// The read skipped to a byte that may be within a character.
    mid_character: bool,
    /// The text may start within a line (older output was left out): it is dropped up to the next
    /// line break.
    mid_line: bool,
}

impl Output {
    /// Adds `page`; true when more output waits and the read moved on.
    fn add(&mut self, page: Page) -> bool {
        let read = page.cursor.saturating_sub(self.cursor);
        let moved = read > 0;
        let waiting = page.size.saturating_sub(page.cursor);

        if moved {
            let mid_character = std::mem::take(&mut self.mid_character);
            let (text, held) = whole_characters(&page.output, read, mid_character);

            self.text.push_str(text);
            self.cursor = page.cursor - held;
        }

        let limit = MAX_OUTPUT as u64;

        // More waits than a card shows: only the newest output is read.
        if page.size.saturating_sub(self.cursor) > limit {
            self.text.clear();
            self.cursor = page.size - limit;
            self.cut = true;
            self.mid_character = true;
            self.mid_line = true;
        }

        self.keep_newest();
        self.drop_partial_line();

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
        self.mid_line = true;
    }

    /// Drops the start of a line the cut fell in, up to and with the next line break; all of the
    /// text while it has none.
    fn drop_partial_line(&mut self) {
        if !self.mid_line {
            return;
        }

        let Some(at) = self.text.find(['\n', '\r']) else {
            self.text.clear();
            return;
        };

        let end = match self.text[at..].starts_with("\r\n") {
            true => at + 2,
            false => at + 1,
        };

        self.text.drain(..end);
        self.mid_line = false;
    }

    /// The output as the card shows it.
    fn shown(&self) -> String {
        if self.cut {
            return format!("{CUT}{}", self.text);
        }

        self.text.clone()
    }
}

/// The whole characters of `output`, a page of `read` bytes, and how many bytes at its end to read
/// again (see the module docs): `mid_character` when the page may start within a character.
fn whole_characters(output: &str, read: u64, mid_character: bool) -> (&str, u64) {
    let mut text = output;
    let mut dropped = 0;

    while mid_character && dropped < MAX_PARTIAL {
        let Some(rest) = text.strip_prefix(REPLACEMENT) else {
            break;
        };

        text = rest;
        dropped += 1;
    }

    let Some(body) = text.strip_suffix(REPLACEMENT) else {
        return (text, 0);
    };

    let decoded = dropped + body.len() as u64;
    let partial = read.saturating_sub(decoded);

    if decoded < read && partial <= MAX_PARTIAL {
        return (body, partial);
    }

    (text, 0)
}

#[cfg(test)]
mod tests {
    use super::{CUT, MAX_OUTPUT, Output, Page};
    use crate::security::mask_secret;

    /// `bytes` as OpenCode decodes a page of them.
    fn decoded(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

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

        let long = "é\n".repeat(MAX_OUTPUT);
        output.add(page(&long, 8 + long.len() as u64, 8 + long.len() as u64));
        let shown = output.shown();

        assert!(shown.starts_with(&format!("{CUT}é\n")), "whole lines after the cut");
        assert!(shown.len() <= MAX_OUTPUT + CUT.len());
        assert!(shown.ends_with("é\n"));
    }

    #[test]
    fn output_too_old_to_show_is_skipped() {
        let mut output = Output::default();
        let size = 10 * MAX_OUTPUT as u64;

        assert!(output.add(page("start\n", 6, size)));
        assert_eq!(output.cursor, size - MAX_OUTPUT as u64);
        assert_eq!(output.shown(), CUT);
    }

    #[test]
    fn a_character_split_by_a_page_end_shows_whole_once_the_next_page_has_it() {
        let bytes = "a€b😀c".as_bytes();
        let mut output = Output::default();

        // "a" and two of the euro sign's three bytes.
        assert!(output.add(page(&decoded(&bytes[..3]), 3, bytes.len() as u64)));
        assert_eq!(output.shown(), "a");
        assert_eq!(output.cursor, 1, "the partial character is read again");

        // From the euro sign on, the page ending within the emoji.
        let end = 7;
        assert!(output.add(page(&decoded(&bytes[1..end]), end as u64, bytes.len() as u64)));
        assert_eq!(output.shown(), "a€b");

        let size = bytes.len() as u64;
        assert!(!output.add(page(&decoded(&bytes[output.cursor as usize..]), size, size)));
        assert_eq!(output.shown(), "a€b😀c");
    }

    #[test]
    fn a_page_of_only_a_partial_character_waits_for_the_rest() {
        let bytes = "é".as_bytes();
        let mut output = Output::default();

        output.add(page(&decoded(&bytes[..1]), 1, 1));
        assert_eq!(output.shown(), "");
        assert_eq!(output.cursor, 0);

        output.add(page(&decoded(bytes), 2, 2));
        assert_eq!(output.shown(), "é");
    }

    #[test]
    fn a_replacement_character_of_the_output_itself_shows_once_more_follows() {
        let text = "bad \u{FFFD}";
        let size = text.len() as u64;
        let mut output = Output::default();

        // A whole U+FFFD at a page's end reads as a partial character: it waits.
        output.add(page(text, size, size));
        assert_eq!(output.shown(), "bad ");

        let more = format!("{text}!");
        let size = more.len() as u64;
        output.add(page(&more[output.cursor as usize..], size, size));
        assert_eq!(output.shown(), more);
    }

    #[test]
    fn a_skip_into_a_character_drops_its_remaining_bytes() {
        // The newest MAX_OUTPUT bytes start two bytes into the first euro sign.
        let all = format!(
            "{}{}\n€abcd",
            "x".repeat(2 * MAX_OUTPUT),
            "€".repeat(MAX_OUTPUT / 3 - 2)
        );
        let size = all.len() as u64;
        let mut output = Output::default();

        assert!(output.add(page("x", 1, size)));
        let start = output.cursor as usize;
        assert!(!all.is_char_boundary(start));

        output.add(page(&decoded(&all.as_bytes()[start..]), size, size));

        let shown = output.shown();
        assert!(!shown.contains('\u{FFFD}'), "no partial character shows");
        assert_eq!(shown, format!("{CUT}€abcd"), "the next whole line");
    }

    /// The fake server password of the cut tests.
    const SECRET: &str = "0123456789abcdef0123456789abcdef";

    /// `text` with [`SECRET`] masked as the chat masks every event.
    fn masked(text: &str) -> String {
        mask_secret(text, SECRET, false)
    }

    #[test]
    fn a_cut_within_the_password_shows_none_of_it() {
        let half = SECRET.len() / 2;
        let rest = "rest\n";
        // Past MAX_OUTPUT by half the password: the newest bytes start within it.
        let filler = "y".repeat(MAX_OUTPUT - half - 1 - rest.len());
        let text = format!("{SECRET}\n{rest}{filler}");
        let size = text.len() as u64;
        let mut output = Output::default();

        output.add(page(&text, size, size));

        let shown = masked(&output.shown());
        assert!(!shown.contains(&SECRET[half..]), "no piece of the password shows");
        assert_eq!(shown, format!("{CUT}{rest}{filler}"));
    }

    #[test]
    fn a_skip_into_the_password_shows_none_of_it() {
        let half = SECRET.len() / 2;
        let rest = "rest\r\n";
        let filler = "y".repeat(MAX_OUTPUT - half - rest.len());
        let all = format!("{}{SECRET}{rest}{filler}", "x".repeat(2 * MAX_OUTPUT));
        let size = all.len() as u64;
        let mut output = Output::default();

        assert!(output.add(page("x", 1, size)));
        let start = output.cursor as usize;
        assert_eq!(
            &all[start..start + SECRET.len() - half],
            &SECRET[half..],
            "the skip lands within it"
        );

        output.add(page(&all[start..], size, size));

        let shown = masked(&output.shown());
        assert!(!shown.contains(&SECRET[half..]), "no piece of the password shows");
        assert_eq!(shown, format!("{CUT}{filler}"));
    }

    #[test]
    fn a_cut_line_without_a_break_shows_once_the_next_line_starts() {
        let size = 2 * MAX_OUTPUT as u64;
        let mut output = Output::default();

        output.add(page(&"z".repeat(2 * MAX_OUTPUT), size, size));
        assert_eq!(output.shown(), CUT, "only a partial line so far");

        let more = "z\nnext\n";
        let size = size + more.len() as u64;
        output.add(page(more, size, size));
        assert_eq!(output.shown(), format!("{CUT}next\n"));
    }
}
