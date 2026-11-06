//! The author filter of the notes and scripts lists (`notes-author-filter`), against the stand-in
//! of `fake_agent`: `--author`, `--author-unknown` and `--sort addedBy` on the command line, the
//! authors facet in the page, a server without the feature as the app asks it, and a name the
//! servers refuse that is never sent.

use crate::memory_cli_support;
use crate::memory_remote_support;
use crate::team_support;

use harness_business::ecosystem::{
    FeaturesSupport, ListWindow, MemoryFeatures, NotesQuery, ScriptsQuery, ScriptsSupport, memory_remote_get,
};
use harness_business::model::{
    AuthorCount, AuthorFacet, AuthorFilter, MemoryScriptsAvailability, NoteSort, ScriptSort,
};
use memory_cli_support::installed;
use memory_remote_support::{Home, with_features};
use memory_stand_in::{CURRENT, Setup, last_args, note_ids, notes, script_ids, scripts, value_of};
use pretty_assertions::assert_eq;
use team_support::{Cli, exits, wait};

use crate::memory_stand_in;

/// A server that has `notes-authors` but predates `notes-author-filter`.
const WITHOUT_FILTER: &[(&str, &str)] = &[(
    "FAKE_MEMORY_FEATURES",
    "notes-hide,notes-lang,notes-useful,notes-authors",
)];

fn ada() -> AuthorFilter {
    AuthorFilter::Name {
        name: "Ada Lovelace".to_owned(),
    }
}

fn notes_query(author: Option<AuthorFilter>, sort: Option<NoteSort>) -> NotesQuery {
    NotesQuery {
        window: ListWindow::new(None, None, None, None).unwrap(),
        kind: None,
        scope: None,
        place: None,
        lang: None,
        useful: None,
        author,
        sort,
        hidden: false,
    }
}

fn scripts_query(author: Option<AuthorFilter>, sort: Option<ScriptSort>) -> ScriptsQuery {
    ScriptsQuery {
        author,
        ..ScriptsQuery::new(ListWindow::new(None, None, None, None).unwrap(), None, None, None, sort).unwrap()
    }
}

fn person(name: &str, count: u32) -> AuthorCount {
    AuthorCount {
        name: name.to_owned(),
        count,
    }
}

#[tokio::test]
async fn notes_are_filtered_and_sorted_by_who_added_them_and_the_page_lists_the_authors() {
    let setup = Setup::installed(CURRENT, &[]);

    let by_ada = notes(&setup, &notes_query(Some(ada()), None)).await;
    let args = last_args(&setup);
    assert_eq!(value_of(&args, "--author"), Some("Ada Lovelace"));
    assert!(!args.contains(&"--author-unknown".to_owned()));
    assert_eq!(note_ids(&by_ada), [1, 2, 4, 7]);

    let facet = AuthorFacet {
        people: vec![person("Ada Lovelace", 5), person("Grace Hopper", 1)],
        unknown: 2,
    };
    assert_eq!(
        by_ada.authors.as_ref(),
        Some(&facet),
        "the facet keeps every choice under the other filters"
    );

    let unknown = notes(&setup, &notes_query(Some(AuthorFilter::Unknown), None)).await;
    let args = last_args(&setup);
    assert!(args.contains(&"--author-unknown".to_owned()));
    assert!(!args.contains(&"--author".to_owned()));
    assert_eq!(note_ids(&unknown), [3, 6]);
    assert_eq!(unknown.authors.as_ref(), Some(&facet));

    let sorted = notes(&setup, &notes_query(None, Some(NoteSort::AddedBy))).await;
    let args = last_args(&setup);
    assert_eq!(value_of(&args, "--sort"), Some("addedBy"));
    assert_eq!(value_of(&args, "--order"), Some("asc"), "A to Z by default");
    assert_eq!(note_ids(&sorted), [1, 2, 4, 7, 5, 3, 6], "no name last");
    assert!(!args.contains(&"--author".to_owned()));
}

#[tokio::test]
async fn scripts_are_filtered_and_sorted_by_who_added_them_and_the_page_lists_the_authors() {
    let setup = Setup::installed(CURRENT, &[]);

    let by_ada = scripts(&setup, &scripts_query(Some(ada()), None)).await;
    let args = last_args(&setup);
    assert_eq!(value_of(&args, "--author"), Some("Ada Lovelace"));
    assert_eq!(script_ids(&by_ada), [7]);

    let facet = by_ada.authors.expect("a server with the feature sends the facet");
    assert_eq!(facet.people.first(), Some(&person("Ada Lovelace", 1)));
    assert!(facet.unknown >= 1, "scripts without a name are counted apart");

    let unknown = scripts(&setup, &scripts_query(Some(AuthorFilter::Unknown), None)).await;
    assert!(last_args(&setup).contains(&"--author-unknown".to_owned()));
    assert!(!unknown.scripts.is_empty() && !script_ids(&unknown).contains(&7));

    let sorted = scripts(&setup, &scripts_query(None, Some(ScriptSort::AddedBy))).await;
    let args = last_args(&setup);
    assert_eq!(value_of(&args, "--sort"), Some("addedBy"));
    assert_eq!(value_of(&args, "--order"), Some("asc"));
    assert_eq!(script_ids(&sorted).first(), Some(&7), "Ada before those without a name");
}

#[tokio::test]
async fn a_server_without_the_feature_is_asked_for_no_author_and_its_page_has_no_facet() {
    let setup = Setup::installed(CURRENT, WITHOUT_FILTER);
    let features = MemoryFeatures::new().support(&setup.env, setup.runner()).await;
    assert!(features.notes_authors(), "the rows still carry names");
    assert!(!features.notes_author_filter());

    let asked = notes_query(Some(ada()), Some(NoteSort::AddedBy));
    let refused = notes(&setup, &asked).await;
    assert_eq!(
        refused.availability,
        MemoryScriptsAvailability::Outdated,
        "the usage error of an older server, hence the gate"
    );

    assert!(asked.needs_features());
    let gated = asked.for_server(features);
    assert_eq!(gated, notes_query(None, None));

    let page = notes(&setup, &gated).await;
    let args = last_args(&setup);
    assert_eq!(page.availability, MemoryScriptsAvailability::Available);
    assert_eq!(page.authors, None);
    assert!(page.notes.iter().any(|note| note.added_by.is_some()));
    assert!(!args.contains(&"--author".to_owned()) && !args.contains(&"--author-unknown".to_owned()));
    assert_eq!(value_of(&args, "--sort"), Some("updated"));

    let asked = scripts_query(Some(AuthorFilter::Unknown), Some(ScriptSort::AddedBy));
    let refused = scripts(&setup, &asked).await;
    assert_eq!(refused.availability, MemoryScriptsAvailability::Outdated);

    assert!(asked.needs_features());
    let gated = asked.for_server(features);
    assert_eq!(gated, scripts_query(None, None));

    let page = scripts(&setup, &gated).await;
    let args = last_args(&setup);
    assert_eq!(page.availability, MemoryScriptsAvailability::Available);
    assert_eq!(page.authors, None);
    assert!(!args.contains(&"--author-unknown".to_owned()));
}

#[tokio::test]
async fn the_gate_keeps_everything_for_a_server_with_the_feature_and_asks_nothing_without_an_author() {
    let setup = Setup::installed(CURRENT, &[]);
    let features = MemoryFeatures::new().support(&setup.env, setup.runner()).await;
    assert!(features.notes_author_filter());

    let asked = notes_query(Some(ada()), Some(NoteSort::AddedBy));
    assert_eq!(asked.for_server(features), asked);

    let plain = notes_query(None, Some(NoteSort::Title));
    assert!(!plain.needs_features());
    assert!(!scripts_query(None, Some(ScriptSort::Name)).needs_features());
    assert!(scripts_query(None, Some(ScriptSort::AddedBy)).needs_features());
}

#[tokio::test]
async fn a_name_the_servers_refuse_is_never_sent() {
    let setup = Setup::installed(CURRENT, &[]);
    let too_long = "a".repeat(81);

    for name in [
        "",
        "   ",
        " Ada",
        "Ada ",
        "Ada\nLovelace",
        "Ada\u{202e}",
        too_long.as_str(),
    ] {
        let author = Some(AuthorFilter::Name { name: name.to_owned() });

        let page = notes(&setup, &notes_query(author.clone(), None)).await;
        assert!(
            matches!(page.availability, MemoryScriptsAvailability::Failed { .. }),
            "{name:?} is refused, not read as an older server: {:?}",
            page.availability
        );
        assert!(page.notes.is_empty());

        let page = scripts(&setup, &scripts_query(author, None)).await;
        assert!(
            matches!(page.availability, MemoryScriptsAvailability::Failed { .. }),
            "{name:?}"
        );
    }

    assert!(setup.requested().is_empty(), "no server was started for them");

    let longest = "é".repeat(80);
    assert!(
        AuthorFilter::Name { name: longest }.checked().is_ok(),
        "80 characters are fine, whatever their size in bytes"
    );
    assert!(AuthorFilter::Unknown.checked().is_ok());
}

fn features_cli(stdout: &str) -> Cli {
    Cli::default().starting(&["features"], exits(0, stdout, ""))
}

#[test]
fn the_filter_line_is_read_apart_from_the_authors_line_and_reaches_the_page_as_its_own_flag() {
    let (_dir, env) = installed();

    let filter_only = wait(MemoryFeatures::new().support(&env, &features_cli("notes-author-filter\n")));
    assert!(filter_only.notes_author_filter() && !filter_only.notes_authors());

    let authors_only = wait(MemoryFeatures::new().support(&env, &features_cli("notes-authors\n")));
    assert!(authors_only.notes_authors() && !authors_only.notes_author_filter());

    let home = Home::new();
    let flags = |features| {
        let page = memory_remote_get(&home.env, with_features(ScriptsSupport::Unsupported, features));

        (page.notes_authors_supported, page.notes_author_filter_supported)
    };

    assert_eq!(flags(filter_only), (false, true));
    assert_eq!(flags(authors_only), (true, false));

    for features in [
        FeaturesSupport::Unsupported,
        FeaturesSupport::NotInstalled,
        FeaturesSupport::Unknown,
    ] {
        assert_eq!(flags(features), (false, false), "{features:?}");
    }
}
