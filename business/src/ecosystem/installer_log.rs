//! The installer log of [`super::installer`]: each output stream split into lines, every line
//! cleaned (terminal escapes stripped, secrets redacted) and passed to the sink while the log
//! budget lasts.

use super::installer::{MAX_LOG_BYTES, MAX_LOG_LINE_BYTES, MAX_LOG_LINES};
use crate::harness::clean;
use crate::model::{InstallLogLine, InstallStream};

/// Raw bytes kept of one line before cleaning (room for escapes the cleaning removes).
const MAX_RAW_LINE_BYTES: usize = 2 * MAX_LOG_LINE_BYTES;

/// The sink plus the log budget.
pub(super) struct Log<F> {
    sink: F,
    /// Bytes passed on, a terminator counted per line (so empty lines use the budget too).
    used: usize,
    /// Lines passed on.
    lines: usize,
    pub truncated: bool,
    pub last_stderr: Option<String>,
}

impl<F: FnMut(InstallLogLine)> Log<F> {
    pub fn new(sink: F) -> Self {
        Self {
            sink,
            used: 0,
            lines: 0,
            truncated: false,
            last_stderr: None,
        }
    }

    /// Cleans one raw line (`clipped` when bytes of it were already dropped) and passes it on
    /// while the budget lasts.
    fn emit(&mut self, stream: InstallStream, raw: &[u8], clipped: bool) {
        let text = String::from_utf8_lossy(raw);
        let mut line = clean(text.trim_end_matches('\r'));

        if clipped || line.len() > MAX_LOG_LINE_BYTES {
            line = clip(&line, MAX_LOG_LINE_BYTES);
            self.truncated = true;
        }

        let cost = line.len() + 1;
        if self.lines >= MAX_LOG_LINES || self.used + cost > MAX_LOG_BYTES {
            self.truncated = true;
            return;
        }

        self.used += cost;
        self.lines += 1;

        if stream == InstallStream::Stderr && !line.trim().is_empty() {
            self.last_stderr = Some(clip(line.trim(), 300));
        }

        (self.sink)(InstallLogLine { stream, line });
    }
}

/// Splits one stream into lines, keeping at most [`MAX_RAW_LINE_BYTES`] of each.
pub(super) struct Splitter {
    stream: InstallStream,
    pending: Vec<u8>,
    clipped: bool,
}

impl Splitter {
    pub fn new(stream: InstallStream) -> Self {
        Self {
            stream,
            pending: Vec::new(),
            clipped: false,
        }
    }

    pub fn push<F: FnMut(InstallLogLine)>(&mut self, mut bytes: &[u8], log: &mut Log<F>) {
        while let Some(end) = bytes.iter().position(|byte| *byte == b'\n') {
            self.keep(&bytes[..end]);
            log.emit(self.stream, &self.pending, self.clipped);
            self.pending.clear();
            self.clipped = false;
            bytes = &bytes[end + 1..];
        }

        self.keep(bytes);
    }

    /// Emits an unterminated last line.
    pub fn finish<F: FnMut(InstallLogLine)>(&mut self, log: &mut Log<F>) {
        if !self.pending.is_empty() || self.clipped {
            log.emit(self.stream, &self.pending, self.clipped);
            self.pending.clear();
            self.clipped = false;
        }
    }

    fn keep(&mut self, bytes: &[u8]) {
        let room = MAX_RAW_LINE_BYTES.saturating_sub(self.pending.len());
        let kept = bytes.len().min(room);

        self.pending.extend_from_slice(&bytes[..kept]);
        self.clipped |= kept < bytes.len();
    }
}

/// `text` cut to at most `max` bytes on a character boundary.
fn clip(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_owned();
    }

    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }

    text[..end].to_owned()
}
