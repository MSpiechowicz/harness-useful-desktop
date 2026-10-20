//! The agents' scripts: a page listed with `harness-memory scripts list --json` (every sort), one
//! read or deleted through the pooled `harness-memory mcp` connection, all deleted with
//! `harness-memory scripts delete --all`, against the stand-in server of
//! `fake_agent` (see `src/bin/fake_agent/memory.rs` and `memory_cli.rs`).

use std::ffi::OsString;
use std::path::Path;

use harness_business::BusinessError;
use harness_business::ecosystem::{ListWindow, Repos, ScriptsQuery, ScriptsSupport};
use harness_business::model::{
    MemoryDeleteAll, MemoryScriptsAvailability, ScriptApproval, ScriptGroup, ScriptGroupCounts, ScriptPlace,
    ScriptScope, ScriptSort, SortOrder,
};
use memory_stand_in::{AGENT_APPROVAL, BARE_RUN, CURRENT, LEGACY, NO_RUN, Setup, UNPAGED};
use pretty_assertions::assert_eq;

mod memory_stand_in;

fn query(group: Option<ScriptGroup>, sort: Option<ScriptSort>) -> ScriptsQuery {
    ScriptsQuery {
        window: ListWindow::new(None, None, None, None).unwrap(),
        group,
        scope: None,
        review: None,
        sort,
        hidden: false,
    }
}

/// The names on the first page, sorted by name.
async fn names(setup: &Setup, project: Option<&Path>) -> Vec<String> {
    let page = setup
        .scripts
        .list(
            &setup.env,
            setup.runner(),
            Repos::Project(project),
            &query(None, Some(ScriptSort::Name)),
        )
        .await;
    assert_eq!(page.availability, MemoryScriptsAvailability::Available);

    page.scripts.into_iter().map(|script| script.name).collect()
}

/// Asserts that the list says `outdated`, with no scripts.
async fn assert_outdated_list(setup: &Setup) {
    let page = setup
        .scripts
        .list(&setup.env, setup.runner(), Repos::Project(None), &query(None, None))
        .await;

    assert_eq!(page.availability, MemoryScriptsAvailability::Outdated);
    assert!(page.scripts.is_empty() && page.total == 0);
}

fn texts(args: &[OsString]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

#[tokio::test]
async fn a_current_server_lists_a_page_of_project_and_global_scripts() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    assert_eq!(
        setup.scripts.availability(&setup.env).await,
        MemoryScriptsAvailability::Available
    );
    assert_eq!(
        setup.scripts.support(&setup.env).await,
        ScriptsSupport::Approvals,
        "its script_run takes no sha"
    );

    let page = setup
        .scripts
        .list(
            &setup.env,
            setup.runner(),
            Repos::Project(Some(&project)),
            &query(None, Some(ScriptSort::Name)),
        )
        .await;
    let ids: Vec<i64> = page.scripts.iter().map(|script| script.id).collect();
    assert_eq!(ids, [12, 7], "the entry of an unknown scope is skipped");
    assert_eq!(page.total, 3, "the server's count");
    // The groups are disjoint: the global script needs review, so it is in no scope group.
    assert_eq!(
        page.groups,
        ScriptGroupCounts {
            needs_review: 1,
            repo: 1,
            global: 0
        }
    );

    let repo = &page.scripts[0];
    assert_eq!(repo.scope, ScriptScope::Repo);
    assert_eq!(repo.repo.as_deref(), Some("github.com/o/r"));
    assert_eq!(repo.place, ScriptPlace::Local);
    assert_eq!(repo.author.source, "claude-code");
    assert_eq!(repo.timeout_secs, 60);
    assert_eq!(repo.approval, ScriptApproval::Local);
    assert_eq!(
        page.scripts[1].approval,
        ScriptApproval::Needed,
        "a summary without approval is read from trusted"
    );

    assert_eq!(
        names(&setup, None).await,
        ["release-notes"],
        "global only without a project"
    );

    let project_text = project.to_string_lossy();
    setup.assert_first_run(&[
        "scripts",
        "list",
        "--json",
        "--offset",
        "0",
        "--limit",
        "50",
        "--sort",
        "name",
        "--order",
        "asc",
        "--project",
        &project_text,
    ]);

    let servers: Vec<_> = setup
        .requested()
        .into_iter()
        .filter(|spec| spec.args == ["mcp"])
        .collect();
    assert_eq!(servers.len(), 1, "one server for the probe");
    assert_eq!(servers[0].program, setup.binary);
    assert_eq!(servers[0].cwd.as_deref(), Some(setup.env.home.as_path()));

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn the_page_window_group_and_order_go_to_the_server() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    let needs_review = ScriptsQuery {
        window: ListWindow::new(Some(0), Some(10), Some("  release ".to_owned()), Some(SortOrder::Desc)).unwrap(),
        group: Some(ScriptGroup::NeedsReview),
        scope: None,
        review: None,
        sort: None,
        hidden: false,
    };
    let page = setup
        .scripts
        .list(
            &setup.env,
            setup.runner(),
            Repos::Project(Some(&project)),
            &needs_review,
        )
        .await;
    let ids: Vec<i64> = page.scripts.iter().map(|script| script.id).collect();
    assert_eq!(ids, [7]);
    assert_eq!(page.total, 1);
    assert_eq!(page.groups.repo, 0, "the counts follow the query");

    assert_eq!(
        texts(&setup.runs()[0].args)[3..],
        [
            "--offset",
            "0",
            "--limit",
            "10",
            "--query",
            "release",
            "--group",
            "needsReview",
            "--sort",
            "updated",
            "--order",
            "desc",
            "--project",
            &project.to_string_lossy()
        ]
    );

    let second = ScriptsQuery {
        window: ListWindow::new(Some(1), Some(1), None, None).unwrap(),
        group: None,
        scope: None,
        review: None,
        sort: Some(ScriptSort::Name),
        hidden: false,
    };
    let page = setup
        .scripts
        .list(&setup.env, setup.runner(), Repos::Project(Some(&project)), &second)
        .await;
    let ids: Vec<i64> = page.scripts.iter().map(|script| script.id).collect();
    assert_eq!(ids, [12], "the second of odd-scope, open-prs, release-notes");
    assert_eq!(page.total, 3);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn every_sort_goes_to_the_server_by_its_name() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    let sorts = [
        (ScriptSort::Language, "language"),
        (ScriptSort::Scope, "scope"),
        (ScriptSort::Place, "place"),
        (ScriptSort::Runs, "runs"),
        (ScriptSort::LastRun, "lastRun"),
    ];

    for (run, (sort, name)) in sorts.into_iter().enumerate() {
        let page = setup
            .scripts
            .list(
                &setup.env,
                setup.runner(),
                Repos::Project(Some(&project)),
                &query(None, Some(sort)),
            )
            .await;
        assert_eq!(page.availability, MemoryScriptsAvailability::Available, "{name}");
        assert_eq!(
            texts(&setup.runs()[run].args)[7..11],
            ["--sort", name, "--order", "asc"],
            "{name}"
        );
    }

    let most_runs = ScriptsQuery {
        window: ListWindow::new(None, None, None, Some(SortOrder::Desc)).unwrap(),
        group: None,
        scope: None,
        review: None,
        sort: Some(ScriptSort::Runs),
        hidden: false,
    };
    let page = setup
        .scripts
        .list(&setup.env, setup.runner(), Repos::Project(Some(&project)), &most_runs)
        .await;
    assert_eq!(page.total, 3);
    assert_eq!(
        texts(&setup.runs()[5].args)[7..11],
        ["--sort", "runs", "--order", "desc"]
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn deleting_them_all_sends_the_confirmation_and_reads_the_counts() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    let deleted = setup
        .scripts
        .delete_all(&setup.env, setup.runner(), Repos::Project(Some(&project)), None)
        .await
        .expect("deleted");
    assert_eq!(
        deleted,
        MemoryDeleteAll {
            deleted: 3,
            team: Some(0),
            kept: None,
        }
    );
    assert_eq!(
        texts(&setup.runs()[0].args),
        [
            "scripts",
            "delete",
            "--all",
            "--yes",
            "--json",
            "--project",
            &project.to_string_lossy()
        ]
    );
    setup.scripts.shutdown().await;

    // An older server does not know the command, and without one nothing runs.
    for (unusable, code) in [
        (Setup::installed(CURRENT, UNPAGED), "unsupported"),
        (Setup::without_server(CURRENT, &[]), "notFound"),
    ] {
        let refused = unusable
            .scripts
            .delete_all(&unusable.env, unusable.runner(), Repos::Project(None), None)
            .await
            .expect_err("refused");
        let found = match refused {
            BusinessError::Unsupported(_) => "unsupported",
            BusinessError::NotFound(_) => "notFound",
            _ => "other",
        };
        assert_eq!(found, code, "{refused:?}");

        unusable.scripts.shutdown().await;
    }
}

#[tokio::test]
async fn a_server_whose_list_takes_no_pages_is_outdated() {
    let setup = Setup::installed(CURRENT, UNPAGED);

    assert_outdated_list(&setup).await;

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn get_returns_the_body_and_delete_removes_the_script() {
    let setup = Setup::installed(CURRENT, &[]);
    let project = setup.project();

    let detail = setup
        .scripts
        .get(&setup.env, setup.runner(), 12, Repos::Project(Some(&project)))
        .await
        .expect("script 12");
    assert_eq!(detail.summary.name, "open-prs");
    assert_eq!(detail.body, "gh pr list --state open\n");

    let wire = serde_json::to_value(&detail).unwrap();
    assert_eq!(wire["body"], "gh pr list --state open\n");
    assert_eq!(wire["timeoutSecs"], 60, "the summary is flattened");

    setup
        .scripts
        .delete(&setup.env, setup.runner(), 12, Repos::Project(Some(&project)))
        .await
        .expect("deleted");

    let again = setup
        .scripts
        .delete(&setup.env, setup.runner(), 12, Repos::Project(Some(&project)))
        .await;
    assert!(matches!(again, Err(BusinessError::NotFound(_))), "{again:?}");

    let gone = setup
        .scripts
        .get(&setup.env, setup.runner(), 12, Repos::Project(Some(&project)))
        .await;
    assert!(
        matches!(&gone, Err(BusinessError::NotFound(message)) if message == "script 12 not found"),
        "{gone:?}"
    );

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn another_tool_error_is_not_not_found_and_carries_the_redacted_text() {
    let setup = Setup::installed(CURRENT, &[]);

    for result in [
        setup
            .scripts
            .get(&setup.env, setup.runner(), 500, Repos::Project(None))
            .await
            .map(|_| ()),
        setup
            .scripts
            .delete(&setup.env, setup.runner(), 500, Repos::Project(None))
            .await,
    ] {
        let Err(BusinessError::Process(message)) = &result else {
            panic!("expected a process error, got {result:?}");
        };

        assert!(message.contains("the team database cannot be reached"), "{message}");
        assert!(!message.contains("hunter2"), "{message}");
    }

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_server_whose_agent_approves_scripts_keeps_scripts_without_asking_first() {
    let setup = Setup::installed(CURRENT, AGENT_APPROVAL);

    assert_eq!(
        setup.scripts.availability(&setup.env).await,
        MemoryScriptsAvailability::Available
    );
    assert_eq!(setup.scripts.support(&setup.env).await, ScriptsSupport::Supported);

    setup.scripts.shutdown().await;
}

#[tokio::test]
async fn a_server_whose_script_run_does_not_tell_leaves_the_keys_as_saved() {
    for vars in [NO_RUN, BARE_RUN] {
        let setup = Setup::installed(CURRENT, vars);

        assert_eq!(
            setup.scripts.availability(&setup.env).await,
            MemoryScriptsAvailability::Available,
            "{vars:?}"
        );
        assert_eq!(
            setup.scripts.support(&setup.env).await,
            ScriptsSupport::Unknown,
            "{vars:?}"
        );

        setup.scripts.shutdown().await;
    }
}

#[tokio::test]
async fn a_server_without_script_tools_is_outdated() {
    let setup = Setup::installed(CURRENT, LEGACY);

    assert_eq!(
        setup.scripts.availability(&setup.env).await,
        MemoryScriptsAvailability::Outdated
    );
    assert_eq!(setup.scripts.support(&setup.env).await, ScriptsSupport::Unsupported);

    assert_outdated_list(&setup).await;

    let get = setup
        .scripts
        .get(&setup.env, setup.runner(), 7, Repos::Project(None))
        .await;
    assert!(matches!(get, Err(BusinessError::Unsupported(_))), "{get:?}");

    setup.scripts.shutdown().await;
}
