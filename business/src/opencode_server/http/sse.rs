//! The `GET /global/event` stream: Server-Sent Events read from the response body.
//!
//! - Lines end in `\n` (one `\r` before it is dropped); a bare `\r` does not end a line, OpenCode
//!   never writes one.
//! - An event ends at an empty line. Its payload is its `data:` values joined with `\n` (one space
//!   after the colon is dropped); comments (`:` lines) and other fields (`event`, `id`, `retry`)
//!   are ignored, and an event without `data` is not passed on.
//! - An event larger than `max_event_bytes` is skipped and reported as [`SseFault::TooLarge`]; at
//!   most that much of it is ever buffered. A payload that is not UTF-8 is reported as
//!   [`SseFault::NotUtf8`]. Both are recoverable: the stream continues.
//! - The stream ends (`None`) when the server closes the connection, with or without a final
//!   `0` chunk; an unfinished event is dropped. A broken connection or a malformed chunk is
//!   reported once as [`SseFault::Stream`] before the end.
//! - A task reads the connection; dropping the [`EventStream`] stops it and closes the connection.

use tokio::io::{AsyncBufRead, AsyncRead};
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

use super::chunked::BodyReader;
use crate::pi_family::rpc::LineSplitter;

/// Events read ahead of the consumer at most.
const EVENT_QUEUE: usize = 256;

/// Why an event was not passed on, or why the stream ended. Never carries event content.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SseFault {
    /// One event was larger than the limit and was skipped; the stream continues.
    #[error("an OpenCode event larger than {limit} bytes was skipped")]
    TooLarge {
        /// The limit in bytes.
        limit: usize,
    },

    /// One event's payload was not UTF-8 and was skipped; the stream continues.
    #[error("an OpenCode event that is not UTF-8 was skipped")]
    NotUtf8,

    /// Reading failed; the stream ends after this.
    #[error("the OpenCode event stream broke: {0}")]
    Stream(String),
}

/// The events of one `GET /global/event` connection (see the module docs).
#[derive(Debug)]
pub struct EventStream {
    events: mpsc::Receiver<Result<String, SseFault>>,
    reader: AbortHandle,
}

impl EventStream {
    pub(super) fn spawn<R>(body: BodyReader<R>, max_event_bytes: usize) -> Self
    where
        R: AsyncBufRead + Unpin + Send + 'static,
    {
        let (sender, events) = mpsc::channel(EVENT_QUEUE);
        let reader = tokio::spawn(pump(body, max_event_bytes, sender)).abort_handle();

        Self { events, reader }
    }

    /// The next event's `data:` payload, a recoverable fault, or `None` at the end of the stream.
    pub async fn next(&mut self) -> Option<Result<String, SseFault>> {
        self.events.recv().await
    }
}

impl Drop for EventStream {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

async fn pump<R>(mut body: BodyReader<R>, max_event_bytes: usize, sender: mpsc::Sender<Result<String, SseFault>>)
where
    R: AsyncBufRead + Unpin,
{
    let mut events = EventParser::new(LineSplitter::new(&mut body, max_event_bytes), max_event_bytes);

    while let Some(event) = events.next().await {
        if sender.send(event).await.is_err() {
            return;
        }
    }

    drop(events);

    if let Some(fault) = body.fault() {
        let _ = sender.send(Err(SseFault::Stream(fault.to_owned()))).await;
    }
}

/// Groups lines into events.
struct EventParser<R> {
    lines: LineSplitter<R>,
    max_event_bytes: usize,
}

impl<R: AsyncRead + Unpin> EventParser<R> {
    fn new(lines: LineSplitter<R>, max_event_bytes: usize) -> Self {
        Self { lines, max_event_bytes }
    }

    async fn next(&mut self) -> Option<Result<String, SseFault>> {
        let mut data = Vec::new();
        let mut data_lines = 0_usize;
        let mut too_large = false;

        loop {
            let line = self.lines.next_line().await?;

            if line.bytes.is_empty() && !line.overflowed {
                if too_large {
                    return Some(Err(SseFault::TooLarge {
                        limit: self.max_event_bytes,
                    }));
                }

                if data_lines == 0 {
                    continue;
                }

                return Some(String::from_utf8(data).map_err(|_| SseFault::NotUtf8));
            }

            if too_large {
                continue;
            }

            if line.overflowed {
                too_large = true;
                data = Vec::new();
                continue;
            }

            let Some(value) = data_value(&line.bytes) else {
                continue;
            };

            let separator = usize::from(data_lines > 0);

            if data.len() + separator + value.len() > self.max_event_bytes {
                too_large = true;
                data = Vec::new();
                continue;
            }

            if separator > 0 {
                data.push(b'\n');
            }

            data.extend_from_slice(value);
            data_lines += 1;
        }
    }
}

/// The value of a `data` field line; `None` for comments and other fields.
fn data_value(line: &[u8]) -> Option<&[u8]> {
    let (field, value) = match line.iter().position(|byte| *byte == b':') {
        Some(colon) => (&line[..colon], &line[colon + 1..]),
        None => (line, &[][..]),
    };

    if field != b"data" {
        return None;
    }

    Some(value.strip_prefix(b" ").unwrap_or(value))
}
