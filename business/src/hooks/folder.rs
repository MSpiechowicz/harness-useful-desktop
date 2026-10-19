//! The hooks folder on disk: the folder handling of [`crate::signed_docs`] (reading the
//! `<id>.json` files and their entries, creating the folder, and replacing one file while it still
//! holds the version a writer read) for [`HookStore`](super::HookStore).

#[cfg(test)]
mod tests {
    use crate::error::BusinessError;
    use crate::hooks::sha256_hex;
    use crate::hooks::store::HookStore;

    /// A write whose file changed between its read and its rename is refused at the rename.
    #[test]
    fn a_write_checks_the_version_right_before_its_rename() {
        let temp = tempfile::tempdir().unwrap();
        let store = HookStore::new(temp.path().join("root"), temp.path().join("config"));
        store.ensure_dirs().unwrap();

        let id = "0".repeat(32);
        let first = store.write_file(&id, b"first", None).unwrap();

        // Someone else replaced the file after this writer read `first`.
        std::fs::write(store.file_path(&id), b"theirs").unwrap();

        let stale = store.write_file(&id, b"mine", Some(&first));
        assert!(matches!(stale, Err(BusinessError::Conflict(_))), "{stale:?}");

        let created = store.write_file(&id, b"mine", None);
        assert!(matches!(created, Err(BusinessError::Conflict(_))), "{created:?}");

        assert_eq!(std::fs::read(store.file_path(&id)).unwrap(), b"theirs");
        assert_eq!(
            std::fs::read_dir(store.hooks_dir()).unwrap().count(),
            1,
            "no temporary file is left"
        );

        let current = sha256_hex(b"theirs");
        assert_eq!(
            store.write_file(&id, b"mine", Some(&current)).unwrap(),
            sha256_hex(b"mine")
        );
    }
}
