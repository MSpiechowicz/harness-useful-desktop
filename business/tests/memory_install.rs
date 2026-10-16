//! Installing the app's stable copy of the bundled memory server: a fresh copy, an identical one
//! left alone, a different one replaced, and destinations it refuses.

use std::path::{Path, PathBuf};

use harness_business::BusinessError;
use harness_business::ecosystem::{InstalledBinary, install_memory_binary, memory_binary_name};
use tempfile::TempDir;

struct Dirs {
    root: TempDir,
}

impl Dirs {
    fn new() -> Self {
        Self {
            root: tempfile::tempdir().unwrap(),
        }
    }

    /// The bundled server, holding `bytes`.
    fn source(&self, bytes: &[u8]) -> PathBuf {
        let path = self.root.path().join("bundle").join(memory_binary_name());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        path
    }

    /// The install folder (not created).
    fn bin(&self) -> PathBuf {
        self.root.path().join("home/.local/bin")
    }

    fn installed(&self) -> PathBuf {
        self.bin().join(memory_binary_name())
    }

    fn install(&self, source: &Path) -> harness_business::Result<InstalledBinary> {
        install_memory_binary(source, &self.bin())
    }

    fn bin_files(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.bin())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

#[test]
fn a_fresh_install_creates_the_folder_and_an_executable_copy() {
    let dirs = Dirs::new();
    let source = dirs.source(b"server v1");

    let installed = dirs.install(&source).unwrap();

    assert_eq!(
        installed,
        InstalledBinary {
            path: dirs.installed(),
            changed: true
        }
    );
    assert_eq!(std::fs::read(dirs.installed()).unwrap(), b"server v1");
    assert_eq!(
        dirs.bin_files(),
        vec![memory_binary_name()],
        "no temporary file is left behind"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dirs.installed()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
    }
}

#[test]
fn an_identical_copy_is_left_untouched() {
    let dirs = Dirs::new();
    let source = dirs.source(b"server v1");
    dirs.install(&source).unwrap();
    let before = std::fs::metadata(dirs.installed()).unwrap().modified().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(20));
    let installed = dirs.install(&source).unwrap();

    assert!(!installed.changed);
    assert_eq!(std::fs::metadata(dirs.installed()).unwrap().modified().unwrap(), before);
}

#[test]
fn a_different_copy_is_replaced() {
    let dirs = Dirs::new();
    dirs.install(&dirs.source(b"server v1")).unwrap();

    // Same length, other bytes.
    let installed = dirs.install(&dirs.source(b"server v2")).unwrap();

    assert!(installed.changed);
    assert_eq!(std::fs::read(dirs.installed()).unwrap(), b"server v2");
    assert_eq!(dirs.bin_files(), vec![memory_binary_name()]);
}

#[cfg(windows)]
#[test]
fn leftovers_of_an_earlier_windows_replacement_are_removed() {
    let dirs = Dirs::new();
    std::fs::create_dir_all(dirs.bin()).unwrap();
    std::fs::write(dirs.bin().join("harness-memory.old-123.exe"), "old").unwrap();
    std::fs::write(dirs.bin().join("unrelated"), "keep").unwrap();

    dirs.install(&dirs.source(b"server")).unwrap();

    assert_eq!(dirs.bin_files(), vec![memory_binary_name(), "unrelated".to_owned()]);
}

#[cfg(not(windows))]
#[test]
fn files_that_look_like_windows_leftovers_are_kept_elsewhere() {
    let dirs = Dirs::new();
    std::fs::create_dir_all(dirs.bin()).unwrap();
    std::fs::write(dirs.bin().join("harness-memory.old-123.exe"), "someone's file").unwrap();
    std::fs::write(dirs.bin().join("harness-memory.old-backup"), "another").unwrap();

    dirs.install(&dirs.source(b"server")).unwrap();

    assert_eq!(
        dirs.bin_files(),
        vec![
            memory_binary_name(),
            "harness-memory.old-123.exe".to_owned(),
            "harness-memory.old-backup".to_owned()
        ]
    );
}

#[test]
fn a_missing_source_is_not_found_and_writes_nothing() {
    let dirs = Dirs::new();

    let error = dirs
        .install(&dirs.root.path().join("nowhere/harness-memory"))
        .unwrap_err();

    assert!(matches!(error, BusinessError::NotFound(_)), "{error:?}");
    assert!(!dirs.bin().exists());
}

#[test]
fn a_destination_that_is_not_a_regular_file_is_refused() {
    let dirs = Dirs::new();
    std::fs::create_dir_all(dirs.installed()).unwrap();

    let error = dirs.install(&dirs.source(b"server")).unwrap_err();

    assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");
    assert!(dirs.installed().is_dir());
}

#[cfg(unix)]
#[test]
fn a_symlinked_destination_is_refused_and_its_target_untouched() {
    let dirs = Dirs::new();
    let elsewhere = dirs.root.path().join("elsewhere");
    std::fs::write(&elsewhere, "someone else's").unwrap();
    std::fs::create_dir_all(dirs.bin()).unwrap();
    std::os::unix::fs::symlink(&elsewhere, dirs.installed()).unwrap();

    let error = dirs.install(&dirs.source(b"server")).unwrap_err();

    assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");
    assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), "someone else's");
    assert!(
        std::fs::symlink_metadata(dirs.installed())
            .unwrap()
            .file_type()
            .is_symlink()
    );
}
