//! Reading a response head: the status line, the headers and how the body is framed.
//!
//! - At most [`MAX_HEAD_BYTES`] are read; a longer head fails.
//! - Lines end in `\r\n` (a bare `\n` is accepted). Folded header lines, informational (`1xx`)
//!   responses and transfer codings other than `chunked` are refused.
//! - Body framing: none for `204` and `304`; `Transfer-Encoding: chunked` before `Content-Length`;
//!   differing `Content-Length` values are refused; otherwise the body runs until the server closes.

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};

use super::chunked::Framing;
use super::{HttpError, MAX_HEAD_BYTES};

/// A parsed response head.
#[derive(Debug)]
pub(super) struct Head {
    pub(super) status: u16,
    pub(super) headers: Vec<(String, String)>,
    pub(super) framing: Framing,
}

/// Reads the head from `reader`, leaving the body's first bytes in it.
pub(super) async fn read_head<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Head, HttpError> {
    let mut limited = reader.take(MAX_HEAD_BYTES as u64);
    let mut lines = Vec::new();

    loop {
        let mut line = Vec::new();
        limited.read_until(b'\n', &mut line).await?;

        if line.last() != Some(&b'\n') {
            if limited.limit() == 0 {
                return Err(HttpError::HeadTooLarge);
            }

            return Err(HttpError::Malformed("the response ended inside its head"));
        }

        line.pop();

        if line.last() == Some(&b'\r') {
            line.pop();
        }

        if line.is_empty() {
            break;
        }

        let line = String::from_utf8(line).map_err(|_| HttpError::Malformed("the response head is not UTF-8"))?;
        lines.push(line);
    }

    parse_head(&lines)
}

fn parse_head(lines: &[String]) -> Result<Head, HttpError> {
    let Some((status_line, header_lines)) = lines.split_first() else {
        return Err(HttpError::Malformed("the response has no status line"));
    };

    let status = parse_status(status_line)?;

    let mut headers = Vec::with_capacity(header_lines.len());

    for line in header_lines {
        headers.push(parse_header(line)?);
    }

    let framing = framing(status, &headers)?;

    Ok(Head {
        status,
        headers,
        framing,
    })
}

fn parse_status(line: &str) -> Result<u16, HttpError> {
    let invalid = HttpError::Malformed("invalid status line");

    let Some(rest) = line.strip_prefix("HTTP/1.") else {
        return Err(invalid);
    };

    let mut parts = rest.splitn(3, ' ');
    let _minor = parts.next();
    let code = parts.next().unwrap_or_default();

    if code.len() != 3 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid);
    }

    let status = code.parse::<u16>().map_err(|_| invalid)?;

    if status < 200 {
        return Err(HttpError::Malformed("unexpected informational response"));
    }

    Ok(status)
}

fn parse_header(line: &str) -> Result<(String, String), HttpError> {
    let Some((name, value)) = line.split_once(':') else {
        return Err(HttpError::Malformed("header line without a colon"));
    };

    let valid_name = !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_graphic());

    if !valid_name {
        return Err(HttpError::Malformed("invalid header name"));
    }

    Ok((name.to_ascii_lowercase(), value.trim().to_owned()))
}

fn framing(status: u16, headers: &[(String, String)]) -> Result<Framing, HttpError> {
    if status == 204 || status == 304 {
        return Ok(Framing::Empty);
    }

    if let Some(last) = values(headers, "transfer-encoding").last() {
        if !last.eq_ignore_ascii_case("chunked") {
            return Err(HttpError::Malformed("unsupported transfer coding"));
        }

        return Ok(Framing::Chunked);
    }

    let mut length = None;

    for value in values(headers, "content-length") {
        let digits_only = value.bytes().all(|byte| byte.is_ascii_digit());
        let parsed = value.parse::<u64>().ok().filter(|_| digits_only);

        let Some(parsed) = parsed else {
            return Err(HttpError::Malformed("invalid Content-Length"));
        };

        if length.is_some_and(|known| known != parsed) {
            return Err(HttpError::Malformed("conflicting Content-Length values"));
        }

        length = Some(parsed);
    }

    Ok(length.map_or(Framing::UntilClose, Framing::Length))
}

/// The comma-separated values of every `name` header, in order.
fn values<'a>(headers: &'a [(String, String)], name: &'a str) -> impl Iterator<Item = &'a str> {
    headers
        .iter()
        .filter(move |(key, _)| key == name)
        .flat_map(|(_, value)| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}
