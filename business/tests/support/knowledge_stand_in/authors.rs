//! A `harness-knowledge docs list` that honours the author options, for the tests of the author
//! filter: `--author NAME`, `--author-unknown`, `--sort addedBy` and the `authors` facet of the
//! answer. Without `docs-author-filter` in the features it refuses all of them with a usage error
//! (exit 2) and sends no facet, as an older server does; so it does for a name the server refuses.

use std::collections::BTreeMap;

use harness_business::model::{AuthorCount, AuthorFacet};
use harness_business::process::ExecOutput;
use serde_json::{Value, json};

use super::{ALL_FEATURES, cli, document};
use crate::team_support::{Cli, answers, exits};

/// The feature of the filter.
pub const FILTER_FEATURE: &str = "docs-author-filter";

/// The features of a server that has everything but the author filter.
pub fn without_filter() -> String {
    ALL_FEATURES.replace(&format!("{FILTER_FEATURE}\n"), "")
}

/// The documents of the stand-in: id, title and who first added it (none for `None`).
const DOCS: [(i64, &str, Option<&str>); 5] = [
    (1, "api", Some("Ada Lovelace")),
    (2, "build", Some("Grace Hopper")),
    (3, "deploy", Some("Ada Lovelace")),
    (4, "faq", None),
    (5, "guide", Some("Grace Hopper")),
];

/// The documents as `docs list` prints them, each with its `addedBy`.
fn rows() -> Vec<Value> {
    DOCS.iter()
        .map(|(id, title, by)| {
            let mut row = document(*id, title);
            row["addedBy"] = by.map_or(Value::Null, |name| json!({ "name": name, "via": null }));
            row
        })
        .collect()
}

/// The value after `option`.
fn value_of<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == option)
        .map(|pair| pair[1].as_str())
}

/// Whether `name` is an author name the server takes.
fn takes(name: &str) -> bool {
    !name.is_empty() && name.trim() == name && name.chars().count() <= 80 && !name.chars().any(char::is_control)
}

/// The answer of `docs list` to `args` from a server that lists `features`.
fn list(features: &str, args: &[String]) -> ExecOutput {
    let listed = features.lines().any(|line| line.trim() == FILTER_FEATURE);
    let author = value_of(args, "--author");
    let unknown = args.iter().any(|arg| arg == "--author-unknown");
    let by_author = value_of(args, "--sort") == Some("addedBy");

    let refused = (author.is_some() || unknown || by_author) && !listed;
    let both = author.is_some() && unknown;

    if refused || both || author.is_some_and(|name| !takes(name)) {
        return exits(2, "", "usage: harness-knowledge docs list …");
    }

    let mut kept = rows();

    kept.sort_by_key(|row| row["addedBy"]["name"].as_str().map(str::to_owned));
    kept.sort_by_key(|row| row["addedBy"].is_null());

    if by_author && args.iter().any(|arg| arg == "--desc") {
        kept.reverse();
        kept.sort_by_key(|row| row["addedBy"].is_null());
    }

    let mut answer = json!({ "total": 0, "items": [] });

    if listed {
        answer["authors"] = facet(&kept);
    }

    kept.retain(|row| match (author, unknown) {
        (Some(name), _) => row["addedBy"]["name"] == name,
        (None, true) => row["addedBy"].is_null(),
        (None, false) => true,
    });

    answer["total"] = json!(kept.len());
    answer["items"] = json!(kept);

    answers(answer)
}

/// Who added `rows`, as the model reads it: people with the most documents first, then by name,
/// and how many have no name.
fn facet(rows: &[Value]) -> Value {
    let mut tally: BTreeMap<&str, u32> = BTreeMap::new();
    let mut unknown = 0;

    for row in rows {
        match row["addedBy"]["name"].as_str() {
            Some(name) => *tally.entry(name).or_default() += 1,
            None => unknown += 1,
        }
    }

    let mut people: Vec<AuthorCount> = tally
        .into_iter()
        .map(|(name, count)| AuthorCount {
            name: name.to_owned(),
            count,
        })
        .collect();
    people.sort_by_key(|person| std::cmp::Reverse(person.count));

    serde_json::to_value(AuthorFacet { people, unknown }).expect("a facet is JSON")
}

/// A server that lists `features` and answers `docs list` as described in the module docs.
pub fn authored_docs(features: &str) -> Cli {
    let features = features.to_owned();
    let docs_list = ["docs".to_owned(), "list".to_owned()];
    let answer_from = features.clone();

    cli(&features).answering(move |args| args.starts_with(&docs_list).then(|| list(&answer_from, args)))
}
