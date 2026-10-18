//! The host environment for children of an AppImage.
//!
//! The AppImage launcher points `LD_LIBRARY_PATH`, `PATH`, `PYTHONHOME`, `PYTHONPATH`, `XDG_DATA_DIRS`,
//! the GTK/GIO module paths and more into the mounted image, so the app finds its bundled libraries.
//! Children are host programs (the agent, its extensions, the commands it runs, git): with that
//! environment they load the image's libraries, or a Python home that is not theirs, and fail. The
//! launcher keeps no copy of the original values, but it only prepends to the inherited path lists,
//! so dropping every entry inside the image restores them.
//!
//! An AppImage started from another one (the updater relaunching the new version from the old
//! one, or one Harness app opening another) inherits that one's entries behind its own: they point
//! into a mount that may already be gone, or hold older libraries (`node` then fails on the
//! image's `libffi`). So entries inside any AppImage mount beside ours (`.mount_*` in the same
//! folder, where the runtime mounts every image) are dropped too.

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
    let mounts = appdir.parent();

    vars.iter()
        .filter_map(|(name, value)| {
            if LAUNCHER_ONLY.iter().any(|launcher| name == *launcher) {
                return Some((name.clone(), None));
            }

            let entries: Vec<&[u8]> = value.as_bytes().split(|byte| *byte == b':').collect();
            let inside = |entry: &&[u8]| in_image(Path::new(OsStr::from_bytes(entry)), appdir, mounts);
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

/// True when `entry` is inside `appdir`, or inside another AppImage mount in `mounts` (the folder
/// that holds `appdir`): a `.mount_*` folder there.
fn in_image(entry: &Path, appdir: &Path, mounts: Option<&Path>) -> bool {
    if entry.starts_with(appdir) {
        return true;
    }

    let Some(rest) = mounts.and_then(|mounts| entry.strip_prefix(mounts).ok()) else {
        return false;
    };

    rest.components()
        .next()
        .is_some_and(|first| first.as_os_str().as_bytes().starts_with(b".mount_"))
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
    fn strips_the_mounts_of_the_appimage_that_started_this_one() {
        // The updater relaunched this image from the old one: the old mount's entries follow ours.
        let old = "/tmp/.mount_harnesOld";
        let libs = format!("{APPDIR}/usr/lib/:{old}/usr/lib/:{old}/lib64/:/opt/cuda/lib64");
        let path = format!("{APPDIR}/usr/bin/:{old}/usr/bin/:/usr/bin");

        assert_eq!(
            launched(&[("LD_LIBRARY_PATH", &libs), ("PATH", &path), ("OWD", old)]),
            [
                set("LD_LIBRARY_PATH", "/opt/cuda/lib64"),
                set("PATH", "/usr/bin"),
                removed("OWD"),
            ]
        );
    }

    #[test]
    fn keeps_other_folders_beside_the_mount() {
        assert!(launched(&[("LD_LIBRARY_PATH", "/tmp/mylibs:/tmp/.cache/lib")]).is_empty());
    }

    #[test]
    fn leaves_the_users_own_variables_alone() {
        assert!(launched(&[("HOME", "/home/user"), ("PATH", "/usr/bin:/bin"), ("EDITOR", "nvim")]).is_empty());
    }
}
