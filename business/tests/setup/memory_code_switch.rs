//! The code index switch file `code.toml` as the desktop reads it before each prompt, without
//! starting a process: a missing file or key is on, anything it cannot vouch for is off, and
//! `HARNESS_MEMORY_CODE_INDEX` wins over the file.

use harness_business::ecosystem::code_switch_is_on;
use harness_business::env::Env;
use tempfile::TempDir;

/// An environment whose server data folder is `data`, with `extra` variables set.
fn switch_env(data: &TempDir, extra: &[(&str, &str)]) -> Env {
    let mut env =
        Env::for_tests(data.path().join("home")).with_var("HARNESS_MEMORY_DIR", data.path().to_string_lossy());

    for (name, value) in extra {
        env = env.with_var(name, *value);
    }

    env
}

fn switch_file(data: &TempDir, text: &str) {
    std::fs::write(data.path().join("code.toml"), text).unwrap();
}

#[test]
fn the_switch_file_decides_without_a_process() {
    let data = tempfile::tempdir().unwrap();
    let env = switch_env(&data, &[]);

    assert!(code_switch_is_on(&env), "a missing file is on");

    switch_file(&data, "enabled = false\n");
    assert!(!code_switch_is_on(&env));

    switch_file(&data, "enabled = true\n");
    assert!(code_switch_is_on(&env));

    switch_file(&data, "# nothing set\n");
    assert!(code_switch_is_on(&env), "a file without the key is on");
}

#[test]
fn a_switch_file_nobody_can_vouch_for_is_off() {
    let data = tempfile::tempdir().unwrap();
    let env = switch_env(&data, &[]);

    for text in ["enabled = [", "enabled = \"yes\"\n", "enabled = 1\n"] {
        switch_file(&data, text);
        assert!(!code_switch_is_on(&env), "{text:?}");
    }

    switch_file(&data, &format!("enabled = true\n# {}\n", "x".repeat(8 * 1024)));
    assert!(!code_switch_is_on(&env), "a file over the cap");

    std::fs::remove_file(data.path().join("code.toml")).unwrap();
    std::fs::create_dir(data.path().join("code.toml")).unwrap();
    assert!(!code_switch_is_on(&env), "a folder is not a switch file");
}

#[test]
fn the_environment_variable_wins_over_the_switch_file() {
    let data = tempfile::tempdir().unwrap();

    switch_file(&data, "enabled = true\n");
    assert!(!code_switch_is_on(&switch_env(
        &data,
        &[("HARNESS_MEMORY_CODE_INDEX", "0")]
    )));

    switch_file(&data, "enabled = false\n");
    assert!(code_switch_is_on(&switch_env(
        &data,
        &[("HARNESS_MEMORY_CODE_INDEX", "1")]
    )));
}
