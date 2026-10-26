//! A one-connection stand-in for `opencode serve`: it answers with recorded or crafted bytes,
//! written in small pieces so chunk and line boundaries fall across TCP reads, and hands back the
//! request it received.
//!
//! Each test crate uses a different subset, hence the `dead_code` allowance.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use harness_business::opencode_server::{ClientOptions, OpenCodeClient, ServerPassword};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

/// The bytes of `tests/fixtures/opencode/server/<name>`.
pub fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/opencode/server")
        .join(name);

    std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

/// How the stand-in answers.
#[derive(Debug, Clone, Copy)]
pub struct Answer {
    /// Bytes per write; the sizes cycle from 1 up to this so boundaries move around.
    pub max_piece: usize,
    /// Keep the connection open after the response (the client must close it).
    pub hold: bool,
}

impl Default for Answer {
    fn default() -> Self {
        Self {
            max_piece: 13,
            hold: false,
        }
    }
}

/// A started stand-in.
pub struct Server {
    /// Where it listens.
    pub addr: SocketAddr,
    request: JoinHandle<Vec<u8>>,
}

impl Server {
    /// The request bytes it received (head and body), once the response was written.
    pub async fn request(self) -> String {
        let bytes = self.request.await.expect("stand-in server task");
        String::from_utf8(bytes).expect("request is UTF-8")
    }
}

/// Answers one connection with `response`.
pub async fn serve(response: Vec<u8>, answer: Answer) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind 127.0.0.1:0");
    let addr = listener.local_addr().expect("local address");

    let request = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        stream.set_nodelay(true).expect("nodelay");

        let request = read_request(&mut stream).await;
        write_in_pieces(&mut stream, &response, answer.max_piece).await;

        if answer.hold {
            let mut rest = Vec::new();
            let _ = stream.read_to_end(&mut rest).await;
        }

        request
    });

    Server { addr, request }
}

/// Accepts one connection, reads the request and never answers.
pub async fn serve_silence() -> Server {
    serve(
        Vec::new(),
        Answer {
            max_piece: 1,
            hold: true,
        },
    )
    .await
}

/// A client of `server` with a fresh random password.
pub fn connect(server: &Server, options: ClientOptions) -> (OpenCodeClient, ServerPassword) {
    let password = ServerPassword::random();
    let client = OpenCodeClient::with_options(server.addr, password.clone(), options).expect("loopback client");

    (client, password)
}

/// `body` as a chunked response with `Content-Type: text/event-stream`, cut into chunks of
/// `chunk` bytes; `finish` adds the final `0` chunk.
pub fn chunked_response(body: &[u8], chunk: usize, finish: bool) -> Vec<u8> {
    let mut response =
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();

    for piece in body.chunks(chunk) {
        response.extend_from_slice(format!("{:x}\r\n", piece.len()).as_bytes());
        response.extend_from_slice(piece);
        response.extend_from_slice(b"\r\n");
    }

    if finish {
        response.extend_from_slice(b"0\r\n\r\n");
    }

    response
}

/// A `200` JSON response with `Content-Length`.
pub fn json_response(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

async fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];

    loop {
        if let Some(end) = find(&request, b"\r\n\r\n") {
            let body_length = content_length(&request[..end]);

            if request.len() >= end + 4 + body_length {
                return request;
            }
        }

        let read = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buffer))
            .await
            .expect("request within 5 s")
            .expect("read request");

        if read == 0 {
            return request;
        }

        request.extend_from_slice(&buffer[..read]);
    }
}

async fn write_in_pieces(stream: &mut TcpStream, response: &[u8], max_piece: usize) {
    let mut offset = 0;
    let mut size = 1;

    while offset < response.len() {
        let end = (offset + size).min(response.len());

        if stream.write_all(&response[offset..end]).await.is_err() {
            return;
        }

        offset = end;
        size = size % max_piece.max(1) + 1;
        tokio::task::yield_now().await;
    }

    let _ = stream.flush().await;
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

fn content_length(head: &[u8]) -> usize {
    String::from_utf8_lossy(head)
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())
                .flatten()
        })
        .unwrap_or(0)
}
