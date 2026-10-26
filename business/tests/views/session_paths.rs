//! Session folder names and locations for OMP, PI and Claude Code on Linux, macOS and Windows.

use crate::session_support;

use std::fs;
use std::path::PathBuf;

use harness_business::env::{Env, OsKind};
use harness_business::harness::Harness;
use harness_business::harnesses::claude::project_slug;
use harness_business::harnesses::{Claude, Omp, Pi};
use harness_business::pi_family::sessions::{omp_dir_name, pi_dir_name, temp_root};
use pretty_assertions::assert_eq;
use session_support::session_home;

#[test]
fn omp_names_follow_home_temp_and_legacy_rules() {
    // (cwd, home, temp, windows, expected)
    let cases = [
        (
            "/home/u/Projekty/harness-useful-skills",
            "/home/u",
            "/tmp",
            false,
            "-Projekty-harness-useful-skills",
        ),
        ("/home/u", "/home/u", "/tmp", false, "-"),
        ("/home/u/", "/home/u", "/tmp", false, "-"),
        ("/home/u/a:b/c\\d", "/home/u", "/tmp", false, "-a-b-c-d"),
        ("/tmp/x/y", "/home/u", "/tmp", false, "-tmp-x-y"),
        ("/tmp", "/home/u", "/tmp", false, "-tmp"),
        ("/opt/work/app", "/home/u", "/tmp", false, "--opt-work-app--"),
        ("/home/u2/x", "/home/u", "/tmp", false, "--home-u2-x--"),
        (
            "/Users/u/dev/app",
            "/Users/u",
            "/private/var/folders/ab/T",
            false,
            "-dev-app",
        ),
        (
            "/private/var/folders/ab/T/tmp.1",
            "/Users/u",
            "/private/var/folders/ab/T",
            false,
            "-tmp-tmp.1",
        ),
        (
            "/Volumes/Data/x",
            "/Users/u",
            "/private/var/folders/ab/T",
            false,
            "--Volumes-Data-x--",
        ),
        ("/Users/U/dev", "/Users/u", "/tmp", false, "--Users-U-dev--"),
        (
            r"C:\Users\u\dev\app",
            r"C:\Users\u",
            r"C:\Users\u\AppData\Local\Temp",
            true,
            "-dev-app",
        ),
        (
            r"c:\users\U\dev",
            r"C:\Users\u",
            r"C:\Users\u\AppData\Local\Temp",
            true,
            "-dev",
        ),
        (
            r"C:\Users\u\AppData\Local\Temp\x",
            r"C:\Users\u",
            r"C:\Users\u\AppData\Local\Temp",
            true,
            "-tmp-x",
        ),
        (
            r"C:\Users\u",
            r"C:\Users\u",
            r"C:\Users\u\AppData\Local\Temp",
            true,
            "-",
        ),
        (
            r"D:\work\app",
            r"C:\Users\u",
            r"C:\Users\u\AppData\Local\Temp",
            true,
            "--D--work-app--",
        ),
        (
            r"\\server\share\x",
            r"C:\Users\u",
            r"C:\Temp",
            true,
            "---server-share-x--",
        ),
    ];

    for (cwd, home, temp, windows, expected) in cases {
        assert_eq!(omp_dir_name(cwd, home, temp, windows), expected, "{cwd}");
    }
}

#[test]
fn pi_names_strip_one_separator_and_replace_the_rest() {
    let cases = [
        ("/home/u/proj", "--home-u-proj--"),
        ("/a:b", "--a-b--"),
        (r"C:\Users\u\proj", "--C--Users-u-proj--"),
        ("/Users/u/My Project", "--Users-u-My Project--"),
    ];

    for (cwd, expected) in cases {
        assert_eq!(pi_dir_name(cwd), expected, "{cwd}");
    }
}

#[test]
fn temp_root_follows_os_tmpdir() {
    let linux = Env::for_tests("/home/u").with_os(OsKind::Linux);
    assert_eq!(temp_root(&linux), PathBuf::from("/tmp"));
    assert_eq!(
        temp_root(&linux.clone().with_var("TMPDIR", "/var/tmp/")),
        PathBuf::from("/var/tmp")
    );
    assert_eq!(temp_root(&linux.with_var("TMP", "/scratch")), PathBuf::from("/scratch"));

    let windows = Env::for_tests(r"C:\Users\u").with_os(OsKind::Windows);
    assert_eq!(temp_root(&windows), PathBuf::from(r"C:\Windows\Temp"));
    assert_eq!(
        temp_root(&windows.with_var("TEMP", r"C:\Users\u\AppData\Local\Temp\")),
        PathBuf::from(r"C:\Users\u\AppData\Local\Temp")
    );
}

#[test]
fn omp_folders_honor_profiles_xdg_data_and_the_session_dir_variable() {
    let home = session_home();
    let project = &home.project;
    let agent = home.home.join(".omp/agent/sessions");

    assert_eq!(Omp.sessions_dir(&home.env, project), agent.join("-Projects-demo"));

    let profile = home.env.clone().with_var("OMP_PROFILE", "work");
    assert_eq!(
        Omp.sessions_dir(&profile, project),
        home.home.join(".omp/profiles/work/agent/sessions/-Projects-demo")
    );

    // XDG data applies on Linux and macOS only, and a Windows temp path cannot pass for a Linux one.
    if cfg!(unix) {
        let xdg = home.home.join("xdg");
        fs::create_dir_all(xdg.join("omp")).unwrap();
        let with_xdg = home
            .env
            .clone()
            .with_os(OsKind::Linux)
            .with_var("XDG_DATA_HOME", xdg.to_string_lossy());
        assert_eq!(
            Omp.sessions_dir(&with_xdg, project),
            xdg.join("omp/sessions/-Projects-demo")
        );
    }

    let custom = home
        .env
        .clone()
        .with_var("PI_CODING_AGENT_SESSION_DIR", "sessions-here");
    assert_eq!(Omp.sessions_dir(&custom, project), project.join("sessions-here"));
}

#[test]
fn claude_slugs_replace_every_character_but_ascii_letters_and_digits() {
    let cases = [
        ("/home/u/.config/x", "-home-u--config-x"),
        (
            "/home/u/Projekty/harness-useful-desktop",
            "-home-u-Projekty-harness-useful-desktop",
        ),
        ("/w/my_app v2", "-w-my-app-v2"),
        ("/w/zażółć", "-w-za----"),
        (r"C:\Users\u\dev\app", "C--Users-u-dev-app"),
    ];

    for (project, expected) in cases {
        assert_eq!(project_slug(std::path::Path::new(project)), expected, "{project}");
    }
}

#[test]
fn claude_folders_live_below_the_config_dir_and_follow_its_variable() {
    let home = session_home();
    let project = &home.project;
    let slug = project_slug(project);

    assert_eq!(
        Claude.sessions_dir(&home.env, project),
        home.home.join(".claude/projects").join(&slug)
    );

    let custom = home.home.join("claude-work");
    let moved = home.env.clone().with_var("CLAUDE_CONFIG_DIR", custom.to_string_lossy());
    assert_eq!(
        Claude.sessions_dir(&moved, project),
        custom.join("projects").join(&slug)
    );

    let relative = home.env.clone().with_var("CLAUDE_CONFIG_DIR", "claude-work");
    assert_eq!(
        Claude.sessions_dir(&relative, project),
        home.home.join(".claude/projects").join(&slug)
    );
}

#[test]
fn projects_in_the_temp_root_use_the_tmp_prefix() {
    let home = session_home();
    let in_temp = home.home.join("tmp/scratch");
    fs::create_dir_all(&in_temp).unwrap();

    assert_eq!(
        Omp.sessions_dir(&home.env, &in_temp),
        home.home.join(".omp/agent/sessions/-tmp-scratch")
    );
}

#[test]
fn pi_folders_prefer_the_variable_then_project_then_user_setting() {
    let home = session_home();
    let project = &home.project;
    let encoded = pi_dir_name(&project.to_string_lossy());

    assert_eq!(
        Pi.sessions_dir(&home.env, project),
        home.home.join(".pi/agent/sessions").join(encoded)
    );

    fs::create_dir_all(home.home.join(".pi/agent")).unwrap();
    fs::write(
        home.home.join(".pi/agent/settings.json"),
        r#"{ "sessionDir": "~/pi-sessions" }"#,
    )
    .unwrap();
    assert_eq!(Pi.sessions_dir(&home.env, project), home.home.join("pi-sessions"));

    fs::create_dir_all(project.join(".pi")).unwrap();
    fs::write(project.join(".pi/settings.json"), r#"{ "sessionDir": ".sessions" }"#).unwrap();
    assert_eq!(Pi.sessions_dir(&home.env, project), project.join(".sessions"));

    let absolute = home.home.join("data/pi");
    let custom = home
        .env
        .clone()
        .with_var("PI_CODING_AGENT_SESSION_DIR", absolute.to_string_lossy());
    assert_eq!(Pi.sessions_dir(&custom, project), absolute);
}

#[test]
fn only_a_projects_own_session_dir_setting_counts_as_configured() {
    let home = session_home();
    let project = &home.project;
    let text = |path: PathBuf| path.to_string_lossy().into_owned();

    // OMP has no project-level setting, not even with the user-level variable set.
    let omp = Omp.session_folder(&home.env, project);
    assert_eq!(omp.path, text(Omp.sessions_dir(&home.env, project)));
    assert!(!omp.configured);

    let variable = home.env.clone().with_var("PI_CODING_AGENT_SESSION_DIR", "elsewhere");
    assert!(!Omp.session_folder(&variable, project).configured);

    // PI: the user's setting is not the project's.
    fs::create_dir_all(home.home.join(".pi/agent")).unwrap();
    fs::write(
        home.home.join(".pi/agent/settings.json"),
        r#"{ "sessionDir": "~/pi-sessions" }"#,
    )
    .unwrap();
    let user = Pi.session_folder(&home.env, project);
    assert_eq!(user.path, text(home.home.join("pi-sessions")));
    assert!(!user.configured);

    fs::create_dir_all(project.join(".pi")).unwrap();
    fs::write(project.join(".pi/settings.json"), r#"{ "sessionDir": ".sessions" }"#).unwrap();
    let own = Pi.session_folder(&home.env, project);
    assert_eq!(own.path, text(project.join(".sessions")));
    assert!(own.configured);

    // The variable outranks the project's setting.
    let overridden = Pi.session_folder(&variable, project);
    assert_eq!(overridden.path, text(project.join("elsewhere")));
    assert!(!overridden.configured);
}

#[cfg(unix)]
#[test]
fn a_projects_symlinked_session_dir_shows_its_real_folder() {
    let home = session_home();
    let project = &home.project;
    let target = home.home.join(".pi/agent/sessions/--other-project--");
    fs::create_dir_all(&target).unwrap();
    std::os::unix::fs::symlink(&target, project.join("chats")).unwrap();

    fs::create_dir_all(project.join(".pi")).unwrap();
    fs::write(project.join(".pi/settings.json"), r#"{ "sessionDir": "chats" }"#).unwrap();

    let folder = Pi.session_folder(&home.env, project);
    // The temporary home may itself sit behind a link (macOS /var), so compare with the resolved target.
    assert_eq!(folder.path, dunce::canonicalize(&target).unwrap().to_string_lossy());
    assert!(folder.configured);

    // Listing and deleting still use the configured folder.
    assert_eq!(Pi.sessions_dir(&home.env, project), project.join("chats"));
}
