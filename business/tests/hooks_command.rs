//! Command hooks run through `sh -c` (Unix): stdout is captured, trimmed, redacted and
//! capped, stderr is dropped, a failure or timeout gives no output, the `HARNESS_HOOK_*`
//! variables and the working directory are set, and fixed texts pass through in run order.

#[cfg(unix)]
mod hooks_command {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use harness_business::env::Env;
    use harness_business::hooks::command::MAX_OUTPUT_BYTES;
    use harness_business::hooks::{
        ChatContext, CommandFailureKind, Firing, FiringResult, HookSession, HookSet, SendOrigin, SendPlan, run_firing,
        run_firings, with_hook_context,
    };
    use harness_business::model::{
        AgentAction, Hook, HookAction, HookApproval, HookAuthor, HookEvent, HookScope, HookStatus, HookView,
    };
    use harness_business::process::TokioRunner;
    use pretty_assertions::assert_eq;

    fn hook(number: u8, action: HookAction) -> Hook {
        Hook {
            id: format!("{number:032x}"),
            name: format!("hook {number}"),
            description: None,
            scope: HookScope::Global,
            project: None,
            event: HookEvent::PromptSubmit,
            matcher: None,
            action,
            enabled: true,
            requires: Vec::new(),
            seed: None,
            created_at: format!("2026-10-17T10:00:{number:02}Z"),
            created_by: HookAuthor::Desktop,
            updated_at: "2026-10-17T10:00:00Z".to_owned(),
            updated_by: HookAuthor::Desktop,
        }
    }

    fn shell(number: u8, command: &str, timeout_secs: Option<u32>) -> Hook {
        hook(
            number,
            HookAction::Command {
                command: command.to_owned(),
                timeout_secs,
            },
        )
    }

    /// The send plan of a first prompt in a new chat whose folder is `folder`.
    fn send_plan(folder: &Path, hooks: Vec<Hook>) -> SendPlan {
        let views: Vec<HookView> = hooks
            .into_iter()
            .map(|hook| HookView {
                id: hook.id.clone(),
                name: Some(hook.name.clone()),
                status: HookStatus::Active,
                sha256: "0".repeat(64),
                problem: None,
                approval: HookApproval::Approved,
                approved: None,
                hook: Some(hook),
            })
            .collect();

        let mut session = HookSession::new(ChatContext {
            session_id: "session-7".to_owned(),
            folder: folder.to_path_buf(),
            project: Some("/work/app".to_owned()),
            fresh: true,
            utility: false,
        });
        let prompt = AgentAction::Prompt {
            message: "hi".to_owned(),
            images: Vec::new(),
            streaming_behavior: None,
        };

        session.on_send(&HookSet::for_chat(&views, None), &prompt, SendOrigin::User)
    }

    /// The `promptSubmit` firing of `hooks` in a chat whose folder is `folder`.
    fn firing(folder: &Path, hooks: Vec<Hook>) -> Firing {
        let mut plan = send_plan(folder, hooks);
        plan.firings.remove(0)
    }

    fn env() -> Env {
        Env::for_tests("/nonexistent-home")
    }

    /// Runs the `promptSubmit` firing of `hooks` in a chat whose folder is `folder`.
    async fn run(folder: &Path, hooks: Vec<Hook>) -> FiringResult {
        run_firing(&TokioRunner, &env(), &firing(folder, hooks)).await
    }

    /// The texts of `result`, in run order.
    fn texts(result: &FiringResult) -> Vec<&str> {
        result.parts.iter().map(|part| part.text.as_str()).collect()
    }

    #[tokio::test]
    async fn output_is_trimmed_and_texts_keep_their_order() {
        let folder = tempfile::tempdir().unwrap();
        let hooks = vec![
            shell(1, "printf 'one\\n\\n'", None),
            hook(
                2,
                HookAction::AddContext {
                    text: "fixed".to_owned(),
                },
            ),
            shell(3, "echo three; echo hidden >&2", None),
        ];

        let result = run(folder.path(), hooks).await;

        assert_eq!(texts(&result), vec!["one", "fixed", "three"]);
        assert!(result.failures.is_empty());

        assert_eq!(
            with_hook_context("Go", &result.parts),
            "<harness-hook-context>\n<hook name=\"hook 1\">one</hook>\n<hook name=\"hook 2\">fixed</hook>\n<hook name=\"hook 3\">three</hook>\n</harness-hook-context>\n\nGo"
        );
    }

    #[tokio::test]
    async fn output_is_redacted_and_capped() {
        let folder = tempfile::tempdir().unwrap();
        let token = format!("npm_{}", "a1B2".repeat(8));
        let hooks = vec![
            shell(1, &format!("echo token={token}"), None),
            shell(2, "head -c 40000 /dev/zero | tr '\\0' x", None),
        ];

        let result = run(folder.path(), hooks).await;

        assert!(!result.parts[0].text.contains(&token), "{}", result.parts[0].text);
        assert!(result.parts[0].text.starts_with("token="));
        assert_eq!(result.parts[1].text.len(), MAX_OUTPUT_BYTES);
    }

    #[tokio::test]
    async fn failures_and_blank_output_give_nothing() {
        let folder = tempfile::tempdir().unwrap();
        let hooks = vec![
            shell(1, "echo partial; exit 3", None),
            shell(2, "true", None),
            shell(3, "echo ok", None),
        ];

        let result = run(folder.path(), hooks).await;

        assert_eq!(texts(&result), vec!["ok"]);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].hook_name, "hook 1");
        assert_eq!(result.failures[0].kind, CommandFailureKind::Exit(Some(3)));
    }

    #[tokio::test]
    async fn a_timeout_kills_the_command() {
        let folder = tempfile::tempdir().unwrap();
        let marker = folder.path().join("still-running");
        let command = format!("sleep 2; touch '{}'", marker.display());

        let started = Instant::now();
        let result = run(folder.path(), vec![shell(1, &command, Some(1))]).await;

        assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
        assert!(result.parts.is_empty());
        assert_eq!(result.failures[0].kind, CommandFailureKind::Timeout);

        tokio::time::sleep(Duration::from_millis(2500)).await;
        assert!(!marker.exists(), "the shell's children were killed too");
    }

    #[tokio::test]
    async fn the_hook_variables_and_folder_are_set() {
        let folder = tempfile::tempdir().unwrap();
        std::fs::write(folder.path().join("marker-file"), "").unwrap();

        let command = concat!(
            "printf '%s\\n' \"$HARNESS_HOOK_EVENT\" \"$HARNESS_HOOK_PROJECT\" \"$HARNESS_HOOK_SESSION\" ",
            "\"${HARNESS_HOOK_TOOL-unset}.\" \"${HARNESS_HOOK_TOOL_ERROR-unset}.\" \"${HARNESS_HOOK_TOUCHED-unset}.\"; ls"
        );

        let result = run(folder.path(), vec![shell(1, command, None)]).await;

        assert_eq!(
            result.parts[0].text,
            "promptSubmit\n/work/app\nsession-7\n.\n.\n.\nmarker-file"
        );
    }

    #[tokio::test]
    async fn session_start_and_prompt_submit_context_merge_in_run_order() {
        let folder = tempfile::tempdir().unwrap();
        let mut start = shell(2, "echo branch", None);
        start.event = HookEvent::SessionStart;

        let plan = send_plan(
            folder.path(),
            vec![
                hook(
                    1,
                    HookAction::AddContext {
                        text: "style".to_owned(),
                    },
                ),
                start,
                hook(
                    3,
                    HookAction::AddContext {
                        text: "tests".to_owned(),
                    },
                ),
            ],
        );
        assert_eq!(plan.firings.len(), 2);

        let result = run_firings(&TokioRunner, &env(), &plan.firings).await;

        assert_eq!(texts(&result), vec!["style", "branch", "tests"]);
    }
}
