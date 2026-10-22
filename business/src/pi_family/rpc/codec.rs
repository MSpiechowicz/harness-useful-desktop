//! Newline-delimited JSON framing of the agent RPC streams.
//!
//! - Frames are split on `\n` only: U+2028 and U+2029 are valid inside JSON strings and never end
//!   a frame. One trailing `\r` is stripped; blank lines are skipped.
//! - A frame longer than [`MAX_FRAME_BYTES`] becomes a [`FrameFault`] and reading resumes at the
//!   next `\n`; at most the limit is ever buffered.
//! - Invalid JSON, or JSON that is not an object, becomes a [`FrameFault`]; reading never stops.
//! - Encoding is `serde_json` plus `\n` (JSON escapes every raw newline inside strings).

use std::sync::LazyLock;

use regex::bytes::Regex;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

/// Largest accepted frame (4 MiB, excluding the newline). OMP caps its own frames at 1 MiB.
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;

/// How much of a faulty frame is searched for its `type` and `id`.
const FAULT_PREFIX_BYTES: usize = 1024;

static TYPE_FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""type"\s*:\s*"([A-Za-z0-9_:.-]{1,64})""#).expect("static regex is valid"));

static ID_FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""id"\s*:\s*"([A-Za-z0-9_:.-]{1,128})""#).expect("static regex is valid"));

static RPC_ID_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""id"\s*:\s*(-?[0-9]{1,18}|"[A-Za-z0-9_:.-]{1,128}")"#).expect("static regex is valid")
});

static METHOD_FIELD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""method"\s*:\s*"([A-Za-z0-9_/:.-]{1,128})""#).expect("static regex is valid"));

/// The `id` of the `item` a notification is about (`{"params": {"item": {"type": …, "id": …`).
static ITEM_ID_FIELD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""item"\s*:\s*\{[^{}\[\]]{0,256}?"id"\s*:\s*"([A-Za-z0-9_:.-]{1,128})""#)
        .expect("static regex is valid")
});

/// One decoded frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    /// A JSON object.
    Json(Value),
    /// A frame that could not be used; the stream continues with the next one.
    Fault(FrameFault),
}

/// Why a frame was dropped. Never carries frame content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameFault {
    /// The frame's `type`, when it could be read from its first bytes.
    pub original_type: Option<String>,
    /// The `id` of a dropped `response` frame, so its request can fail at once.
    pub response_id: Option<String>,
    /// A JSON-RPC frame's own `id` as written (`7`, `"req-7"`), read before its `params`, `result`
    /// or `error`.
    pub rpc_id: Option<String>,
    /// A JSON-RPC frame's `method`, read the same way.
    pub method: Option<String>,
    /// The `id` of the `item` a notification is about, when its first bytes name it.
    pub item_id: Option<String>,
    /// What went wrong.
    pub error: String,
}

/// Encodes one frame: compact JSON followed by `\n`.
pub fn encode_frame(frame: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(frame).unwrap_or_else(|_| b"{}".to_vec());
    bytes.push(b'\n');
    bytes
}

/// One line read by [`LineSplitter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The first `max` bytes of the line, without the `\n` and one trailing `\r`.
    pub bytes: Vec<u8>,
    /// True when the line was longer than `max` and the rest was discarded.
    pub overflowed: bool,
}

/// Splits a byte stream on `\n`, keeping at most `max` bytes of each line in memory.
pub struct LineSplitter<R> {
    reader: BufReader<R>,
    max: usize,
    line: Vec<u8>,
    overflowed: bool,
}

impl<R: AsyncRead + Unpin> LineSplitter<R> {
    /// A splitter over `reader` that keeps at most `max` bytes per line.
    pub fn new(reader: R, max: usize) -> Self {
        Self {
            reader: BufReader::new(reader),
            max,
            line: Vec::new(),
            overflowed: false,
        }
    }

    /// The next line; `None` at end of stream (or on a read error). A final line without `\n` is
    /// still returned.
    pub async fn next_line(&mut self) -> Option<Line> {
        loop {
            let (consumed, complete) = {
                let Ok(chunk) = self.reader.fill_buf().await else {
                    return self.finish();
                };

                if chunk.is_empty() {
                    return self.finish();
                }

                match chunk.iter().position(|byte| *byte == b'\n') {
                    Some(end) => {
                        Self::keep(&mut self.line, &mut self.overflowed, self.max, &chunk[..end]);
                        (end + 1, true)
                    }
                    None => {
                        Self::keep(&mut self.line, &mut self.overflowed, self.max, chunk);
                        (chunk.len(), false)
                    }
                }
            };

            self.reader.consume(consumed);

            if complete {
                return Some(self.take());
            }
        }
    }

    fn keep(line: &mut Vec<u8>, overflowed: &mut bool, max: usize, bytes: &[u8]) {
        let room = max.saturating_sub(line.len());

        if bytes.len() > room {
            *overflowed = true;
        }

        line.extend_from_slice(&bytes[..bytes.len().min(room)]);
    }

    fn take(&mut self) -> Line {
        let mut bytes = std::mem::take(&mut self.line);
        let overflowed = std::mem::replace(&mut self.overflowed, false);

        if !overflowed && bytes.last() == Some(&b'\r') {
            bytes.pop();
        }

        Line { bytes, overflowed }
    }

    fn finish(&mut self) -> Option<Line> {
        if self.line.is_empty() && !self.overflowed {
            return None;
        }

        Some(self.take())
    }
}

/// Reads [`Frame`]s from an agent's stdout.
pub struct FrameReader<R> {
    lines: LineSplitter<R>,
}

impl<R: AsyncRead + Unpin> FrameReader<R> {
    /// A reader with the default [`MAX_FRAME_BYTES`] limit.
    pub fn new(reader: R) -> Self {
        Self::with_limit(reader, MAX_FRAME_BYTES)
    }

    /// A reader with a custom frame limit (tests).
    pub fn with_limit(reader: R, max_frame_bytes: usize) -> Self {
        Self {
            lines: LineSplitter::new(reader, max_frame_bytes),
        }
    }

    /// The next frame; `None` at end of stream. Blank lines are skipped.
    pub async fn next_frame(&mut self) -> Option<Frame> {
        loop {
            let line = self.lines.next_line().await?;

            if line.overflowed {
                return Some(Frame::Fault(fault(&line.bytes, "frame exceeds the 4 MiB limit")));
            }

            if line.bytes.iter().all(u8::is_ascii_whitespace) {
                continue;
            }

            return Some(parse_frame(&line.bytes));
        }
    }
}

/// Parses one frame's bytes (without the newline).
pub fn parse_frame(bytes: &[u8]) -> Frame {
    match serde_json::from_slice::<Value>(bytes) {
        Ok(value @ Value::Object(_)) => Frame::Json(value),
        Ok(_) => Frame::Fault(fault(bytes, "frame is not a JSON object")),
        Err(error) => Frame::Fault(fault(bytes, &format!("invalid JSON at column {}", error.column()))),
    }
}

fn fault(bytes: &[u8], error: &str) -> FrameFault {
    let prefix = &bytes[..bytes.len().min(FAULT_PREFIX_BYTES)];
    let original_type = capture(&TYPE_FIELD, prefix);

    let response_id = match original_type.as_deref() {
        Some("response") => capture(&ID_FIELD, prefix),
        _ => None,
    };

    let head = rpc_head(prefix);

    FrameFault {
        original_type,
        response_id,
        rpc_id: capture(&RPC_ID_FIELD, head),
        method: capture(&METHOD_FIELD, head),
        item_id: capture(&ITEM_ID_FIELD, prefix),
        error: error.to_owned(),
    }
}

/// The top level of a JSON-RPC frame's first bytes, where its own `id` and `method` are (Codex's
/// frames have no `type`): up to its `params`, `result` or `error`, or the first nested value.
fn rpc_head(prefix: &[u8]) -> &[u8] {
    let body = prefix.trim_ascii_start();
    let body = body.strip_prefix(b"{").unwrap_or(body);

    let nested = body
        .iter()
        .position(|byte| matches!(byte, b'{' | b'['))
        .unwrap_or(body.len());
    let payload = [&b"\"params\""[..], b"\"result\"", b"\"error\""]
        .iter()
        .filter_map(|key| body.windows(key.len()).position(|window| window == *key))
        .min()
        .unwrap_or(body.len());

    &body[..nested.min(payload)]
}

fn capture(pattern: &Regex, haystack: &[u8]) -> Option<String> {
    let captures = pattern.captures(haystack)?;
    let matched = captures.get(1)?;

    String::from_utf8(matched.as_bytes().to_vec()).ok()
}
