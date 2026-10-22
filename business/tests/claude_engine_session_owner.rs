//! The Claude Code chat engine reads only its own project's saved sessions: a session file in the
//! project's session folder that records another folder with the same slug is not the chat's.

mod agent_support;
mod claude_engine_support;

use claude_engine_support::{BASIC_SESSION, Setup, history, stop_cleanly};

#[tokio::test]
async fn a_saved_session_of_another_folder_with_the_same_slug_is_not_the_chats() {
    let chat = Setup {
        resume: Some(BASIC_SESSION),
        sessions: &[BASIC_SESSION],
        ..Setup::new(&["claude_init.jsonl"])
    }
    .start()
    .await;
    assert!(!history(&chat).await.is_empty(), "the project's own session");

    // `/tmp/.tmpX` and `/tmp/_tmpX` share Claude's slug (`-tmp--tmpX`).
    let project = chat.project.to_string_lossy().into_owned();
    assert!(project.contains('.'), "{project}");
    let file = chat.project.join(format!("{BASIC_SESSION}.jsonl"));
    let text = std::fs::read_to_string(&file).expect("the session file");
    let elsewhere = text.replace("/home/demo/Projects/demo", &project.replace('.', "_"));
    std::fs::write(&file, elsewhere).expect("rewrite the session file");

    assert!(history(&chat).await.is_empty(), "another folder's session is not read");

    stop_cleanly(&chat).await;
}
