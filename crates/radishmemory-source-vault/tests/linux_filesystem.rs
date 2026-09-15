//! Linux public API acceptance using only isolated synthetic files and existing dependencies.
use aead_stream as _;
#[cfg(target_os = "macos")]
use apple_native_keyring_store as _;
use chacha20poly1305 as _;
use getrandom as _;
use keyring_core as _;
use radishmemory_source_vault as _;
#[cfg(windows)]
use radishmemory_windows_filesystem as _;
#[cfg(target_os = "linux")]
use secret_service as _;
#[cfg(target_os = "macos")]
use security_framework as _;
use sha2 as _;
#[cfg(windows)]
use windows_native_keyring_store as _;
#[cfg(target_os = "linux")]
use zbus_secret_service_keyring_store as _;
use zeroize as _;

#[cfg(target_os = "linux")]
mod linux {
    use radishmemory_source_vault::{
        AttemptState, KeyEncryptionKey, ObjectDirectory, ObjectMetadata, ObjectWrite,
        SourceVaultError, SourceVaultErrorCode,
    };
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::io::ErrorKind;
    use std::os::unix::fs::{DirBuilderExt, FileTypeExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Root(PathBuf);
    impl Root {
        fn new() -> Self {
            // Root or DAC capabilities would invalidate permission-denial evidence.
            let status = fs::read_to_string("/proc/self/status").unwrap();
            let uid = status
                .lines()
                .find(|line| line.starts_with("Uid:"))
                .unwrap();
            assert!(uid.split_whitespace().skip(1).all(|value| value != "0"));
            let caps = status
                .lines()
                .find(|line| line.starts_with("CapEff:"))
                .unwrap();
            assert_eq!(
                u64::from_str_radix(caps.split_whitespace().nth(1).unwrap(), 16).unwrap(),
                0,
                "ordinary-user acceptance requires no effective capabilities"
            );
            static SEQ: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "radishmemory-s03b-linux-public-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }
    }
    impl Drop for Root {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    struct Permissions {
        path: PathBuf,
        previous: fs::Permissions,
    }
    impl Permissions {
        fn set(path: &Path, mode: u32) -> Self {
            let previous = fs::metadata(path).unwrap().permissions();
            fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
            Self {
                path: path.to_owned(),
                previous,
            }
        }
    }
    impl Drop for Permissions {
        fn drop(&mut self) {
            fs::set_permissions(&self.path, self.previous.clone()).unwrap();
        }
    }

    fn fixture() -> (KeyEncryptionKey, ObjectMetadata, ObjectWrite) {
        let bytes = b"synthetic Linux filesystem acceptance";
        let key = KeyEncryptionKey::new([0xa5; 32]);
        let metadata = ObjectMetadata::new(
            "synthetic-namespace",
            "synthetic-linux-source",
            Sha256::digest(bytes).into(),
            bytes.len() as u64,
            "text/plain",
        )
        .unwrap();
        let write = ObjectWrite::seal(&key, &metadata, bytes).unwrap();
        (key, metadata, write)
    }

    fn assert_denied(error: SourceVaultError) {
        assert_eq!(error.code(), SourceVaultErrorCode::Io);
        assert_eq!(error.io_kind(), Some(ErrorKind::PermissionDenied));
        assert_eq!(error.os_code(), Some(13));
    }

    #[test]
    fn ordinary_user_creation_and_read_revocation_fail_closed_and_recover() {
        let root = Root::new();
        let denied = Permissions::set(&root.0, 0o500);
        assert_denied(ObjectDirectory::open_application_directory(&root.0).unwrap_err());
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
        drop(denied);

        let vault = ObjectDirectory::open_application_directory(&root.0).unwrap();
        let (key, metadata, write) = fixture();
        let staging = root.0.join("source-staging-v1");
        let objects = root.0.join("source-objects-v1");
        let denied = Permissions::set(&staging, 0o500);
        assert_denied(vault.publish(&write, &key).unwrap_err());
        assert_eq!(fs::read_dir(&staging).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&objects).unwrap().count(), 0);
        assert_eq!(
            vault
                .inspect_attempt(write.locator(), write.attempt_id(), &metadata, &key)
                .unwrap(),
            AttemptState::Absent
        );
        drop(denied);

        vault.publish(&write, &key).unwrap();
        let object = objects.join(format!("{}.rmo", write.locator().token()));
        let before = fs::read(&object).unwrap();
        let denied = Permissions::set(&object, 0o000);
        assert_denied(vault.read(write.locator(), &metadata, &key).unwrap_err());
        assert_denied(
            vault
                .inspect_attempt(write.locator(), write.attempt_id(), &metadata, &key)
                .unwrap_err(),
        );
        drop(denied);
        assert!(fs::read(&object).unwrap() == before);
        drop(vault);
        let reopened = ObjectDirectory::open_application_directory(&root.0).unwrap();
        assert!(
            reopened.read(write.locator(), &metadata, &key).unwrap()
                == b"synthetic Linux filesystem acceptance"
        );
        assert_eq!(
            reopened
                .inspect_attempt(write.locator(), write.attempt_id(), &metadata, &key)
                .unwrap(),
            AttemptState::AuthenticatedPublishedCandidate
        );
    }

    #[test]
    fn fifo_object_and_staging_entries_are_rejected_without_consuming_them() {
        let root = Root::new();
        let vault = ObjectDirectory::open_application_directory(&root.0).unwrap();
        let (key, metadata, write) = fixture();
        let object = root
            .0
            .join("source-objects-v1")
            .join(format!("{}.rmo", write.locator().token()));
        let staging = root.0.join("source-staging-v1").join(format!(
            "{}.{}.stage",
            write.locator().token(),
            write.attempt_id().token()
        ));
        for path in [&object, &staging] {
            assert!(Command::new("mkfifo").arg(path).status().unwrap().success());
            assert!(fs::symlink_metadata(path).unwrap().file_type().is_fifo());
            assert_eq!(
                vault.publish(&write, &key).unwrap_err().code(),
                if path == &object {
                    SourceVaultErrorCode::InvalidFile
                } else {
                    SourceVaultErrorCode::ObjectExists
                }
            );
            assert_eq!(
                vault
                    .read(write.locator(), &metadata, &key)
                    .unwrap_err()
                    .code(),
                if path == &object {
                    SourceVaultErrorCode::InvalidFile
                } else {
                    SourceVaultErrorCode::ObjectMissing
                }
            );
            assert_eq!(
                vault
                    .inspect_attempt(write.locator(), write.attempt_id(), &metadata, &key)
                    .unwrap_err()
                    .code(),
                SourceVaultErrorCode::InvalidFile
            );
            assert!(fs::symlink_metadata(path).unwrap().file_type().is_fifo());
            fs::remove_file(path).unwrap();
        }
        assert_eq!(
            fs::read_dir(root.0.join("source-objects-v1"))
                .unwrap()
                .count(),
            0
        );
        assert_eq!(
            fs::read_dir(root.0.join("source-staging-v1"))
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    fn replacement_private_directories_do_not_inherit_open_capabilities() {
        for name in ["source-objects-v1", "source-staging-v1"] {
            let root = Root::new();
            let vault = ObjectDirectory::open_application_directory(&root.0).unwrap();
            let (key, metadata, write) = fixture();
            vault.publish(&write, &key).unwrap();
            let path = root.0.join(name);
            let retained = root.0.join("retained-directory");
            fs::rename(&path, &retained).unwrap();
            fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
            let marker = path.join("unknown-retained-file");
            fs::write(&marker, b"synthetic unknown marker").unwrap();
            assert_eq!(
                vault.publish(&write, &key).unwrap_err().code(),
                SourceVaultErrorCode::FilesystemChanged
            );
            assert_eq!(
                vault
                    .read(write.locator(), &metadata, &key)
                    .unwrap_err()
                    .code(),
                SourceVaultErrorCode::FilesystemChanged
            );
            assert_eq!(
                vault
                    .inspect_attempt(write.locator(), write.attempt_id(), &metadata, &key)
                    .unwrap_err()
                    .code(),
                SourceVaultErrorCode::FilesystemChanged
            );
            assert!(fs::read(&marker).unwrap() == b"synthetic unknown marker");
            assert_eq!(fs::read_dir(&path).unwrap().count(), 1);
            fs::remove_file(&marker).unwrap();
            fs::remove_dir(&path).unwrap();
            fs::rename(&retained, &path).unwrap();
            assert!(
                vault.read(write.locator(), &metadata, &key).unwrap()
                    == b"synthetic Linux filesystem acceptance"
            );
        }
    }
}
