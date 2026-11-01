//! Reading the embedder's stdout and stderr for [`super::memory_embedder_supervisor`].

use std::io;

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt};

/// The longest stdout line read; the rest of a longer line is dropped.
const MAX_LINE_BYTES: u64 = 64 * 1024;

/// Reads and drops `stream` so a full pipe never blocks the embedder. Its text is not kept: it can
/// quote the data the embedder works on.
pub(super) async fn discard(mut stream: Box<dyn AsyncRead + Send + Unpin>) {
    let mut buffer = [0_u8; 4096];

    while matches!(stream.read(&mut buffer).await, Ok(read) if read > 0) {}
}

/// The next line of `reader` (at most [`MAX_LINE_BYTES`], the rest of a longer one is dropped);
/// `None` at the end of the stream.
pub(super) async fn next_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    buffer: &mut Vec<u8>,
) -> io::Result<Option<String>> {
    loop {
        buffer.clear();
        let read = (&mut *reader).take(MAX_LINE_BYTES).read_until(b'\n', buffer).await?;

        if read == 0 {
            return Ok(None);
        }

        if buffer.ends_with(b"\n") || (read as u64) < MAX_LINE_BYTES {
            return Ok(Some(String::from_utf8_lossy(buffer).into_owned()));
        }

        skip_rest_of_line(reader, buffer).await?;
    }
}

/// Reads up to the end of the line a read stopped in the middle of.
async fn skip_rest_of_line<R: AsyncBufRead + Unpin>(reader: &mut R, buffer: &mut Vec<u8>) -> io::Result<()> {
    loop {
        buffer.clear();
        let read = (&mut *reader).take(MAX_LINE_BYTES).read_until(b'\n', buffer).await?;

        if read == 0 || buffer.ends_with(b"\n") {
            return Ok(());
        }
    }
}
