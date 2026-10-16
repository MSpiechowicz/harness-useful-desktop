//! Read-only smoke test against the real machine. Ignored by default; run with
//! `cargo test -p harness_business --test real_home_smoke -- --ignored --nocapture`.
//!
//! It prints counts only — never paths, names or values — and fails only when an adapter returns
//! an error. The project is the current directory (this repository).

use harness_business::env::Env;
use harness_business::harness::HarnessRegistry;
use harness_business::model::{ConfigView, ExtensionsView};

fn counts(config: &ConfigView, extensions: &ExtensionsView) -> String {
    let parse_errors = config.sections.iter().filter(|section| section.error.is_some()).count();
    let marketplace_errors = extensions
        .marketplaces
        .iter()
        .filter(|marketplace| marketplace.error.is_some())
        .count();
    let enabled_mcp = extensions.mcp_servers.iter().filter(|server| server.enabled).count();

    format!(
        "sections={} section_errors={} config_notices={} plugins={} marketplaces={} marketplace_errors={} \
         marketplace_entries={} skills={} templates={} mcp_servers={} mcp_enabled={} themes={} packages={} \
         modules={} extension_notices={}",
        config.sections.len(),
        parse_errors,
        config.notices.len(),
        extensions.plugins.len(),
        extensions.marketplaces.len(),
        marketplace_errors,
        extensions
            .marketplaces
            .iter()
            .map(|marketplace| marketplace.entries.len())
            .sum::<usize>(),
        extensions.skills.len(),
        extensions.prompt_templates.len(),
        extensions.mcp_servers.len(),
        enabled_mcp,
        extensions.themes.len(),
        extensions.packages.len(),
        extensions.extension_modules.len(),
        extensions.notices.len(),
    )
}

#[test]
#[ignore = "reads the real home directory; run manually"]
fn real_home_smoke() {
    let env = Env::from_process().unwrap();
    let project = std::env::current_dir()
        .ok()
        .and_then(|dir| dunce::canonicalize(dir).ok());

    for harness in HarnessRegistry::with_defaults().all() {
        for (label, scope) in [("user", None), ("project", project.as_deref())] {
            let config = harness.config_view(&env, scope).unwrap();
            let extensions = harness.extensions_view(&env, scope).unwrap();

            println!("{:?} {label}: {}", harness.id(), counts(&config, &extensions));
        }
    }
}

/// Projects whose saved sessions the session smoke reads (missing ones are skipped).
const SESSION_PROJECTS: [&str; 2] = [
    "/home/frogius/Projekty/harness-useful-desktop",
    "/home/frogius/Projekty/harness-useful-skills",
];

#[test]
#[ignore = "reads real session files; run manually"]
fn real_sessions_smoke() {
    use harness_business::pi_family::sessions::{SessionCache, read_tree};

    let env = Env::from_process().unwrap();
    let cache = SessionCache::default();

    for (index, raw) in SESSION_PROJECTS.iter().enumerate() {
        let Ok(project) = dunce::canonicalize(raw) else {
            println!("project#{index}: missing");
            continue;
        };

        for harness in HarnessRegistry::with_defaults().all() {
            // CLI-backed sessions need the harness binary; this smoke reads session files only.
            if harness.cli_sessions().is_some() {
                continue;
            }

            let sessions = harness.list_sessions(&env, &project, &cache).unwrap();
            let (mut nodes, mut branches, mut bad_lines, mut truncated, mut errors) = (0, 0, 0, 0, 0);

            for summary in &sessions {
                let tree = harness
                    .resolve_session(&env, &project, &summary.session)
                    .and_then(|path| read_tree(&path, &summary.session));

                match tree {
                    Ok(report) => {
                        nodes += report.tree.nodes.len();
                        branches += report.tree.branch_count as usize;
                        bad_lines += report.bad_lines;
                        truncated += usize::from(report.tree.truncated);
                    }
                    Err(_) => errors += 1,
                }
            }

            println!(
                "project#{index} {:?}: sessions={} nodes={nodes} branches={branches} bad_lines={bad_lines} \
                 truncated={truncated} errors={errors}",
                harness.id(),
                sessions.len()
            );
        }
    }
}

#[tokio::test]
#[ignore = "runs git on this repository; run manually"]
async fn real_git_smoke() {
    use harness_business::git::{git_branches, git_status};
    use harness_business::process::TokioRunner;

    let env = Env::from_process().unwrap();
    let project = dunce::canonicalize(env!("CARGO_MANIFEST_DIR")).unwrap();
    let project = project.parent().unwrap();

    let status = git_status(&env, &TokioRunner, project).await.unwrap();
    let branches = git_branches(&env, &TokioRunner, project).await.unwrap();

    println!(
        "git: is_repo={} detached={} has_upstream={} ahead={} behind={} entries={} branches={} local={} remote={}",
        status.is_repo,
        status.branch.is_none(),
        status.upstream.is_some(),
        status.ahead,
        status.behind,
        status.entries.len(),
        branches.len(),
        branches.iter().filter(|branch| !branch.is_remote).count(),
        branches.iter().filter(|branch| branch.is_remote).count(),
    );
}
