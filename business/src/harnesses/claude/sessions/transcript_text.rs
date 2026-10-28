//! The plain text of the tagged user lines of a Claude Code session: shell runs, command output and
//! the compaction summary.

use super::entries::tagged;
use crate::pi_family::normalize::plain_text;

/// `stdout` then `stderr` of a shell run, each without terminal escapes, joined by a line break.
pub(super) fn shell_output(raw: &str) -> String {
    let parts: Vec<String> = ["bash-stdout", "bash-stderr"]
        .iter()
        .filter_map(|tag| tagged(raw, tag))
        .map(|part| plain_text(part.trim_end()))
        .filter(|part| !part.is_empty())
        .collect();

    parts.join("\n")
}

/// The plain text of a command's output.
pub(super) fn local_output(raw: &str) -> String {
    let inner = tagged(raw, "local-command-stdout")
        .or_else(|| tagged(raw, "local-command-stderr"))
        .unwrap_or(raw);

    plain_text(inner.trim())
}

/// The summary of a compaction message: the part after its `Summary:` line when Claude Code's
/// framing is there, else the whole text.
pub(super) fn compaction_summary(raw: &str) -> String {
    const START: &str = "\nSummary:\n";
    const END: &str = "\n\nIf you need specific details from before compaction";

    let Some(start) = raw.find(START).map(|start| start + START.len()) else {
        return raw.trim().to_owned();
    };

    let rest = &raw[start..];
    let summary = rest.find(END).map_or(rest, |end| &rest[..end]);

    summary.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_drops_claude_codes_framing() {
        let raw = "This session is being continued.\n\n<tools-used-before-summary></tools-used-before-summary>\nSummary:\npong\nping\n\nIf you need specific details from before compaction, read the transcript.";
        assert_eq!(compaction_summary(raw), "pong\nping");
        assert_eq!(compaction_summary("  just a summary "), "just a summary");
    }

    #[test]
    fn shell_output_joins_stdout_and_stderr() {
        let raw = "<bash-stdout>out\n</bash-stdout><bash-stderr>\u{1b}[31merr\u{1b}[0m</bash-stderr>";
        assert_eq!(shell_output(raw), "out\nerr");
        assert_eq!(
            shell_output("<bash-stdout></bash-stdout><bash-stderr></bash-stderr>"),
            ""
        );
    }
}
