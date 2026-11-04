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

#[test]
fn the_queue_sorts_by_the_person_it_is_written_for_with_unknown_names_last() {
    let (_dir, env) = installed();
    let probe = KnowledgeFeatures::new();

    let named = |id: &str, title: &str, by: serde_json::Value| {
        let mut item = held_item(id, title, "preview", "Words.");
        item["addedBy"] = by;
        item
    };
    let server_cli = cli(ALL_FEATURES).starting(
        &["held", "list"],
        answers(json!({ "held": [
            named(HELD_ID, "Nameless", json!(null)),
            named(HELD_ID_2, "By bob", json!("bob")),
            named("7c1d6f1e-0000-4000-8000-000000000003", "By Ada", json!(" Ada Lovelace ")),
            named("7c1d6f1e-0000-4000-8000-000000000004", "By Cara", json!("Cara")),
        ] })),
    );

    let order = |window: ListWindow| {
        let page = wait(list_knowledge_held(
            &env,
            &server_cli,
            &probe,
            &window,
            HeldSort::AddedBy,
            &KnowledgeScope::All,
        ));

        page.items.iter().map(|item| item.title.clone()).collect::<Vec<_>>()
    };

    assert_eq!(
        order(ListWindow::new(None, None, None, Some(SortOrder::Asc)).unwrap()),
        ["By Ada", "By bob", "By Cara", "Nameless"]
    );
    assert_eq!(
        order(ListWindow::new(None, None, None, Some(SortOrder::Desc)).unwrap()),
        ["By Cara", "By bob", "By Ada", "Nameless"],
        "no name is last in either direction"
    );
    assert_eq!(HeldSort::of_key(Some("addedBy")), HeldSort::AddedBy);
}
