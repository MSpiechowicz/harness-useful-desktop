//! The app window's listing of every hook and every routine (`all`): every local file of every
//! project (invalid ones included) and every team copy, each row with its project root and
//! repository key. A team row gets a root only through a root verified this session, and a local
//! row's share block is its own root's.

mod hook_store_support;
mod routine_support;
mod team_support;

use std::collections::BTreeMap;

use harness_business::hooks::team::{ProjectKey, app_listing, app_listing_all};
use harness_business::model::{
    HookAuthor, HookOrigin, HookScope, HookView, RoutineView, ShareBlock, TeamHooks, TeamRoutines,
};
use harness_business::routines::team::{app_listing as routine_listing, app_listing_all as routine_listing_all};
use harness_business::signed_docs::DocKind;
use harness_business::signed_docs::team::TeamFolder;
use hook_store_support::{at, draft};
use pretty_assertions::assert_eq;
use routine_support::{REPO as ROUTINE_REPO, body, command_draft, command_json, moment, rid, root, team_row};
use team_support::{NONE, REPO, id, local, row, team};

/// The key of a repository no project of the tests has.
const ELSEWHERE: &str = "git:github.com/o/elsewhere";

/// A row's origin, project root and repository key.
type Placed = (HookOrigin, Option<String>, Option<String>);

/// A folder under `temp`, canonical (as a project root).
fn project_dir(temp: &tempfile::TempDir, name: &str) -> String {
    let dir = temp.path().join(name);
    std::fs::create_dir_all(dir.join(".git")).unwrap();

    dunce::canonicalize(dir).unwrap().to_string_lossy().into_owned()
}

/// `projects.json`: each root with its key (`path:` keys cannot be shared).
fn map_projects<K: DocKind>(folder: &TeamFolder<K>, roots: &[(&str, &str)]) {
    let projects: BTreeMap<String, ProjectKey> = roots
        .iter()
        .map(|(root, key)| {
            let project = ProjectKey {
                key: (*key).to_owned(),
                shareable: key.starts_with("git:"),
            };

            ((*root).to_owned(), project)
        })
        .collect();

    folder.save_projects(&projects).unwrap();
}

/// Implements [`Listed`] for a listing row type.
macro_rules! listed {
    ($view:ty) => {
        impl Listed for $view {
            fn placed(&self) -> (&str, Placed) {
                (&self.id, (self.origin, self.root.clone(), self.repo_key.clone()))
            }
        }
    };
}

/// A listing row: its id and where it is placed.
trait Listed {
    fn placed(&self) -> (&str, Placed);
}

listed!(HookView);
listed!(RoutineView);

/// Where row `id` of `rows` is placed.
fn at_row<V: Listed>(rows: &[V], id: &str) -> Placed {
    rows.iter()
        .map(Listed::placed)
        .find(|(row, _)| *row == id)
        .expect("listed")
        .1
}

fn some(text: &str) -> Option<String> {
    Some(text.to_owned())
}

#[test]
fn every_hook_of_every_project_and_every_team_hook_is_listed_with_its_repository() {
    let team = team();
    let other = project_dir(&team.temp, "other");
    let stale = project_dir(&team.temp, "stale");

    let global = local(&team.store, "Global");
    let mut own = draft("Own");
    own.scope = HookScope::Project;
    own.project = Some(team.project.clone());
    let own = team.store.save(&own, HookAuthor::Desktop, NONE, at(1)).unwrap();
    let mut theirs = draft("Other");
    theirs.scope = HookScope::Project;
    theirs.project = Some(other.clone());
    let theirs = team.store.save(&theirs, HookAuthor::Desktop, NONE, at(2)).unwrap();
    let invalid = id(9);
    std::fs::write(team.store.hooks_dir().join(format!("{invalid}.json")), "not json").unwrap();

    team.copies(&[
        row(&id(1), "Team global", None),
        row(&id(2), "Team repo", Some(REPO)),
        row(&id(3), "Team elsewhere", Some(ELSEWHERE)),
    ]);
    let other_key = format!("path:{other}");
    map_projects(
        &team.folder,
        &[(&team.project, REPO), (&other, &other_key), (&stale, ELSEWHERE)],
    );

    let settings = TeamHooks {
        trust: true,
        problem: None,
        refreshed_at: None,
    };
    // `stale` maps to ELSEWHERE in `projects.json`, but no refresh of this session confirmed it.
    let verified = |root: &str| root == team.project || root == other;
    let listing = app_listing_all(
        &team.store,
        &team.folder,
        NONE,
        Some(&team.project),
        Some(&settings),
        &verified,
    );

    let ids: Vec<&str> = listing.hooks.iter().map(|view| view.id.as_str()).collect();
    assert_eq!(ids.len(), 7, "{ids:?}");

    assert_eq!(at_row(&listing.hooks, &global.id), (HookOrigin::Local, None, None));
    assert_eq!(
        at_row(&listing.hooks, &own.id),
        (HookOrigin::Local, some(&team.project), some(REPO))
    );
    assert_eq!(
        at_row(&listing.hooks, &theirs.id),
        (HookOrigin::Local, some(&other), some(&other_key))
    );
    assert_eq!(at_row(&listing.hooks, &invalid), (HookOrigin::Local, None, None));
    assert_eq!(at_row(&listing.hooks, &id(1)), (HookOrigin::Team, None, None));
    assert_eq!(
        at_row(&listing.hooks, &id(2)),
        (HookOrigin::Team, some(&team.project), some(REPO))
    );
    assert_eq!(
        at_row(&listing.hooks, &id(3)),
        (HookOrigin::Team, None, some(ELSEWHERE))
    );

    // The other project's hook cannot be shared for its own root: it has no `origin` remote.
    let block = |id: &str| listing.hooks.iter().find(|view| view.id == id).unwrap().share_block;
    assert_eq!(block(&theirs.id), Some(ShareBlock::NoOrigin));
    assert_eq!(block(&own.id), None);

    // Without `all` the listing is the project's, as before.
    let scoped = app_listing(
        &team.store,
        &team.folder,
        NONE,
        Some(&team.project),
        Some(&settings),
        true,
    );
    let mut ids: Vec<String> = scoped.hooks.into_iter().map(|view| view.id).collect();
    ids.sort_unstable();
    let mut expected = vec![global.id, own.id, invalid, id(1), id(2)];
    expected.sort_unstable();
    assert_eq!(ids, expected);
}

#[test]
fn every_routine_of_every_project_and_every_team_routine_is_listed_with_its_repository() {
    let root = root();
    let other = project_dir(&root.temp, "other");

    let global = root
        .store
        .save(&command_draft("Global"), HookAuthor::Desktop, moment(0))
        .unwrap();
    let mut theirs = command_draft("Other");
    theirs.scope = HookScope::Project;
    theirs.project = Some(other.clone());
    let theirs = root.store.save(&theirs, HookAuthor::Desktop, moment(1)).unwrap();
    let invalid = rid(9);
    std::fs::write(root.store.routines_dir().join(format!("{invalid}.json")), "not json").unwrap();

    root.folder
        .write_row(&team_row(
            &rid(1),
            body(&rid(1), "Team repo", true, command_json()),
            true,
        ))
        .unwrap();
    let elsewhere = body(&rid(2), "Team elsewhere", true, command_json()).replace(ROUTINE_REPO, ELSEWHERE);
    let mut elsewhere = team_row(&rid(2), elsewhere, true);
    elsewhere.repo = Some(ELSEWHERE.to_owned());
    root.folder.write_row(&elsewhere).unwrap();
    map_projects(&root.folder, &[(&root.project, ROUTINE_REPO)]);

    let settings = TeamRoutines {
        trust: true,
        problem: None,
        refreshed_at: None,
    };
    let verified = |root: &str| !root.is_empty();
    let listing = routine_listing_all(&root.store, &root.folder, None, Some(&settings), &verified);

    assert_eq!(listing.routines.len(), 5);
    assert_eq!(listing.counts.total, 5);

    assert_eq!(at_row(&listing.routines, &global.id), (HookOrigin::Local, None, None));
    assert_eq!(
        at_row(&listing.routines, &theirs.id),
        (HookOrigin::Local, some(&other), None)
    );
    assert_eq!(at_row(&listing.routines, &invalid), (HookOrigin::Local, None, None));
    // Off (never turned on here) team routines of the project are listed too, with its root.
    assert_eq!(
        at_row(&listing.routines, &rid(1)),
        (HookOrigin::Team, some(&root.project), some(ROUTINE_REPO))
    );
    assert_eq!(
        at_row(&listing.routines, &rid(2)),
        (HookOrigin::Team, None, some(ELSEWHERE))
    );

    let global_only = routine_listing(&root.store, &root.folder, None, Some(&settings), true);
    assert_eq!(global_only.routines.len(), 2, "the global and the invalid file");
}
