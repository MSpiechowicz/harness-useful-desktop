//! Bounding free text for the webview: redacted first, then cut.

use super::{mask_secret, redact_text};

/// `text` redacted and cut to `max_chars` characters, the cut marked with an ellipsis.
pub fn bounded_redacted(text: &str, max_chars: usize) -> String {
    bounded_masked(text, None, max_chars)
}

/// [`bounded_redacted`] with `secret` (a password the agent has in its environment) masked
/// first, so a cut can never leave a piece of it.
pub fn bounded_masked(text: &str, secret: Option<&str>, max_chars: usize) -> String {
    let redacted = redact_text(&mask_first(text, secret));

    match redacted.char_indices().nth(max_chars.saturating_sub(1)) {
        Some((cut, _)) if redacted.chars().count() > max_chars => format!("{}…", &redacted[..cut]),
        _ => redacted,
    }
}

/// The end of `text`, redacted: at most `max_lines` lines and `max_bytes` bytes, cut at line
/// boundaries (a single line longer than `max_bytes` loses its start instead). Also returns how
/// many lines `text` had before the cut.
pub fn tail_redacted(text: &str, max_lines: usize, max_bytes: usize) -> (String, u32) {
    tail_masked(text, None, max_lines, max_bytes)
}

/// [`tail_redacted`] with `secret` masked first, so a cut can never leave a piece of it.
pub fn tail_masked(text: &str, secret: Option<&str>, max_lines: usize, max_bytes: usize) -> (String, u32) {
    let redacted = redact_text(&mask_first(text, secret));
    let lines: Vec<&str> = redacted.lines().collect();
    let total = u32::try_from(lines.len()).unwrap_or(u32::MAX);

    let mut kept: Vec<&str> = Vec::new();
    let mut bytes = 0;

    for line in lines.iter().rev().take(max_lines) {
        // The newline that joins this line to the kept ones counts too.
        let cost = line.len() + usize::from(!kept.is_empty());

        if bytes + cost > max_bytes {
            break;
        }

        bytes += cost;
        kept.push(line);
    }

    if kept.is_empty() {
        return (last_line_tail(&lines, max_lines, max_bytes), total);
    }

    kept.reverse();

    (kept.join("\n"), total)
}

/// The end of the last line when even that line alone is over the byte limit.
fn last_line_tail(lines: &[&str], max_lines: usize, max_bytes: usize) -> String {
    let Some(last) = lines.last().filter(|_| max_lines > 0) else {
        return String::new();
    };

    let mut start = last.len().saturating_sub(max_bytes);
    while !last.is_char_boundary(start) {
        start += 1;
    }

    last[start..].to_owned()
}

/// `text` with every occurrence of `secret` masked; `text` itself without one.
fn mask_first(text: &str, secret: Option<&str>) -> String {
    match secret {
        Some(secret) => mask_secret(text, secret, false),
        None => text.to_owned(),
    }
}
