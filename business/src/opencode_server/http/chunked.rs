//! Response bodies: `Content-Length`, `Transfer-Encoding: chunked` and until-close framing.
//!
//! [`ChunkedReader`] decodes a chunked body as a byte stream:
//!
//! - A chunk-size line may take [`MAX_LINE_BYTES`] (extensions are ignored), the trailer section
//!   [`MAX_TRAILER_BYTES`]; sizes are 1–15 hex digits.
//! - Chunk data must be followed by an empty line; anything else is an `InvalidData` error.
//! - The server closing the connection between chunks ends the body without an error (OpenCode's
//!   event stream ends that way when the server stops); [`ChunkedReader::is_complete`] tells it
//!   apart from a final `0` chunk. Closing inside a chunk is an `UnexpectedEof` error.
//!
//! [`BodyReader`] picks the decoder for a [`Framing`] and remembers the first read error, so a
//! reader that swallows errors (the SSE line splitter) can still report why the stream ended.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, ReadBuf, Take};

/// Longest chunk-size line, extensions included.
pub(super) const MAX_LINE_BYTES: usize = 4096;

/// Largest trailer section.
pub(super) const MAX_TRAILER_BYTES: usize = 64 * 1024;

/// Most hex digits of a chunk size (keeps the size within `u64`).
const MAX_SIZE_DIGITS: usize = 15;

/// How a response body is delimited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Framing {
    /// No body (`204`, `304`).
    Empty,
    /// Exactly this many bytes.
    Length(u64),
    /// `Transfer-Encoding: chunked`.
    Chunked,
    /// Until the server closes the connection.
    UntilClose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Expecting a chunk-size line.
    Size,
    /// Inside chunk data, with this many bytes left.
    Data(u64),
    /// Expecting the empty line after chunk data.
    DataEnd,
    /// Reading trailer lines after the `0` chunk.
    Trailers,
    /// The final empty line was read.
    Done,
    /// The server closed the connection between chunks.
    Closed,
}

/// Decodes a chunked body (see the module docs).
pub(super) struct ChunkedReader<R> {
    inner: R,
    state: State,
    line: Vec<u8>,
    trailer_bytes: usize,
}

impl<R: AsyncBufRead + Unpin> ChunkedReader<R> {
    pub(super) fn new(inner: R) -> Self {
        Self {
            inner,
            state: State::Size,
            line: Vec::new(),
            trailer_bytes: 0,
        }
    }

    /// True once the final `0` chunk and its trailers were read.
    pub(super) fn is_complete(&self) -> bool {
        self.state == State::Done
    }

    /// The next `\n`-terminated line without its line end; `None` when the connection closed
    /// before the line started.
    fn poll_line(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<Option<Vec<u8>>>> {
        loop {
            let available = ready!(Pin::new(&mut self.inner).poll_fill_buf(cx))?;

            if available.is_empty() {
                if self.line.is_empty() {
                    return Poll::Ready(Ok(None));
                }

                return Poll::Ready(Err(truncated()));
            }

            let (used, complete) = match available.iter().position(|byte| *byte == b'\n') {
                Some(end) => (end + 1, true),
                None => (available.len(), false),
            };

            if self.line.len() + used > MAX_LINE_BYTES {
                return Poll::Ready(Err(malformed("a chunk line is too long")));
            }

            self.line.extend_from_slice(&available[..used]);
            Pin::new(&mut self.inner).consume(used);

            if complete {
                let mut line = std::mem::take(&mut self.line);
                line.pop();

                if line.last() == Some(&b'\r') {
                    line.pop();
                }

                return Poll::Ready(Ok(Some(line)));
            }
        }
    }

    /// Moves past one framing line (`None`: the connection closed before it).
    fn advance(&mut self, line: Option<Vec<u8>>) -> io::Result<()> {
        let Some(line) = line else {
            self.state = State::Closed;
            return Ok(());
        };

        self.state = match self.state {
            State::Size => match chunk_size(&line)? {
                0 => State::Trailers,
                size => State::Data(size),
            },
            State::DataEnd if line.is_empty() => State::Size,
            State::DataEnd => return Err(malformed("chunk data is longer than its size")),
            State::Trailers if line.is_empty() => State::Done,
            State::Trailers => {
                self.trailer_bytes += line.len();

                if self.trailer_bytes > MAX_TRAILER_BYTES {
                    return Err(malformed("the chunk trailers are too large"));
                }

                State::Trailers
            }
            state => state,
        };

        Ok(())
    }

    fn poll_data(&mut self, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>, remaining: u64) -> Poll<io::Result<()>> {
        let available = ready!(Pin::new(&mut self.inner).poll_fill_buf(cx))?;

        if available.is_empty() {
            return Poll::Ready(Err(truncated()));
        }

        let wanted = usize::try_from(remaining).unwrap_or(usize::MAX);
        let count = available.len().min(buf.remaining()).min(wanted);

        buf.put_slice(&available[..count]);
        Pin::new(&mut self.inner).consume(count);

        let left = remaining - count as u64;
        self.state = if left == 0 { State::DataEnd } else { State::Data(left) };

        Poll::Ready(Ok(()))
    }
}

impl<R: AsyncBufRead + Unpin> AsyncRead for ChunkedReader<R> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }

        loop {
            match this.state {
                State::Data(remaining) => return this.poll_data(cx, buf, remaining),
                State::Done | State::Closed => return Poll::Ready(Ok(())),
                State::Size | State::DataEnd | State::Trailers => {
                    let line = ready!(this.poll_line(cx))?;
                    this.advance(line)?;
                }
            }
        }
    }
}

fn chunk_size(line: &[u8]) -> io::Result<u64> {
    let digits = line.split(|byte| *byte == b';').next().unwrap_or_default().trim_ascii();

    let valid = !digits.is_empty() && digits.len() <= MAX_SIZE_DIGITS && digits.iter().all(u8::is_ascii_hexdigit);

    if !valid {
        return Err(malformed("invalid chunk size"));
    }

    let text = String::from_utf8_lossy(digits);

    u64::from_str_radix(&text, 16).map_err(|_| malformed("invalid chunk size"))
}

fn malformed(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn truncated() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "the response ended inside a chunk")
}

enum Body<R> {
    Empty,
    Length(Take<R>),
    Chunked(ChunkedReader<R>),
    UntilClose(R),
}

/// A response body as a byte stream (see the module docs).
pub(super) struct BodyReader<R> {
    body: Body<R>,
    fault: Option<String>,
}

impl<R: AsyncBufRead + Unpin> BodyReader<R> {
    pub(super) fn new(reader: R, framing: Framing) -> Self {
        let body = match framing {
            Framing::Empty => Body::Empty,
            Framing::Length(length) => Body::Length(reader.take(length)),
            Framing::Chunked => Body::Chunked(ChunkedReader::new(reader)),
            Framing::UntilClose => Body::UntilClose(reader),
        };

        Self { body, fault: None }
    }

    /// False when the server closed the connection before the body's declared end.
    pub(super) fn is_complete(&self) -> bool {
        match &self.body {
            Body::Empty | Body::UntilClose(_) => true,
            Body::Length(reader) => reader.limit() == 0,
            Body::Chunked(reader) => reader.is_complete(),
        }
    }

    /// The first read error, if reading failed.
    pub(super) fn fault(&self) -> Option<&str> {
        self.fault.as_deref()
    }
}

impl<R: AsyncBufRead + Unpin> AsyncRead for BodyReader<R> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        let result = match &mut this.body {
            Body::Empty => return Poll::Ready(Ok(())),
            Body::Length(reader) => ready!(Pin::new(reader).poll_read(cx, buf)),
            Body::Chunked(reader) => ready!(Pin::new(reader).poll_read(cx, buf)),
            Body::UntilClose(reader) => ready!(Pin::new(reader).poll_read(cx, buf)),
        };

        if let Err(error) = &result
            && this.fault.is_none()
        {
            this.fault = Some(error.to_string());
        }

        Poll::Ready(result)
    }
}
