//! What Codex asks a chat: approvals as approval cards bound to their tool card (allowed once, for
//! the session where Codex offers it, rejected), the model's questions and an MCP server's form as
//! question dialogs, and the requests the app declines or refuses at once.

use crate::codex_engine_support;

use codex_engine_support::{CodexChat, Setup, prompt, start};
use harness_business::BusinessError;
use harness_business::model::{AgentEvent, NoticeLevel, UiPurpose, UiRequest, UiRequestKind};
use pretty_assertions::assert_eq;

/// The title and options of a `select` dialog.
fn select(request: &UiRequest) -> (String, Vec<String>) {
    match &request.kind {
        UiRequestKind::Select { title, options, .. } => (title.clone(), options.clone()),
        other => panic!("not a select: {other:?}"),
    }
}

/// The title of an `input` dialog.
fn input_title(request: &UiRequest) -> String {
    match &request.kind {
        UiRequestKind::Input { title, .. } => title.clone(),
        other => panic!("not an input: {other:?}"),
    }
}

/// Answers the next dialog (the `seen`-th) with `value`; the dialog.
async fn answer(chat: &CodexChat, seen: usize, value: &str) -> UiRequest {
    let dialog = chat.dialog(seen).await;
    chat.pick(&dialog.id, value).await.expect("the answer is taken");
    dialog
}

#[tokio::test]
async fn approvals_are_allowed_once_for_the_session_or_rejected() {
    let chat = start("codex_engine_approvals.jsonl").await;
    chat.act(prompt("Create hello.txt, then run ls -1.")).await;

    let probe = answer(&chat, 0, "Allow once").await;
    let (title, options) = select(&probe);
    assert_eq!(probe.purpose, Some(UiPurpose::Approval));
    assert_eq!(
        probe.tool_call_id.as_deref(),
        Some("exec-f61152e4-887b-4723-add5-193b11a1ad79")
    );
    // The command as it runs: its script is double-quoted, so the wrapper stays.
    assert!(
        title.starts_with("Allow shell: /usr/bin/bash -lc \"pwd && rg --files"), // guard-allow: installer-shell
        "{title}"
    );
    assert_eq!(options, ["Allow once", "Reject"], "Codex lists no session answer here");

    let change = answer(&chat, 1, "Always allow").await;
    let (title, options) = select(&change);
    assert_eq!(title, "Allow apply_patch: hello.txt?");
    assert_eq!(options, ["Allow once", "Always allow", "Reject"]);

    let listing = chat.dialog(2).await;
    let refused = chat.pick(&listing.id, "Always allow").await.unwrap_err();
    assert!(matches!(refused, BusinessError::InvalidInput(_)), "{refused:?}");
    chat.pick(&listing.id, "Reject").await.expect("the dialog stayed open");
    assert_eq!(select(&listing).0, "Allow shell: ls -1?");

    let events = chat.settled(1).await;
    let ends = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::ToolEnd { .. }))
        .count();
    assert_eq!(ends, 3);

    chat.finish().await;
}

#[tokio::test]
async fn always_allow_answers_for_the_session_when_codex_offers_it() {
    let chat = start("codex_engine_always.jsonl").await;
    chat.act(prompt("Check the repository.")).await;

    let dialog = answer(&chat, 0, "Always allow").await;
    let UiRequestKind::Select { title, details, .. } = &dialog.kind else {
        panic!("not a select");
    };
    assert_eq!(title, "Allow shell: git status?");
    assert_eq!(details[0].as_deref(), Some("Read the repository state"));
    assert_eq!(dialog.tool_call_id.as_deref(), Some("exec-always"));

    chat.settled(1).await;
    chat.finish().await;
}

#[tokio::test]
async fn questions_and_forms_are_asked_one_dialog_at_a_time() {
    let chat = start("codex_engine_questions.jsonl").await;
    chat.act(prompt("Ask me.")).await;

    let colour = answer(&chat, 0, "blue").await;
    let (title, options) = select(&colour);
    assert_eq!(colour.purpose, Some(UiPurpose::Question));
    assert_eq!(title, "Question 1 of 2: Which colour?");
    assert_eq!(&options[..2], ["red", "blue"]);

    let name = answer(&chat, 1, "Ada").await;
    assert_eq!(input_title(&name), "Question 2 of 2: Your name?");

    let count = answer(&chat, 2, "3").await;
    assert_eq!(
        input_title(&count),
        "Question 1 of 2: tracker asks for Count. Create the ticket?"
    );
    let urgent = answer(&chat, 3, "Yes").await;
    assert_eq!(&select(&urgent).1[..2], ["Yes", "No"]);

    let events = chat.settled(1).await;
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::Notice { level: NoticeLevel::Warning, message } if message.contains("tracker asked to open a web page")
    )));
    let shown = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::UiRequest { .. }))
        .count();
    assert_eq!(shown, 4, "the URL, the tool call and the token refresh show no dialog");

    chat.finish().await;
}

#[tokio::test]
async fn asks_are_answered_by_what_the_chat_can_show() {
    let chat = start("codex_engine_asks_shown.jsonl").await;
    chat.act(prompt("Tidy up.")).await;

    // The permissions it cannot show and the other thread's ask got no dialog.
    let change = chat.dialog(0).await;
    let (title, options) = select(&change);
    assert_eq!(
        title,
        "Allow apply_patch: writing anywhere below /, outside the project /home/user/.bashrc, a.rs, b.rs, c.rs, d.rs?"
    );
    assert_eq!(
        options,
        ["Allow once", "Reject"],
        "no Always allow for a folder to write below"
    );
    assert!(change.id.starts_with("s-file%3A1"), "{}", change.id);
    chat.pick(&change.id, "Allow once").await.unwrap();

    let stdin = answer(&chat, 1, "Reject").await;
    assert_eq!(select(&stdin).0, "Allow input to the running command: python3?");

    let permissions = answer(&chat, 2, "Allow once").await;
    assert_eq!(
        select(&permissions).0,
        "Allow extra permissions: writing /srv/out, network access?"
    );

    let events = chat.settled(1).await;
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::Notice { level: NoticeLevel::Warning, message } if message.contains("cannot show in full")
    )));
    let shown = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::UiRequest { .. }))
        .count();
    assert_eq!(shown, 3);

    chat.finish().await;
}

#[tokio::test]
async fn a_long_command_is_shown_in_full_as_written_and_a_form_masks_secrets() {
    let chat = start("codex_engine_asks_long.jsonl").await;
    chat.act(prompt("Echo a lot.")).await;

    let dialog = answer(&chat, 0, "Reject").await;
    let (title, _) = select(&dialog);
    let command = format!("printf  '%s'\n{} end-of-command", "x".repeat(2480));
    assert_eq!(title, format!("Allow shell: {command}?"));

    let form = chat.dialog(1).await;
    let shown = input_title(&form);
    assert!(shown.starts_with("tracker asks for Code. Sign in with"), "{shown}");
    assert!(!shown.contains("Zx9Qw8Er7Ty6Ui5Op4As3Df2Gh1Jk0Lm"), "{shown}");
    chat.session
        .respond(harness_business::model::UiResponse {
            id: form.id.clone(),
            outcome: harness_business::model::UiOutcome::Cancelled,
        })
        .await
        .unwrap();

    chat.settled(1).await;
    chat.finish().await;
}

#[tokio::test]
async fn frames_over_the_limit_are_answered_and_end_what_they_were_about() {
    let chat = start("codex_engine_oversized.jsonl").await;
    chat.act(prompt("Make it big.")).await;

    let events = chat.settled(1).await;
    let ended = events.iter().find_map(|event| match event {
        AgentEvent::ToolEnd {
            tool_call_id,
            is_error,
            touched_paths,
            ..
        } if tool_call_id == "patch-big" => Some((*is_error, touched_paths.clone())),
        _ => None,
    });
    assert_eq!(ended, Some((true, vec!["big.rs".to_owned()])));
    let faults = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::FrameError { .. }))
        .count();
    assert_eq!(faults, 2);
    assert!(
        !events.iter().any(|event| matches!(event, AgentEvent::UiRequest { .. })),
        "nothing waits for an answer"
    );

    let started = std::time::Instant::now();
    let rename = harness_business::model::AgentAction::SetSessionName { name: "Big".to_owned() };
    let refused = chat.session.send(rename).await.unwrap_err();
    assert!(matches!(refused, BusinessError::Protocol(_)), "{refused:?}");
    assert!(started.elapsed() < std::time::Duration::from_secs(5), "failed at once");

    chat.finish().await;
}

#[tokio::test]
async fn approvals_name_everything_they_grant() {
    let chat = codex_engine_support::start_with("codex_engine_asks_full.jsonl", Setup::default(), |project| {
        // A link in the project to the folder above it.
        #[cfg(unix)]
        std::os::unix::fs::symlink("..", project.join("cfg")).unwrap();
        #[cfg(not(unix))]
        let _ = project;
    })
    .await;
    chat.act(prompt("Tidy up.")).await;

    // The whole script runs, not only the ls Codex reads in it: the card shows it too, ls as a hint.
    let command = answer(&chat, 0, "Reject").await;
    assert_eq!(select(&command).0, "Allow shell: ls | xargs touch x?");
    let card = chat.events().into_iter().find_map(|event| match event {
        AgentEvent::ToolStart {
            tool_call_id,
            args,
            intent,
            ..
        } if tool_call_id == "exec-pipe" => Some((args, intent)),
        _ => None,
    });
    let (args, intent) = card.expect("the command's card");
    assert_eq!(args["command"], "ls | xargs touch x");
    assert_eq!(intent.as_deref(), Some("ls"));

    // The write comes first, before 2 KB of reads.
    let permissions = answer(&chat, 1, "Reject").await;
    let title = select(&permissions).0;
    assert!(
        title.starts_with("Allow extra permissions: writing /, reading /data/archive/0000/"),
        "{title}"
    );
    assert!(
        title.contains("/data/archive/0044/") && title.len() > 2_100,
        "nothing is cut"
    );

    let change = answer(&chat, 2, "Reject").await;
    let title = select(&change).0;
    #[cfg(unix)]
    assert!(
        title.starts_with(&format!(
            "Allow apply_patch: outside the project {}",
            chat.project.join("cfg/autostart/x.desktop").display()
        )),
        "{title}"
    );
    assert!(title.ends_with("src/lib.rs?"), "{title}");

    chat.settled(1).await;
    chat.finish().await;
}

/// The args of the card of tool call `id`.
fn card_args(chat: &CodexChat, id: &str) -> serde_json::Value {
    chat.events()
        .into_iter()
        .find_map(|event| match event {
            AgentEvent::ToolStart { tool_call_id, args, .. } if tool_call_id == id => Some(args),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no card {id}"))
}

#[tokio::test]
async fn approvals_show_what_a_title_could_hide() {
    let chat = codex_engine_support::start_with("codex_engine_asks_hidden.jsonl", Setup::default(), |project| {
        // A dangling link in the project to a file outside it.
        #[cfg(unix)]
        std::os::unix::fs::symlink("../gone/evil.desktop", project.join("link.md")).unwrap();
        #[cfg(not(unix))]
        let _ = project;
    })
    .await;
    chat.act(prompt("Tidy up.")).await;

    // A move names where it goes: out of the project counts as outside, so nothing is allowed for good.
    let change = answer(&chat, 0, "Reject").await;
    let (title, options) = select(&change);
    #[cfg(unix)]
    assert_eq!(
        title,
        format!(
            "Allow apply_patch: outside the project /etc/autostart/x.desktop (moved from notes.md), {}, a.txt (moved to sub/b.txt)?",
            chat.project.join("link.md").display()
        )
    );
    #[cfg(unix)]
    assert_eq!(options, ["Allow once", "Reject"]);
    let args = card_args(&chat, "patch-move");
    assert_eq!(args["changes"][0]["moveTo"], "/etc/autostart/x.desktop");
    assert_eq!(args["changes"][2].get("moveTo"), None);

    // A repository's own bash is not a system shell: the whole line shows, its program first.
    let repo = answer(&chat, 1, "Reject").await;
    assert_eq!(select(&repo).0, "Allow shell: ./tools/bash -lc 'ls -1'?"); // guard-allow: installer-shell
    let command = &card_args(&chat, "exec-repo")["command"];
    assert_eq!(command, "./tools/bash -lc 'ls -1'"); // guard-allow: installer-shell

    // No card shows this command: the title shows all that runs, nothing masked.
    let unseen = answer(&chat, 2, "Reject").await;
    assert_eq!(select(&unseen).0, "Allow shell: A_TOKEN=x&&touch y?");

    // Hidden characters are spelled out in the title and on the card.
    let bidi = answer(&chat, 3, "Reject").await;
    let shown = "echo ok⟨U+202E⟩ #; rm -rf x⟨U+200B⟩";
    assert_eq!(select(&bidi).0, format!("Allow shell: {shown}?"));
    assert_eq!(card_args(&chat, "exec-bidi")["command"], shown);

    chat.settled(1).await;
    chat.finish().await;
}
