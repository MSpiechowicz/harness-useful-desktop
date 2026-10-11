//! The host environment for children of an AppImage.
//!
//! The AppImage launcher points `LD_LIBRARY_PATH`, `PATH`, `PYTHONHOME`, `PYTHONPATH`, `XDG_DATA_DIRS`,
//! the GTK/GIO module paths and more into the mounted image, so the app finds its bundled libraries.
//! Children are host programs (the agent, its extensions, the commands it runs, git): with that
//! environment they load the image's libraries, or a Python home that is not theirs, and fail. The
//! launcher keeps no copy of the original values, but it only prepends to the inherited path lists,
//! so dropping every entry inside the image restores them.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

/// Set by the launcher with no inherited part to keep, so they are dropped outright.
const LAUNCHER_ONLY: &[&str] = &[
    "APPDIR",
    "APPIMAGE",
    "ARGV0",
    "OWD",
    "PYTHONDONTWRITEBYTECODE",
    "GTK_THEME",
];

/// The changes that give a child the host environment: `Some` sets a variable, `None` removes it.
/// Empty unless `vars` (the current environment) comes from an AppImage launcher.
pub(crate) fn host_env(vars: impl IntoIterator<Item = (OsString, OsString)>) -> Vec<(OsString, Option<OsString>)> {
    let vars: Vec<(OsString, OsString)> = vars.into_iter().collect();
    let get = |name: &str| vars.iter().find(|(key, _)| key == name).map(|(_, value)| value);

    // Both are set by the AppImage runtime; APPDIR alone could be anything.
    let (Some(appdir), Some(_)) = (get("APPDIR"), get("APPIMAGE")) else {
        return Vec::new();
    };
    let appdir = Path::new(appdir);
    if !appdir.is_absolute() {
        return Vec::new();
    }

    vars.iter()
        .filter_map(|(name, value)| {
            if LAUNCHER_ONLY.iter().any(|launcher| name == *launcher) {
                return Some((name.clone(), None));
            }

            let entries: Vec<&[u8]> = value.as_bytes().split(|byte| *byte == b':').collect();
            let inside = |entry: &&[u8]| Path::new(OsStr::from_bytes(entry)).starts_with(appdir);
            if !entries.iter().any(inside) {
                return None;
            }

            let kept: Vec<&[u8]> = entries
                .into_iter()
                .filter(|entry| !entry.is_empty() && !inside(entry))
                .collect();
            let restored = (!kept.is_empty()).then(|| OsString::from_vec(kept.join(&b':')));

            Some((name.clone(), restored))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const APPDIR: &str = "/tmp/.mount_harnesKF";

    fn vars(pairs: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        pairs
            .iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value)))
            .collect()
    }

    fn launched(extra: &[(&str, &str)]) -> Vec<(OsString, Option<OsString>)> {
        let mut pairs = vec![
            ("APPDIR", APPDIR),
            ("APPIMAGE", "/home/user/.local/bin/harness-desktop"),
        ];
        pairs.extend_from_slice(extra);
        let mut changes = host_env(vars(&pairs));
        changes.retain(|(name, _)| name != "APPDIR" && name != "APPIMAGE");
        changes
    }

    fn set(name: &str, value: &str) -> (OsString, Option<OsString>) {
        (name.into(), Some(value.into()))
    }

    fn removed(name: &str) -> (OsString, Option<OsString>) {
        (name.into(), None)
    }

    #[test]
    fn changes_nothing_outside_an_appimage() {
        assert!(host_env(vars(&[("PATH", "/usr/bin"), ("PYTHONHOME", "/opt/py")])).is_empty());
        // APPDIR without the runtime's APPIMAGE is someone else's variable.
        assert!(
            host_env(vars(&[
                ("APPDIR", APPDIR),
                ("PATH", &format!("{APPDIR}/usr/bin:/usr/bin"))
            ]))
            .is_empty()
        );
    }

    #[test]
    fn strips_the_image_from_prepended_path_lists() {
        let path = format!("{APPDIR}/usr/bin/:{APPDIR}/usr/sbin/:/usr/local/bin:/usr/bin");
        let libs = format!("{APPDIR}/usr/lib/:{APPDIR}/lib64/:/opt/cuda/lib64");

        assert_eq!(
            launched(&[("PATH", &path), ("LD_LIBRARY_PATH", &libs)]),
            [
                set("PATH", "/usr/local/bin:/usr/bin"),
                set("LD_LIBRARY_PATH", "/opt/cuda/lib64")
            ]
        );
    }

    #[test]
    fn removes_variables_that_only_point_into_the_image() {
        let python_path = format!("{APPDIR}/usr/share/pyshared/:");
        let gio = format!("{APPDIR}//usr/lib/gio/modules");

        assert_eq!(
            launched(&[
                ("PYTHONHOME", &format!("{APPDIR}/usr/")),
                ("PYTHONPATH", &python_path),
                ("GIO_MODULE_DIR", &gio),
                ("PYTHONDONTWRITEBYTECODE", "1"),
                ("GTK_THEME", "Adwaita:dark"),
            ]),
            [
                removed("PYTHONHOME"),
                removed("PYTHONPATH"),
                removed("GIO_MODULE_DIR"),
                removed("PYTHONDONTWRITEBYTECODE"),
                removed("GTK_THEME"),
            ]
        );
    }

    #[test]
    fn leaves_the_users_own_variables_alone() {
        assert!(launched(&[("HOME", "/home/user"), ("PATH", "/usr/bin:/bin"), ("EDITOR", "nvim")]).is_empty());
    }
}
