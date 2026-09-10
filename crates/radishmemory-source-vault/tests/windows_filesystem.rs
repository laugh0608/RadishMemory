//! Windows filesystem boundaries; all paths and payloads are isolated synthetic fixtures.
// Cargo passes the package's direct dependencies to integration test crates on every platform.
use aead_stream as _;
use chacha20poly1305 as _;
use getrandom as _;
use radishmemory_source_vault as _;
use sha2 as _;
use zeroize as _;

#[cfg(windows)]
mod windows {
    use radishmemory_source_vault::{
        KeyEncryptionKey, ObjectDirectory, ObjectMetadata, ObjectWrite, SourceVaultErrorCode,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    struct Root(PathBuf);
    impl Root {
        fn new() -> Self {
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "radishmemory-s03b-public-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn windows_directory_capabilities_prevent_rename_until_dropped() {
        let root = Root::new();
        let vault = ObjectDirectory::open_application_directory(&root.0).unwrap();
        for path in [
            root.0.clone(),
            root.0.join("source-objects-v1"),
            root.0.join("source-staging-v1"),
        ] {
            let moved = path.with_extension("moved");
            assert_eq!(
                fs::rename(&path, &moved).unwrap_err().raw_os_error(),
                Some(32)
            );
            assert!(!moved.exists());
        }
        drop(vault);
        let path = root.0.join("source-staging-v1");
        let moved = path.with_extension("moved");
        fs::rename(&path, &moved).unwrap();
        fs::rename(&moved, &path).unwrap();
    }
    #[test]
    #[ignore = "requires Windows symlink creation privilege; run explicitly in an authorized elevated test session"]
    fn windows_reparse_and_nonregular_entries_fail_closed() {
        use std::os::windows::fs::{symlink_dir, symlink_file};
        let root = Root::new();
        let outside = Root::new();
        let link = root.0.join("application-link");
        symlink_dir(&outside.0, &link).unwrap();
        assert_eq!(
            ObjectDirectory::open_application_directory(&link)
                .unwrap_err()
                .code(),
            SourceVaultErrorCode::InvalidDirectory
        );
        fs::remove_dir(&link).unwrap();
        for name in ["source-objects-v1", "source-staging-v1"] {
            let path = root.0.join(name);
            if path.exists() {
                fs::remove_dir(&path).unwrap();
            }
            symlink_dir(&outside.0, &path).unwrap();
            assert_eq!(
                ObjectDirectory::open_application_directory(&root.0)
                    .unwrap_err()
                    .code(),
                SourceVaultErrorCode::InvalidDirectory
            );
            fs::remove_dir(&path).unwrap();
        }
        let vault = ObjectDirectory::open_application_directory(&root.0).unwrap();
        let digest = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        let metadata = ObjectMetadata::new(
            "synthetic-namespace",
            "synthetic-source",
            digest,
            0,
            "text/plain",
        )
        .unwrap();
        let key = KeyEncryptionKey::new([0xa5; 32]);
        let write = ObjectWrite::seal(&key, &metadata, b"").unwrap();
        let marker = outside.0.join("marker");
        fs::write(&marker, b"synthetic outside marker").unwrap();
        let target = root
            .0
            .join("source-objects-v1")
            .join(format!("{}.rmo", write.locator().token()));
        symlink_file(&marker, &target).unwrap();
        assert_eq!(
            vault.publish(&write, &key).unwrap_err().code(),
            SourceVaultErrorCode::InvalidFile
        );
        assert_eq!(
            vault
                .read(write.locator(), &metadata, &key)
                .unwrap_err()
                .code(),
            SourceVaultErrorCode::InvalidFile
        );
        fs::remove_file(&target).unwrap();
        let stage = root.0.join("source-staging-v1").join(format!(
            "{}.{}.stage",
            write.locator().token(),
            write.attempt_id().token()
        ));
        symlink_file(&marker, &stage).unwrap();
        assert_eq!(
            vault.publish(&write, &key).unwrap_err().code(),
            SourceVaultErrorCode::ObjectExists
        );
        fs::remove_file(&stage).unwrap();
        fs::create_dir(&target).unwrap();
        assert_eq!(
            vault.publish(&write, &key).unwrap_err().code(),
            SourceVaultErrorCode::InvalidFile
        );
        assert_eq!(fs::read(&marker).unwrap(), b"synthetic outside marker");
    }
}
