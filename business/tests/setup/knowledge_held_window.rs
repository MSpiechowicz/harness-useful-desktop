//! The review queue sorted by a column and narrowed to a scope, in the app.

use crate::knowledge_stand_in;
use crate::team_support;

use harness_business::ecosystem::{HeldSort, KnowledgeFeatures, ListWindow, list_knowledge_held};
use harness_business::model::{KnowledgeScope, SortOrder};
use knowledge_stand_in::{ALL_FEATURES, HELD_ID, HELD_ID_2, REPO, cli, held_item, installed};
use pretty_assertions::assert_eq;
use serde_json::json;
use team_support::{answers, wait};

#[test]
fn the_queue_sorts_by_a_column_and_narrows_to_a_scope() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();

    let mut global = held_item(HELD_ID_2, "Alpha glossary", "preview", "Words.");
    global["links"] = json!([]);
    global["repoKey"] = json!(null);
    let server_cli = cli(ALL_FEATURES).starting(
        &["held", "list"],
        answers(json!({ "held": [held_item(HELD_ID, "Retry policy", "preview", "Retry."), global] })),
    );

    let a_to_z = ListWindow::new(None, None, None, Some(SortOrder::Asc)).unwrap();
    let page = wait(list_knowledge_held(
        &env,
        &server_cli,
        &probe,
        &a_to_z,
        HeldSort::Title,
        &KnowledgeScope::All,
    ));
    let titles: Vec<&str> = page.items.iter().map(|item| item.title.as_str()).collect();
    assert_eq!(titles, ["Alpha glossary", "Retry policy"]);

    let any = ListWindow::new(None, None, None, None).unwrap();
    let page = wait(list_knowledge_held(
        &env,
        &server_cli,
        &probe,
        &any,
        HeldSort::Proposed,
        &KnowledgeScope::Global,
    ));
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].title, "Alpha glossary");

    let repo = KnowledgeScope::Repo { key: REPO.to_owned() };
    let page = wait(list_knowledge_held(
        &env,
        &server_cli,
        &probe,
        &any,
        HeldSort::Proposed,
        &repo,
    ));
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].title, "Retry policy");
}
