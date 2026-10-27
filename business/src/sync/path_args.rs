//! Arguments of an MCP server that hold a path of this machine (`--image-dir=/home/me/x`,
//! `--image-dir /home/me/x`): left out of an export ([`without_machine_paths`]). An import compares
//! arguments exactly, so an existing entry with its own path differs from the shared server
//! (`differentDefinition`) and may be replaced.

use super::validate::is_local_path;
use crate::model::SyncExcludedReason;

/// The ends of an option name (`--image-dir`) that say the value after it is a path.
const PATH_OPTIONS: &[&str] = &[
    "dir",
    "directory",
    "path",
    "file",
    "folder",
    "root",
    "home",
    "cwd",
    "config",
];

/// True for `--<name>` without a value of its own, where `<name>` ends with one of
/// [`PATH_OPTIONS`] (lower case or not).
fn takes_path(arg: &str) -> bool {
    let name = arg.trim_start_matches('-').to_ascii_lowercase();

    arg.starts_with("--") && !arg.contains('=') && PATH_OPTIONS.iter().any(|end| name.ends_with(end))
}

/// `args` without the arguments that hold a path of this machine, and whether any was dropped. The
/// server then runs with the program's own default for that option on the machine that imports it.
///
/// - A long option with its own value (`--image-dir=/home/me/x`) is dropped alone. Any other
///   argument with `=` and a path (`KEY=/p`, `-v=/p`, `-c=/p`) leaves the server out.
/// - A path on its own is dropped with the option before it only when that is `--<name>` with a
///   name ending in `dir`, `directory`, `path`, `file`, `folder`, `root`, `home`, `cwd` or `config`.
///
/// Any other path (a positional one, one after a flag that takes no path, the script a program
/// runs) cannot be dropped without changing what the server does: the server is left out
/// ([`SyncExcludedReason::LocalPath`]).
pub(super) fn without_machine_paths(args: Vec<String>) -> Result<(Vec<String>, bool), SyncExcludedReason> {
    let mut kept: Vec<String> = Vec::with_capacity(args.len());
    let mut trimmed = false;

    for arg in args {
        if !is_local_path(&arg) {
            kept.push(arg);
            continue;
        }

        trimmed = true;

        if arg.contains('=') {
            if arg.starts_with("--") {
                continue;
            }

            return Err(SyncExcludedReason::LocalPath);
        }

        if kept.last().is_some_and(|flag| takes_path(flag)) {
            kept.pop();
            continue;
        }

        return Err(SyncExcludedReason::LocalPath);
    }

    Ok((kept, trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn an_option_with_its_own_value_is_dropped_alone() {
        assert_eq!(
            without_machine_paths(words(&["--stdio", "--image-dir=/home/me/x"])),
            Ok((words(&["--stdio"]), true))
        );
    }

    #[test]
    fn a_path_after_a_path_option_goes_with_it() {
        assert_eq!(
            without_machine_paths(words(&["-y", "pkg", "--Config-File", "/home/me/c.json", "--quiet"])),
            Ok((words(&["-y", "pkg", "--quiet"]), true))
        );
    }

    #[test]
    fn any_other_path_leaves_the_server_out() {
        for args in [
            &["--read-only", "/p"][..],
            &["node", "/abs/x.js"][..],
            &["-d", "/p"][..],
            &["/abs/x.js"][..],
            &["--stdio", "/p"][..],
            &["KEY=/home/alice/x"][..],
            &["-v=/home/alice/x"][..],
            &["-c=/home/alice/c.json"][..],
        ] {
            assert_eq!(
                without_machine_paths(words(args)),
                Err(SyncExcludedReason::LocalPath),
                "{args:?}"
            );
        }
    }
}
