//! Session folder names and locations for OMP and PI on Linux, macOS and Windows.

mod session_support;

use std::fs;
use std::path::PathBuf;

use harness_business::env::{Env, OsKind};
use harness_business::harness::Harness;
use harness_business::harnesses::{Omp, Pi};
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

    let custom = home
        .env
        .clone()
        .with_var("PI_CODING_AGENT_SESSION_DIR", "sessions-here");
    assert_eq!(Omp.sessions_dir(&custom, project), project.join("sessions-here"));
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
