//! Extension UI requests over a running session: approvals, every dialog kind, cancellation and
//! the automatic answers to `ask` and unknown methods.

mod agent_support;

use std::sync::Arc;

use agent_support::{start_omp, start_pi};
use harness_business::model::{
    AgentAction, AgentEvent, AgentReply, IpcErrorCode, PromptStatus, UiOutcome, UiRequestKind, UiResponse,
};
use pretty_assertions::assert_eq;

fn prompt(message: &str) -> AgentAction {
    AgentAction::Prompt {
        message: message.to_owned(),
        streaming_behavior: None,
    }
}

fn answer(id: &str, outcome: UiOutcome) -> UiResponse {
    UiResponse {
        id: id.to_owned(),
        outcome,
    }
}

fn value(text: &str) -> UiOutcome {
    UiOutcome::Value { value: text.to_owned() }
}

fn is_request(id: &'static str) -> impl Fn(&AgentEvent) -> bool {
    move |event| matches!(event, AgentEvent::UiRequest { request } if request.id == id)
}

#[tokio::test]
async fn approving_a_tool_call_sends_the_chosen_option_back() {
    let run = start_omp("omp_approval.jsonl").await;
    run.manager.send(run.key(), prompt("Run the tests.")).await.unwrap();

    let AgentEvent::UiRequest { request } = run.wait("approval", is_request("approval-1")).await else {
        unreachable!();
    };
    let UiRequestKind::Select { options, details, .. } = &request.kind else {
        panic!("approvals are selects");
    };
    assert_eq!(options, &["Approve", "Deny"]);
    assert_eq!(details, &[Some("Runs once in the project directory".to_owned()), None]);

    run.manager
        .respond(run.key(), answer("approval-1", value("Approve")))
        .await
        .unwrap();
    run.wait("completion", |event| {
        matches!(
            event,
            AgentEvent::PromptResult {
                status: PromptStatus::Completed,
                ..
            }
        )
    })
    .await;

    // The dialog is closed once answered.
    let again = run
        .manager
        .respond(run.key(), answer("approval-1", value("Approve")))
        .await;
    assert_eq!(again.unwrap_err().code(), IpcErrorCode::NotFound);

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn denying_a_tool_call_fails_the_tool_and_aborts_the_prompt() {
    let run = start_omp("omp_approval_denied.jsonl").await;
    run.manager.send(run.key(), prompt("Run the tests.")).await.unwrap();

    run.wait("approval", is_request("approval-1")).await;
    run.manager
        .respond(run.key(), answer("approval-1", value("Deny")))
        .await
        .unwrap();

    let failed = run
        .wait("tool end", |event| {
            matches!(event, AgentEvent::ToolEnd { is_error: true, .. })
        })
        .await;
    assert!(matches!(failed, AgentEvent::ToolEnd { name, .. } if name == "bash"));
    run.wait("aborted", |event| {
        matches!(
            event,
            AgentEvent::PromptResult {
                status: PromptStatus::Aborted,
                ..
            }
        )
    })
    .await;

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn omp_extension_command_dialogs_complete_before_a_handled_prompt() {
    let run = Arc::new(start_omp("omp_ui_requests.jsonl").await);

    // OMP acknowledges an extension command only after its dialogs, so the prompt is answered
    // while `send` is still waiting (its timeout does not run out while a dialog is open).
    let sender = run.clone();
    let pending = tokio::spawn(async move { sender.manager.send(sender.key(), prompt("/setup")).await });

    run.wait("confirm", is_request("ui-1")).await;
    let confirmed = UiOutcome::Confirmed { confirmed: true };
    run.manager.respond(run.key(), answer("ui-1", confirmed)).await.unwrap();
    run.wait("input", is_request("ui-2")).await;
    run.manager
        .respond(run.key(), answer("ui-2", value("pre-commit")))
        .await
        .unwrap();

    let AgentEvent::UiRequest { request } = run.wait("editor", is_request("ui-3")).await else {
        unreachable!();
    };
    assert_eq!(
        request.kind,
        UiRequestKind::Editor {
            title: "Edit the hook script".to_owned(),
            prefill: Some("#!/bin/sh\ncargo fmt --all --check\n".to_owned()),
        }
    );
    run.manager
        .respond(run.key(), answer("ui-3", value("#!/bin/sh\n")))
        .await
        .unwrap();

    let reply = pending.await.unwrap().unwrap();
    assert_eq!(
        reply,
        AgentReply::Prompt {
            agent_invoked: Some(false),
            disposition: None,
        }
    );
    run.wait("handled", |event| {
        matches!(
            event,
            AgentEvent::PromptResult {
                status: PromptStatus::Handled,
                ..
            }
        )
    })
    .await;

    let events = run.events();
    let has = |predicate: &dyn Fn(&AgentEvent) -> bool| events.iter().any(predicate);
    assert!(has(
        &|event| matches!(event, AgentEvent::Title { title } if title == "Project setup")
    ));
    assert!(has(&|event| matches!(event, AgentEvent::Status { text: None, .. })));
    assert!(has(
        &|event| matches!(event, AgentEvent::Widget { placement, lines, .. } if placement == "belowEditor" && lines.len() == 2)
    ));
    assert!(has(
        &|event| matches!(event, AgentEvent::EditorText { text } if text == "/review src/lib.rs")
    ));
    assert!(has(
        &|event| matches!(event, AgentEvent::OpenUrl { url, .. } if url == "https://github.com/settings/tokens")
    ));

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_cancelled_dialog_closes_and_ask_or_unknown_methods_are_declined() {
    let run = start_omp("omp_ui_cancel.jsonl").await;

    run.wait("select", is_request("sel-1")).await;
    run.wait(
        "cancel",
        |event| matches!(event, AgentEvent::UiCancel { request_id } if request_id == "sel-1"),
    )
    .await;
    // The fake agent only gets here after reading `cancelled` answers to `ask` and `teleport`.
    run.wait("notice", |event| matches!(event, AgentEvent::Notice { .. }))
        .await;

    let late = run.manager.respond(run.key(), answer("sel-1", value("a"))).await;
    assert_eq!(late.unwrap_err().code(), IpcErrorCode::NotFound);

    let shown = run
        .events()
        .iter()
        .filter(|event| matches!(event, AgentEvent::UiRequest { .. }))
        .count();
    assert_eq!(shown, 1, "ask and unknown methods never reach the webview");

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn pi_dialog_answers_use_confirmed_cancelled_and_value() {
    let run = Arc::new(start_pi("pi_ui.jsonl").await);
    let sender = run.clone();
    let pending = tokio::spawn(async move { sender.manager.send(sender.key(), prompt("/setup")).await });

    run.wait("confirm", is_request("u1")).await;
    let declined = UiOutcome::Confirmed { confirmed: false };
    run.manager.respond(run.key(), answer("u1", declined)).await.unwrap();
    run.wait("input", is_request("u2")).await;
    run.manager
        .respond(run.key(), answer("u2", UiOutcome::Cancelled))
        .await
        .unwrap();
    run.wait("editor", is_request("u3")).await;
    run.manager
        .respond(run.key(), answer("u3", value("a\nb\nc")))
        .await
        .unwrap();

    let reply = pending.await.unwrap().unwrap();
    assert!(matches!(reply, AgentReply::Prompt { disposition: Some(d), .. } if d == "handled"));
    run.wait("handled", |event| {
        matches!(
            event,
            AgentEvent::PromptResult {
                status: PromptStatus::Handled,
                ..
            }
        )
    })
    .await;

    // A queued prompt whose run fails completes as an error when the run settles.
    run.manager.send(run.key(), prompt("fail please")).await.unwrap();
    let failed = run
        .wait("error", |event| {
            matches!(
                event,
                AgentEvent::PromptResult {
                    status: PromptStatus::Error,
                    ..
                }
            )
        })
        .await;
    assert!(matches!(failed, AgentEvent::PromptResult { error: Some(error), .. } if error == "overloaded"));

    run.manager.stop(run.key()).await.unwrap();
}
