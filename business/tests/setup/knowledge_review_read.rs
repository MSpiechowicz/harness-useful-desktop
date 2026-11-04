//! Reading a pending version for its review: compare, history, comments, download, and the team
//! approval switch.

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::BusinessError;
use harness_business::ecosystem::{
    compare_doc, doc_comments, doc_history, download_doc, get_approval, reply_comment, resolve_comment, set_approval,
};
use harness_business::model::{
    CommentSide, DiffLineKind, GitFileStatus, KnowledgeEventKind, KnowledgeId, KnowledgeLastChange, KnowledgeOutcome,
    KnowledgePlace, KnowledgeRefusalCode, MAX_AUTHOR_NAME_CHARS,
};
use knowledge_stand_in::review::{ID, doc_id, docs_answering};
use knowledge_stand_in::{ALL_FEATURES, cli, installed, refusing, server};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use team_support::{answers, wait};

fn diff_section(path: &str, status: &str, additions: u32, deletions: u32) -> Value {
    json!({
        "path": path, "oldPath": null, "status": status, "staged": false, "isBinary": false,
        "truncated": false, "additions": additions, "deletions": deletions,
        "hunks": [{
            "header": "@@ -1,2 +1,2 @@", "oldStart": 1, "oldLines": 2, "newStart": 1, "newLines": 2,
            "lines": [
                { "kind": "context", "oldNo": 1, "newNo": 1, "text": "Run it." },
                { "kind": "remove", "oldNo": 2, "newNo": null, "text": "old line" },
                { "kind": "add", "oldNo": null, "newNo": 2, "text": "new line" }
            ]
        }]
    })
}

#[test]
fn a_compare_is_a_file_diff_per_changed_section_and_since_names_the_version_asked() {
    let (_dir, env) = installed();
    let server_cli = docs_answering(
        ALL_FEATURES,
        &["docs", "compare"],
        answers(json!({
            "live": 5, "pending": ID, "truncated": true,
            "sections": [
                diff_section("Install", "modified", 1, 1),
                diff_section("Old part", "deleted", 0, 4),
                diff_section("New part", "added", 6, 0),
                { "path": "Same part", "status": "same" }
            ]
        })),
    );

    let outcome = wait(compare_doc(&env, &server_cli, &server(&env), &doc_id(), Some(3))).unwrap();
    let KnowledgeOutcome::Done { value: compare } = outcome else {
        panic!("refused");
    };

    assert_eq!(
        server_cli.commands().pop().unwrap(),
        ["docs", "compare", "--json", "21", "--since", "3"]
    );
    assert_eq!(
        (compare.live, compare.pending, compare.truncated),
        (Some(5.into()), Some(doc_id()), true)
    );

    let found: Vec<(&str, GitFileStatus)> = compare
        .sections
        .iter()
        .map(|file| (file.diff.path.as_str(), file.diff.status))
        .collect();
    assert_eq!(
        found,
        [
            ("Install", GitFileStatus::Modified),
            ("Old part", GitFileStatus::Deleted),
            ("New part", GitFileStatus::Added)
        ],
        "a section that did not change is left out"
    );

    let install = &compare.sections[0].diff;
    assert_eq!(
        (install.additions, install.deletions, install.staged, install.is_binary),
        (1, 1, false, false)
    );
    assert_eq!(install.hunks[0].lines[1].kind, DiffLineKind::Remove);
    assert_eq!(install.hunks[0].lines[2].new_no, Some(2));
}

#[test]
fn a_compare_of_a_new_document_has_no_live_row_and_a_lost_snapshot_is_a_refusal() {
    let (_dir, env) = installed();
    let fresh = docs_answering(
        ALL_FEATURES,
        &["docs", "compare"],
        answers(json!({ "live": null, "pending": ID, "sections": [diff_section("All", "added", 3, 0)] })),
    );

    let KnowledgeOutcome::Done { value } = wait(compare_doc(&env, &fresh, &server(&env), &doc_id(), None)).unwrap()
    else {
        panic!("refused");
    };
    assert_eq!((value.live, value.truncated), (None, false));
    assert_eq!(fresh.commands().pop().unwrap(), ["docs", "compare", "--json", "21"]);

    let lost = docs_answering(
        ALL_FEATURES,
        &["docs", "compare"],
        refusing("sinceUnavailable", "too large to keep"),
    );
    let outcome = wait(compare_doc(&env, &lost, &server(&env), &doc_id(), Some(2))).unwrap();
    assert!(matches!(
        outcome,
        KnowledgeOutcome::Refused {
            code: KnowledgeRefusalCode::SinceUnavailable,
            ..
        }
    ));

    let gone = docs_answering(ALL_FEATURES, &["docs", "compare"], answers(json!({ "live": null })));
    assert!(matches!(
        wait(compare_doc(&env, &gone, &server(&env), &doc_id(), None)),
        Err(BusinessError::Protocol(_))
    ));
}

#[test]
fn a_history_lists_the_events_newest_first_and_reads_the_words_of_an_older_server() {
    let (_dir, env) = installed();
    let server_cli = docs_answering(
        ALL_FEATURES,
        &["docs", "history"],
        answers(json!({
            "total": 5,
            "events": [
                { "kind": "wentLive", "by": "Ada", "at": "2026-10-05T10:00:00Z", "comment": null },
                { "kind": "sentBack", "by": "Grace", "at": "2026-10-04T10:00:00Z", "comment": "More detail" },
                { "kind": "asked", "by": null, "at": "2026-10-03T10:00:00Z" },
                { "kind": "somethingNew", "by": "Ada", "at": "2026-10-02T10:00:00Z", "comment": null },
                { "kind": "withdrawn", "by": "Ada", "at": "2026-10-01T10:00:00Z", "comment": null }
            ]
        })),
    );

    let history = wait(doc_history(&env, &server_cli, &server(&env), &doc_id())).unwrap();

    assert_eq!(history.total, 5);
    let kinds: Vec<KnowledgeEventKind> = history.events.iter().map(|event| event.kind).collect();
    assert_eq!(
        kinds,
        [
            KnowledgeEventKind::WentLive,
            KnowledgeEventKind::ChangesRequested,
            KnowledgeEventKind::Asked,
            KnowledgeEventKind::Other,
            KnowledgeEventKind::Withdrawn
        ]
    );
    assert_eq!(history.events[1].comment.as_deref(), Some("More detail"));
    assert_eq!(history.events[2].by, None);
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        ["docs", "history", "--json", "21"]
    );

    let gone = docs_answering(
        ALL_FEATURES,
        &["docs", "history"],
        refusing("notFound", "no such document"),
    );
    assert!(matches!(
        wait(doc_history(&env, &gone, &server(&env), &doc_id())),
        Err(BusinessError::NotFound(_))
    ));
}

fn comment_json(id: i64, parent: Option<i64>, lines: (&str, &str)) -> Value {
    json!({
        "id": id, "reviewId": 8, "section": "Install", "occurrence": 2, "side": "new", lines.0: 2, lines.1: 3,
        "body": "Why this flag?", "by": "Grace", "at": "2026-10-04T10:00:00Z", "parentId": parent,
        "resolvedAt": null, "resolvedBy": null, "revision": 3, "outdated": true
    })
}

#[test]
fn comments_are_read_with_their_lines_replies_and_outdated_flag_and_answered_by_id() {
    let (_dir, env) = installed();
    let server_cli = cli(ALL_FEATURES)
        .starting(
            &["docs", "comments"],
            answers(json!({ "comments": [comment_json(5, None, ("from", "to")), comment_json(6, Some(5), ("lineFrom", "lineTo"))] })),
        )
        .starting(&["docs", "comment", "reply"], answers(json!({ "comment": comment_json(7, Some(5), ("from", "to")) })))
        .starting(&["docs", "comment", "resolve"], answers(json!({ "id": 5 })));

    let comments = wait(doc_comments(&env, &server_cli, &server(&env), &doc_id())).unwrap();

    assert_eq!(comments.len(), 2);
    assert_eq!(
        (comments[0].from, comments[0].to, comments[0].side),
        (Some(2), Some(3), Some(CommentSide::New))
    );
    assert_eq!(
        (comments[1].from, comments[1].to),
        (Some(2), Some(3)),
        "a store's lineFrom and lineTo are read too"
    );
    assert_eq!(comments[1].parent_id, Some(5.into()));
    assert_eq!(comments[0].occurrence, Some(2));
    assert!(comments[0].outdated);

    let reply = wait(reply_comment(
        &env,
        &server_cli,
        &server(&env),
        &doc_id(),
        &5.into(),
        " Because. ",
    ))
    .unwrap();
    let KnowledgeOutcome::Done { value } = reply else {
        panic!("refused");
    };
    assert_eq!(value.comment.map(|comment| comment.id), Some(7.into()));
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        ["docs", "comment", "reply", "--json", "21", "5", "--body", "Because."]
    );

    let resolved = wait(resolve_comment(
        &env,
        &server_cli,
        &server(&env),
        &doc_id(),
        &5.into(),
        true,
    ))
    .unwrap();
    assert!(matches!(resolved, KnowledgeOutcome::Done { value } if value.comment.is_none()));
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        ["docs", "comment", "resolve", "--json", "21", "5"]
    );

    let reopened = wait(resolve_comment(
        &env,
        &server_cli,
        &server(&env),
        &doc_id(),
        &5.into(),
        false,
    ));
    assert!(
        matches!(reopened, Err(BusinessError::Unsupported(_))),
        "this stand-in has no unresolve: a usage error"
    );
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        ["docs", "comment", "unresolve", "--json", "21", "5"]
    );

    for bad in [
        wait(reply_comment(
            &env,
            &server_cli,
            &server(&env),
            &doc_id(),
            &0.into(),
            "x",
        )),
        wait(reply_comment(
            &env,
            &server_cli,
            &server(&env),
            &doc_id(),
            &5.into(),
            " ",
        )),
    ] {
        assert!(matches!(bad, Err(BusinessError::InvalidInput(_))));
    }
}

#[test]
fn a_comment_id_that_could_read_as_an_option_is_refused_before_the_server_is_asked() {
    let (_dir, env) = installed();
    let server_cli = cli(ALL_FEATURES)
        .starting(
            &["docs", "comment", "reply"],
            answers(json!({ "comment": comment_json(7, Some(5), ("from", "to")) })),
        )
        .starting(&["docs", "comment", "resolve"], answers(json!({ "id": 5 })));

    for bad in ["--body", "-h", "--json", "5", "local:0", "local:5:6:7", "local:-5"] {
        let id = KnowledgeId::Local(bad.to_owned());

        assert!(
            matches!(
                wait(reply_comment(&env, &server_cli, &server(&env), &doc_id(), &id, "x")),
                Err(BusinessError::InvalidInput(_))
            ),
            "reply {bad}"
        );
        assert!(
            matches!(
                wait(resolve_comment(&env, &server_cli, &server(&env), &doc_id(), &id, true)),
                Err(BusinessError::InvalidInput(_))
            ),
            "resolve {bad}"
        );
    }

    for good in [
        KnowledgeId::Remote(5),
        KnowledgeId::Local("local:5".to_owned()),
        KnowledgeId::Local("local:5:2".to_owned()),
    ] {
        let reply = wait(reply_comment(&env, &server_cli, &server(&env), &doc_id(), &good, "x"));

        assert!(matches!(reply, Ok(KnowledgeOutcome::Done { .. })), "{good}");
    }
}

/// A name as a hostile teammate sets it: a right-to-left override, a zero-width space, a line break
/// and far more than a name's length, around "Grace".
fn hostile_name() -> String {
    format!("\u{202E}Gra\u{200B}ce\n{}", "x".repeat(300))
}

fn cleaned(name: Option<&str>) {
    let name = name.expect("a name");

    assert!(name.starts_with("Grace"), "{name:?}");
    assert!(
        name.chars()
            .all(|c| !c.is_control() && c != '\u{202E}' && c != '\u{200B}'),
        "{name:?}"
    );
    assert!(name.chars().count() <= MAX_AUTHOR_NAME_CHARS, "{name:?}");
}

#[test]
fn every_name_a_teammate_supplies_is_cleaned_before_the_page_shows_it() {
    let (_dir, env) = installed();
    let name = hostile_name();

    let mut comment = comment_json(5, None, ("from", "to"));
    comment["by"] = json!(name);
    comment["resolvedBy"] = json!(name);
    let server_cli = cli(ALL_FEATURES)
        .starting(&["docs", "comments"], answers(json!({ "comments": [comment] })))
        .starting(
            &["docs", "history"],
            answers(json!({ "total": 1, "events": [{ "kind": "asked", "by": name, "at": "2026-10-03T10:00:00Z" }] })),
        )
        .starting(
            &["approval", "get"],
            answers(json!({ "approval": true, "place": "remote", "writable": true, "changedBy": name })),
        );

    let comments = wait(doc_comments(&env, &server_cli, &server(&env), &doc_id())).unwrap();
    cleaned(comments[0].by.as_deref());
    cleaned(comments[0].resolved_by.as_deref());

    let history = wait(doc_history(&env, &server_cli, &server(&env), &doc_id())).unwrap();
    cleaned(history.events[0].by.as_deref());

    let approval = wait(get_approval(&env, &server_cli, &server(&env))).unwrap();
    cleaned(approval.changed_by.as_deref());

    let change: KnowledgeLastChange =
        serde_json::from_value(json!({ "by": name, "at": "2026-10-03T10:00:00Z" })).unwrap();
    cleaned(change.by.as_deref());

    // A name that is nothing once cleaned is none.
    let blank: KnowledgeLastChange = serde_json::from_value(json!({ "by": "\u{200B}\u{202E} ", "at": "" })).unwrap();
    assert_eq!(blank.by, None);
}

#[test]
fn a_download_names_the_file_and_says_whether_it_is_the_original() {
    let (_dir, env) = installed();
    let target = env.home.join("saved").join("guide.pdf");
    let server_cli = docs_answering(
        ALL_FEATURES,
        &["docs", "download"],
        answers(json!({ "path": target.to_string_lossy(), "original": true })),
    );

    let outcome = wait(download_doc(&env, &server_cli, &server(&env), &doc_id(), &target, true)).unwrap();

    let KnowledgeOutcome::Done { value } = outcome else {
        panic!("refused");
    };
    assert_eq!(
        (value.path.as_str(), value.original),
        (target.to_string_lossy().as_ref(), true)
    );
    assert_eq!(
        server_cli.commands().pop().unwrap(),
        [
            "docs",
            "download",
            "--json",
            "21",
            "--to",
            &target.to_string_lossy(),
            "--live"
        ]
    );

    let text = docs_answering(
        ALL_FEATURES,
        &["docs", "download"],
        answers(json!({ "original": false })),
    );
    let KnowledgeOutcome::Done { value } =
        wait(download_doc(&env, &text, &server(&env), &doc_id(), &target, false)).unwrap()
    else {
        panic!("refused");
    };
    assert_eq!(
        (value.path.as_str(), value.original),
        (target.to_string_lossy().as_ref(), false),
        "the path asked for"
    );
    assert!(!text.commands().pop().unwrap().contains(&"--live".to_owned()));
}

#[test]
fn the_approval_switch_is_read_set_and_refused_for_a_read_only_role() {
    let (_dir, env) = installed();
    let server_cli = cli(ALL_FEATURES)
        .starting(
            &["approval", "get"],
            answers(json!({
                "approval": true, "place": "remote", "writable": false,
                "changedBy": "Ada", "changedAt": "2026-11-04T10:00:00Z"
            })),
        )
        .starting(
            &["approval", "set", "--json", "off"],
            answers(json!({ "approval": false })),
        )
        .starting(
            &["approval", "set", "--json", "on"],
            refusing("readOnly", "this role cannot change it"),
        );

    let state = wait(get_approval(&env, &server_cli, &server(&env))).unwrap();
    assert_eq!(
        (state.approval, state.place, state.writable),
        (true, KnowledgePlace::Remote, Some(false))
    );
    assert_eq!(
        (state.changed_by.as_deref(), state.changed_at.as_deref()),
        (Some("Ada"), Some("2026-11-04T10:00:00Z")),
        "who switched it and when"
    );
    assert_eq!(server_cli.commands().pop().unwrap(), ["approval", "get", "--json"]);

    let off = wait(set_approval(&env, &server_cli, &server(&env), false)).unwrap();
    let KnowledgeOutcome::Done { value } = off else {
        panic!("refused");
    };
    assert_eq!(
        (value.approval, value.place, value.writable),
        (false, KnowledgePlace::Remote, Some(true))
    );

    let on = wait(set_approval(&env, &server_cli, &server(&env), true)).unwrap();
    assert!(matches!(
        on,
        KnowledgeOutcome::Refused {
            code: KnowledgeRefusalCode::ReadOnly,
            ..
        }
    ));

    let local = cli(ALL_FEATURES).starting(
        &["approval", "get"],
        answers(json!({ "approval": false, "place": "local", "writable": null })),
    );
    let state = wait(get_approval(&env, &local, &server(&env))).unwrap();
    assert_eq!((state.place, state.writable), (KnowledgePlace::Local, None));
    assert_eq!(
        (state.changed_by, state.changed_at),
        (None, None),
        "an answer without them says none"
    );

    let odd = cli(ALL_FEATURES).starting(&["approval", "get"], answers(json!({ "approval": true })));
    assert!(matches!(
        wait(get_approval(&env, &odd, &server(&env))),
        Err(BusinessError::Protocol(_))
    ));
}
