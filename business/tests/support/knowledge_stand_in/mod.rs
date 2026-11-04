//! An installed `harness-knowledge` under a temporary home and the doubles that stand in for it,
//! for the tests of what the app reads from the server (`features`, `docs`, `held`, `check`,
//! `docs import --ndjson`). The binary in the home is a placeholder that is never run: a rule-based
//! runner answers its commands (see [`cli`]) and a duplex spawner plays its streamed import (see
//! [`play_import`]), so nothing is forked.
//! Each test binary uses only some of these helpers.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use harness_business::env::Env;
use harness_business::process::{ExecOutput, ExecSpec, ExitInfo};
use harness_business::testing::FakePeer;
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;

pub mod review;

use crate::memory_cli_support::{installed_binary, installed_binary_at};
use crate::setup_support::play;
use crate::team_support::{Cli, exits};

/// What a current server lists.
pub const ALL_FEATURES: &str = "docs\nimport-ndjson\nembedder\nsearch\nrecall\nheld\nmcp\nrouting\ndoc-matches\noffline-fallback\ndocs-useful\ndocs-similar\ndocs-authors\ndocs-drafts\ndocs-approval\n";

/// What a server lists with drafts but before team approval: no `docs-approval`.
pub const BEFORE_APPROVAL: &str = "docs\nimport-ndjson\nembedder\nsearch\nrecall\nheld\nmcp\nrouting\ndoc-matches\noffline-fallback\ndocs-useful\ndocs-similar\ndocs-authors\ndocs-drafts\n";

/// What a server lists before drafts and team approval: no `docs-drafts` and no `docs-approval`.
pub const BEFORE_DRAFTS: &str = "docs\nimport-ndjson\nembedder\nsearch\nrecall\nheld\nmcp\nrouting\ndoc-matches\noffline-fallback\ndocs-useful\ndocs-similar\ndocs-authors\n";

/// A repository key the tests link documents to.
pub const REPO: &str = "git:github.com/acme/widget";

/// The `docs get` object of document `id`, as the server prints it.
pub fn document(id: i64, title: &str) -> Value {
    json!({
        "id": id,
        "title": title,
        "sourceName": format!("{title}.md"),
        "sourceKey": format!("/docs/{title}.md"),
        "format": "markdown",
        "contentSha256": "a".repeat(64),
        "fileSha256": "b".repeat(64),
        "bytes": 2048,
        "chunkCount": 3,
        "createdAt": "2026-10-01T10:00:00Z",
        "updatedAt": "2026-10-02T08:30:00Z",
        "origin": "user",
        "warnings": [],
        "links": [{ "repo": REPO, "globs": ["src/**"] }]
    })
}

/// `document` as a server with `docs-authors` and `docs-useful` prints it: added by Ada through
/// Claude Code, last changed by Grace, shown 6 times and found useful twice.
pub fn document_with_quality(id: i64, title: &str) -> Value {
    let mut document = document(id, title);

    document["addedBy"] = json!({ "name": "Ada Lovelace", "via": "claude-code" });
    document["updatedBy"] = json!({ "name": "Grace Hopper", "via": null });
    document["shown"] = json!(6);
    document["useful"] = json!(2);
    document["lastUsefulAt"] = json!("2026-10-05T12:00:00Z");

    document
}

/// `document` as a server with `docs-drafts` prints it: in `status` (`live`, `draft`, `waiting` or
/// `changesRequested`) at `revision`, with no pending version.
pub fn reviewed_document(id: i64, title: &str, status: &str, revision: i64) -> Value {
    let mut document = document(id, title);

    document["status"] = json!(status);
    document["revision"] = json!(revision);
    document["pending"] = Value::Null;

    document
}

/// The pending summary of a live document's `docs list` row: pending row `id`, in `status`.
pub fn pending_summary(id: i64, status: &str, revision: i64) -> Value {
    json!({
        "id": id,
        "status": status,
        "revision": revision,
        "updatedBy": { "name": "Grace Hopper", "via": null },
        "updatedAt": "2026-10-04T09:00:00Z"
    })
}

/// A refusal as the server prints it: exit 1, `{"error": code, "message"}` on stdout and the line
/// on stderr.
pub fn refusing(code: &str, message: &str) -> ExecOutput {
    exits(
        1,
        &json!({ "error": code, "message": message }).to_string(),
        &format!("harness-knowledge: {message}\n"),
    )
}

/// The `stats --json` of a server with `docs-drafts`: 12 documents of which 2 are drafts, 3 wait for
/// approval (2 of them could be approved by this person) and 1 live document has a pending version,
/// with approval on.
pub fn drafts_stats() -> Value {
    json!({
        "documents": 12, "chunks": 40, "embeddedChunks": 40, "bytes": 1000, "globalDocuments": 1,
        "repositories": 2, "generation": 31, "schema": 2, "model": null, "held": 1,
        "drafts": 2, "waiting": 3, "waitingForYou": 2, "pending": 1, "approval": true,
        "me": { "name": "Ada Lovelace" }
    })
}

/// The `match` of `docs similar` for stored document `id`, by `reason`.
pub fn similar_match(id: Value, title: &str, reason: &str) -> Value {
    json!({
        "id": id,
        "title": title,
        "place": "local",
        "reason": reason,
        "sharedChunks": 3,
        "chunks": 4,
        "addedBy": { "name": "Ada Lovelace", "via": null },
        "updatedBy": null,
        "updatedAt": "2026-10-02T08:30:00Z"
    })
}

/// The id of the first proposal of the tests' queue.
pub const HELD_ID: &str = "3f1c2a9e-7b4d-4e0a-9c55-1d2e3f4a5b6c";

/// The id of the second proposal of the tests' queue.
pub const HELD_ID_2: &str = "a0b1c2d3-e4f5-4a6b-8c7d-9e0f1a2b3c4d";

/// The revision of the first proposal.
pub const REVISION: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// A proposal as `held list` prints it (without `content`, with a `preview`), or as `held get`
/// prints it when `text` is under `content`: pass the key.
pub fn held_item(id: &str, title: &str, text_key: &str, text: &str) -> Value {
    json!({
        "version": 1,
        "heldId": id,
        "createdAt": "2026-10-03T09:00:00Z",
        "client": "claude-code",
        "session": "chat-1",
        "projectPath": "/work/widget",
        "repoKey": REPO,
        "title": title,
        "format": "markdown",
        "name": format!("{title}.md"),
        "sourcePath": null,
        "bytes": text.len(),
        "contentSha256": "c".repeat(64),
        "links": [{ "repo": REPO, "globs": [] }],
        "replaces": null,
        "revision": REVISION,
        text_key: text
    })
}

/// A hit as `search --json` prints it.
pub fn hit(doc_id: i64, chunk_id: i64, title: &str, section: &str, snippet: &str) -> Value {
    json!({
        "docId": doc_id,
        "chunkId": chunk_id,
        "id": format!("{doc_id}:{chunk_id}"),
        "title": title,
        "section": section,
        "score": 42,
        "snippet": snippet,
        "block": format!("<knowledge-doc id=\"{doc_id}:{chunk_id}\">\n{snippet}\n</knowledge-doc>")
    })
}

/// Where the installer puts `harness-knowledge` under `home`, and the env that finds it and its
/// config file there.
pub fn installed_at(home: &Path) -> (Env, PathBuf) {
    installed_binary_at(home, "harness-knowledge")
}

/// A home with an installed `harness-knowledge` (never run: the stand-ins answer).
pub fn installed() -> (tempfile::TempDir, Env) {
    installed_binary("harness-knowledge")
}

/// The path of the installed server in `env`.
pub fn server(env: &Env) -> PathBuf {
    installed_at(&env.home).1
}

/// A server that answers `--version` and lists `features`; every other command is a usage error
/// (exit 2) until a rule answers it, as an older server's would be.
pub fn cli(features: &str) -> Cli {
    Cli::default()
        .starting(&["--version"], exits(0, "harness-knowledge 1.0.0\n", ""))
        .starting(&["features"], exits(0, features, ""))
}

/// The JSON of `value` and a newline, one line of the import's output.
pub fn line(value: &Value) -> String {
    format!("{value}\n")
}

/// Plays the streamed import of `peer`: reads the manifest from stdin until the app closes it,
/// writes `lines` to stdout and `stderr` to stderr, closes both and exits with `exit`. Returns the
/// spec the app spawned and the manifest it wrote.
pub async fn play_import(mut peer: FakePeer, lines: &[String], stderr: &str, exit: ExitInfo) -> (ExecSpec, Value) {
    let manifest = read_manifest(&mut peer).await;
    let spec = play(peer, &lines.concat(), stderr, exit).await;

    (spec, manifest)
}

/// The manifest the app wrote to the child's stdin.
pub async fn read_manifest(peer: &mut FakePeer) -> Value {
    let mut text = String::new();
    peer.stdin.read_to_string(&mut text).await.unwrap();

    serde_json::from_str(&text).expect("the manifest is JSON")
}

/// The events of a small import of two files, as the server prints them.
pub fn small_import() -> Vec<String> {
    vec![
        line(&json!({"type": "plan", "total": 2})),
        line(
            &json!({"type": "file", "path": "/docs/a.md", "status": "added", "docId": 7, "chunks": 3, "embedding": "done"}),
        ),
        line(&json!({"type": "progress", "done": 1, "total": 2})),
        line(&json!({"type": "file", "path": "/docs/b.pdf", "status": "failed", "reason": "no text layer"})),
        line(&json!({"type": "progress", "done": 2, "total": 2})),
        line(&json!({
            "type": "done",
            "counts": {"added": 1, "updated": 0, "unchanged": 0, "skipped": 0, "failed": 1},
            "cancelled": false,
            "counter": "tokenizer",
            "embeddingPending": 0
        })),
    ]
}

/// The review queue as the view first asks for it: every scope, newest first.
pub fn queue_page(
    env: &harness_business::env::Env,
    runner: &dyn harness_business::process::ProcessRunner,
    features: &harness_business::ecosystem::KnowledgeFeatures,
    window: &harness_business::ecosystem::ListWindow,
) -> harness_business::model::KnowledgeHeldPage {
    crate::team_support::wait(harness_business::ecosystem::list_knowledge_held(
        env,
        runner,
        features,
        window,
        harness_business::ecosystem::HeldSort::Proposed,
        &harness_business::model::KnowledgeScope::All,
    ))
}
