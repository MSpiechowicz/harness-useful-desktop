//! How a local OpenCode plugin's id is compared: the same file can be written with `/` or `\`
//! separators, a `\\?\` prefix or a lower-case drive letter, as Windows paths often are.

/// The path of a `file://` URL on Windows: `file:///C:/x` holds `/C:/x`, which is `C:/x`.
pub(super) fn file_url_drive_path(path: &str) -> &str {
    match path.strip_prefix('/') {
        Some(rest) if drive_root(rest).is_some() => rest,
        _ => path,
    }
}

/// True when the plugin id `id` names the plugin `name`: the same string, or the same local file
/// written another way (see [`path_key`]), as Windows paths often are.
pub(super) fn same_plugin_id(id: &str, name: &str) -> bool {
    if id == name {
        return true;
    }

    match (path_key(id), path_key(name)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

/// An absolute file path (`/x`, `C:\x`, `\\server\vol\x`) in one form, whatever the OS: `/`
/// separators, no `\\?\` verbatim prefix, an upper-case drive letter, no empty or `.` segments.
/// `None` for anything else (an npm spec).
fn path_key(id: &str) -> Option<String> {
    let unified = id.replace('\\', "/");
    let unified = match unified.strip_prefix("//?/") {
        Some(rest) => match rest.strip_prefix("UNC/") {
            Some(unc) => format!("//{unc}"),
            None => rest.to_owned(),
        },
        None => unified,
    };

    let (root, rest) = if let Some(rest) = unified.strip_prefix("//") {
        ("//".to_owned(), rest)
    } else if let Some(rest) = unified.strip_prefix('/') {
        ("/".to_owned(), rest)
    } else {
        let drive = drive_root(&unified)?;
        (format!("{}:/", drive.to_ascii_uppercase()), &unified[3..])
    };

    let segments: Vec<&str> = rest
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect();

    Some(format!("{root}{}", segments.join("/")))
}

/// The drive letter of a path that starts with `C:/` or `C:\`.
fn drive_root(path: &str) -> Option<char> {
    let mut chars = path.chars();
    let letter = chars.next().filter(char::is_ascii_alphabetic)?;
    let rooted = chars.next() == Some(':') && matches!(chars.next(), Some('/' | '\\'));

    rooted.then_some(letter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugin_file_matches_any_form_of_its_path() {
        let scanned = r"C:\me\plugins\demo.ts";

        assert!(same_plugin_id(scanned, r"C:\me/plugins/demo.ts"));
        assert!(same_plugin_id(scanned, "c:/me/plugins/demo.ts"));
        assert!(same_plugin_id(scanned, r"\\?\C:\me\.\plugins\demo.ts"));
        assert!(same_plugin_id(r"\\host\vol\a.ts", r"\\?\UNC\host\vol\a.ts"));
        assert!(same_plugin_id("/home/me/a.ts", "/home/me//./a.ts"));

        assert!(!same_plugin_id(scanned, r"D:\me\plugins\demo.ts"));
        assert!(!same_plugin_id(scanned, r"C:\me\plugins\other.ts"));
        assert!(!same_plugin_id("/home/me/a.ts", "home/me/a.ts"));
    }

    #[test]
    fn npm_ids_match_only_themselves() {
        assert!(same_plugin_id("@acme/plugin", "@acme/plugin"));
        assert!(!same_plugin_id("@acme/plugin", "@acme\\plugin"));
        assert!(!same_plugin_id("github:a/b", "github:a\\b"));
    }

    #[test]
    fn a_windows_file_url_drops_the_slash_before_the_drive() {
        assert_eq!(file_url_drive_path("/C:/x/demo.ts"), "C:/x/demo.ts");
        assert_eq!(file_url_drive_path(r"/c:\x\demo.ts"), r"c:\x\demo.ts");
        assert_eq!(file_url_drive_path("/home/x/demo.ts"), "/home/x/demo.ts");
        assert_eq!(file_url_drive_path("//server/vol/demo.ts"), "//server/vol/demo.ts");
    }
}
