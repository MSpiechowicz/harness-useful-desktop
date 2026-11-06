//! A look at guards that are already current does not take the lock, so a second desktop that holds
//! it does not hold the reads. Anything else still waits for it.

use std::fs;
use std::time::{Duration, Instant};

use pretty_assertions::assert_eq;

use super::hardening::V1;
use super::*;
use crate::ecosystem::memory_guards::lock::FileLock;
use crate::env::Env;

/// How long the lock is waited for at most.
const WAIT: Duration = Duration::from_secs(5);

#[test]
fn current_guards_are_read_at_once_while_another_process_holds_the_lock() {
    let (_dir, data, _repo) = setup();
    ensure_global(&data).unwrap();
    let before = fs::read(global_path(&data)).unwrap();
    let _held = FileLock::take(&data.join(GUARDS_DIR), Duration::from_secs(1)).unwrap();
    let started = Instant::now();

    ensure_global(&data).unwrap();
    let shown = read_global_healed(&Env::for_tests(data.parent().unwrap()), &data).unwrap();

    assert!(started.elapsed() < Duration::from_secs(1), "{:?}", started.elapsed());
    assert_eq!(shown, MemoryGuards::default());
    assert_eq!(fs::read(global_path(&data)).unwrap(), before);
}

/// Runs [`ensure_global`] on a data folder whose `global.json` holds `file` while another process
/// holds the lock: it waits for the lock, reports it taken and leaves the file as it was.
fn waits_for_the_lock_and_reports_it(file: &str) {
    let (_dir, data, _repo) = setup();
    ensure_global(&data).unwrap();
    fs::write(global_path(&data), file).unwrap();
    let _held = FileLock::take(&data.join(GUARDS_DIR), Duration::from_secs(1)).unwrap();
    let started = Instant::now();

    let result = ensure_global(&data);

    assert!(matches!(result, Err(BusinessError::Protocol(_))), "{result:?}");
    assert!(
        started.elapsed() >= WAIT - Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(fs::read_to_string(global_path(&data)).unwrap(), file, "left as it was");
}

#[test]
fn version_one_still_waits_for_the_lock_and_then_reports_it() {
    waits_for_the_lock_and_reports_it(V1);
}

#[test]
fn a_file_that_cannot_be_read_does_not_take_the_quick_way() {
    waits_for_the_lock_and_reports_it("not json");
}

#[cfg(unix)]
#[test]
fn a_guards_folder_that_is_not_closed_to_others_is_narrowed_even_when_the_files_are_current() {
    use std::os::unix::fs::PermissionsExt;

    let (_dir, data, _repo) = setup();
    ensure_global(&data).unwrap();
    let guards = data.join(GUARDS_DIR);
    fs::set_permissions(&guards, fs::Permissions::from_mode(0o750)).unwrap();

    ensure_global(&data).unwrap();

    assert_eq!(mode(&guards), 0o700);
}
