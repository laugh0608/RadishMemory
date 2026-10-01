use super::*;
use crate::crypto::seal_object_with_random;
use crate::test_support::{FixedRandom, TestDirectory, key, metadata};
use crate::{MAX_OBJECT_PLAINTEXT_BYTES, SEGMENT_PLAINTEXT_BYTES};
use std::io;

fn write(bytes: &[u8], seed: u8) -> ObjectWrite {
    let metadata = metadata(bytes);
    ObjectWrite::from_sealed(
        &metadata,
        seal_object_with_random(&key(), &metadata, bytes, &mut FixedRandom(seed)).unwrap(),
    )
    .unwrap()
}
fn directory(root: &TestDirectory) -> ObjectDirectory {
    ObjectDirectory::open_application_directory(&root.0).unwrap()
}
fn count(path: &std::path::Path) -> usize {
    fs::read_dir(path).unwrap().count()
}
fn injected_error() -> SourceVaultError {
    SourceVaultError::io(
        "synthetic filesystem fault",
        io::Error::from(io::ErrorKind::StorageFull),
    )
}

#[test]
fn publish_reopen_and_read_exact_bytes_at_all_size_boundaries() {
    for size in [
        0,
        37,
        SEGMENT_PLAINTEXT_BYTES,
        SEGMENT_PLAINTEXT_BYTES + 17,
        MAX_OBJECT_PLAINTEXT_BYTES,
    ] {
        let root = TestDirectory::new();
        let vault = directory(&root);
        let bytes: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        let write = write(&bytes, 0);
        let mut steps = Vec::new();
        let published = vault
            .publish_with_step(&write, &key(), |step| {
                steps.push(step);
                Ok(())
            })
            .unwrap();
        assert!(published.locator() == write.locator());
        assert!(published.attempt_id() == write.attempt_id());
        assert_eq!(
            steps,
            vec![
                Step::Create,
                Step::Write,
                Step::Flush,
                Step::FileSync,
                Step::StagingSync,
                Step::StagingReadBack,
                Step::Publish,
                Step::ObjectDirectorySync,
                Step::PublishedReadBack,
                Step::RemoveStaging,
                Step::CleanupSync
            ]
        );
        assert_eq!(count(&vault.objects.path), 1);
        assert_eq!(count(&vault.staging.path), 0);
        drop(vault);
        let reopened = directory(&root);
        let locator = ObjectLocator::from_token(write.locator().token()).unwrap();
        let attempt = AttemptId::from_token(write.attempt_id().token()).unwrap();
        assert!(reopened.read(&locator, &write.metadata, &key()).unwrap() == bytes);
        assert_eq!(
            reopened
                .inspect_attempt(&locator, &attempt, &write.metadata, &key())
                .unwrap(),
            AttemptState::AuthenticatedPublishedCandidate
        );
    }
}

#[test]
fn different_sources_with_equal_bytes_never_share_objects() {
    let root = TestDirectory::new();
    let vault = directory(&root);
    let first = write(b"synthetic shared bytes", 0);
    let other_metadata = ObjectMetadata::new(
        "namespace-synthetic",
        "other-source",
        *first.metadata.exact_digest(),
        first.metadata.plaintext_len(),
        "text/markdown",
    )
    .unwrap();
    let second = ObjectWrite::from_sealed(
        &other_metadata,
        seal_object_with_random(
            &key(),
            &other_metadata,
            b"synthetic shared bytes",
            &mut FixedRandom(1),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(first.locator != second.locator);
    vault.publish(&first, &key()).unwrap();
    vault.publish(&second, &key()).unwrap();
    assert_eq!(count(&vault.objects.path), 2);
    assert!(
        vault
            .read(second.locator(), &other_metadata, &key())
            .unwrap()
            == b"synthetic shared bytes"
    );
    assert_eq!(
        vault
            .read(first.locator(), &other_metadata, &key())
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::InvalidLocator
    );
}

#[test]
fn replay_and_existing_targets_are_never_overwritten() {
    let root = TestDirectory::new();
    let vault = directory(&root);
    let first = write(b"synthetic collision", 0);
    vault.publish(&first, &key()).unwrap();
    let original = fs::read(vault.object_path(first.locator())).unwrap();
    for candidate in [&first, &write(b"synthetic collision", 1)] {
        assert_eq!(
            vault.publish(candidate, &key()).unwrap_err().code(),
            SourceVaultErrorCode::ObjectExists
        );
        assert!(fs::read(vault.object_path(first.locator())).unwrap() == original);
        assert_eq!(count(&vault.staging.path), 0);
    }
}

#[test]
fn every_interrupted_publication_has_explicit_exact_attempt_state_after_reopen() {
    for fault in [
        Step::Create,
        Step::Write,
        Step::Flush,
        Step::FileSync,
        Step::StagingSync,
        Step::StagingReadBack,
        Step::Publish,
        Step::ObjectDirectorySync,
        Step::PublishedReadBack,
        Step::RemoveStaging,
        Step::CleanupSync,
    ] {
        let root = TestDirectory::new();
        let vault = directory(&root);
        let write = write(b"synthetic interruption", 0);
        let unrelated = vault.staging.path.join("unknown-retained-file");
        fs::write(&unrelated, b"synthetic unrelated ciphertext placeholder").unwrap();
        let error = vault
            .publish_with_step(&write, &key(), |step| {
                if step == fault {
                    Err(injected_error())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.io_kind(), Some(io::ErrorKind::StorageFull));
        drop(vault);
        let reopened = directory(&root);
        let state =
            reopened.inspect_attempt(write.locator(), write.attempt_id(), &write.metadata, &key());
        match fault {
            Step::Create => assert_eq!(state.unwrap(), AttemptState::Absent),
            Step::Write => assert_eq!(
                state.unwrap_err().code(),
                SourceVaultErrorCode::InvalidEnvelope
            ),
            Step::Flush
            | Step::FileSync
            | Step::StagingSync
            | Step::StagingReadBack
            | Step::Publish => assert_eq!(state.unwrap(), AttemptState::AuthenticatedStaging),
            Step::ObjectDirectorySync | Step::PublishedReadBack | Step::RemoveStaging => {
                assert_eq!(
                    state.unwrap(),
                    AttemptState::AuthenticatedStagingAndPublishedCandidate
                )
            }
            Step::CleanupSync => assert_eq!(
                state.unwrap(),
                AttemptState::AuthenticatedPublishedCandidate
            ),
        }
        assert!(unrelated.is_file());
        // Presence after reopen does not prove a previously failed sync was durable.
        if fault != Step::Create {
            assert_eq!(
                reopened.publish(&write, &key()).unwrap_err().code(),
                SourceVaultErrorCode::ObjectExists
            );
        }
    }
}

#[test]
fn partial_staging_write_is_retained_and_cannot_be_read_as_a_published_object() {
    let root = TestDirectory::new();
    let vault = directory(&root);
    let write = write(b"synthetic partial write", 0);
    let stage = vault.staging_path(write.locator(), write.attempt_id());
    let error = vault
        .publish_with_operations(
            &write,
            &key(),
            |_| Ok(()),
            |file, bytes| {
                file.write_all(&bytes[..20])?;
                Err(io::ErrorKind::StorageFull.into())
            },
        )
        .unwrap_err();
    assert_eq!(error.io_kind(), Some(io::ErrorKind::StorageFull));
    assert_eq!(fs::metadata(&stage).unwrap().len(), 20);
    assert_eq!(
        vault
            .read(write.locator(), &write.metadata, &key())
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::ObjectMissing
    );
    assert_eq!(
        vault
            .inspect_attempt(write.locator(), write.attempt_id(), &write.metadata, &key())
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::InvalidEnvelope
    );
}

#[test]
fn concurrent_publishers_have_one_winner_and_keep_the_losing_attempt() {
    use std::sync::{Arc, Barrier};
    let root = TestDirectory::new();
    let vault = Arc::new(directory(&root));
    let barrier = Arc::new(Barrier::new(2));
    let mut workers = Vec::new();
    for seed in [0, 1] {
        let vault = vault.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            let write = write(b"synthetic concurrent source", seed);
            let result = vault.publish_with_step(&write, &key(), |step| {
                if step == Step::Publish {
                    barrier.wait();
                }
                Ok(())
            });
            (write, result)
        }));
    }
    let outcomes: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(outcomes.iter().filter(|(_, r)| r.is_ok()).count(), 1);
    assert_eq!(count(&vault.objects.path), 1);
    assert_eq!(count(&vault.staging.path), 1);
    let (loser, error) = outcomes.iter().find(|(_, r)| r.is_err()).unwrap();
    assert_eq!(
        error.as_ref().unwrap_err().code(),
        SourceVaultErrorCode::ObjectExists
    );
    assert!(
        vault
            .staging_path(loser.locator(), loser.attempt_id())
            .is_file()
    );
    assert_eq!(
        vault
            .inspect_attempt(loser.locator(), loser.attempt_id(), &loser.metadata, &key())
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::AttemptMismatch
    );
}

#[test]
fn target_created_at_publish_boundary_is_not_overwritten() {
    let root = TestDirectory::new();
    let vault = directory(&root);
    let write = write(b"synthetic publish collision", 0);
    let target = vault.object_path(write.locator());
    let error = vault
        .publish_with_step(&write, &key(), |step| {
            if step == Step::Publish {
                fs::write(&target, b"synthetic existing marker").unwrap();
            }
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.code(), SourceVaultErrorCode::ObjectExists);
    assert!(fs::read(&target).unwrap() == b"synthetic existing marker");
    assert!(
        vault
            .staging_path(write.locator(), write.attempt_id())
            .is_file()
    );
}

#[test]
fn wrong_key_never_creates_a_stage_and_random_production_constructor_is_not_fixed() {
    let root = TestDirectory::new();
    let vault = directory(&root);
    let write = write(b"synthetic key failure", 0);
    assert_eq!(
        vault
            .publish(&write, &KeyEncryptionKey::new([0xa6; 32]))
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::AuthenticationFailed
    );
    assert_eq!(count(&vault.staging.path), 0);
    let a = ObjectWrite::seal(&key(), &write.metadata, b"synthetic key failure").unwrap();
    let b = ObjectWrite::seal(&key(), &write.metadata, b"synthetic key failure").unwrap();
    assert!(a.attempt_id() != b.attempt_id());
    assert!(a.locator() == b.locator());
}

#[test]
fn ciphertext_envelope_metadata_locator_and_attempt_tampering_fail_closed() {
    let root = TestDirectory::new();
    let vault = directory(&root);
    let write = write(b"synthetic disk authentication", 0);
    vault.publish(&write, &key()).unwrap();
    let path = vault.object_path(write.locator());
    let original = fs::read(&path).unwrap();
    for offset in [0, 5, original.len() - 1] {
        let mut tampered = original.clone();
        tampered[offset] ^= 1;
        fs::write(&path, &tampered).unwrap();
        assert!(
            vault
                .read(write.locator(), &write.metadata, &key())
                .is_err()
        );
        assert!(
            vault
                .inspect_attempt(write.locator(), write.attempt_id(), &write.metadata, &key())
                .is_err()
        );
    }
    fs::write(&path, &original).unwrap();
    let wrong_media = ObjectMetadata::new(
        write.metadata.namespace_id(),
        write.metadata.source_id(),
        *write.metadata.exact_digest(),
        write.metadata.plaintext_len(),
        "text/plain",
    )
    .unwrap();
    assert_eq!(
        vault
            .read(write.locator(), &wrong_media, &key())
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::MetadataMismatch
    );
    assert_eq!(
        vault
            .read(
                write.locator(),
                &write.metadata,
                &KeyEncryptionKey::new([0xa6; 32])
            )
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::AuthenticationFailed
    );
    assert_eq!(
        vault
            .inspect_attempt(
                write.locator(),
                &AttemptId::from_token(&"0".repeat(64)).unwrap(),
                &write.metadata,
                &key()
            )
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::AttemptMismatch
    );
    assert_eq!(
        vault
            .read(
                &ObjectLocator::from_token(&"0".repeat(64)).unwrap(),
                &write.metadata,
                &key()
            )
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::InvalidLocator
    );
    fs::remove_file(&path).unwrap();
    assert_eq!(
        vault
            .read(write.locator(), &write.metadata, &key())
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::ObjectMissing
    );
}

#[test]
fn opaque_tokens_and_ambient_roots_cannot_be_used_as_paths() {
    for token in [
        "",
        "../object",
        "/absolute",
        "C:\\object",
        "a/b",
        "a\\b",
        &"A".repeat(64),
        &"a".repeat(63),
        &"a".repeat(65),
    ] {
        assert!(ObjectLocator::from_token(token).is_err());
        assert!(AttemptId::from_token(token).is_err());
    }
    assert!(ObjectDirectory::open_application_directory("relative").is_err());
    assert!(ObjectDirectory::open_application_directory(std::env::temp_dir()).is_err());
    assert!(ObjectDirectory::open_application_directory(std::env::current_dir().unwrap()).is_err());
    assert!(
        ObjectDirectory::open_application_directory(std::path::Path::new(
            std::path::MAIN_SEPARATOR_STR
        ))
        .is_err()
    );
}

#[test]
fn all_debug_and_io_errors_redact_paths_identity_plaintext_and_secrets() {
    let root = TestDirectory::new();
    let vault = directory(&root);
    let write = write(b"synthetic redaction marker", 0);
    let published = vault.publish(&write, &key()).unwrap();
    let error = SourceVaultError::io(
        "read encrypted object",
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private-source-path-and-secret",
        ),
    );
    let output = format!(
        "{vault:?} {write:?} {published:?} {:?} {:?} {error:?} {error}",
        write.locator(),
        write.attempt_id()
    );
    for secret in [
        root.0.to_str().unwrap(),
        write.locator().token(),
        write.attempt_id().token(),
        "namespace-synthetic",
        "source-synthetic",
        "synthetic redaction marker",
        "private-source-path-and-secret",
    ] {
        assert!(!output.contains(secret));
    }
    assert_eq!(error.io_kind(), Some(io::ErrorKind::PermissionDenied));
}

#[cfg(unix)]
#[test]
fn directory_and_object_symlinks_and_nonregular_entries_are_rejected() {
    use std::os::unix::fs::symlink;
    let root = TestDirectory::new();
    let outside = TestDirectory::new();
    let link = root.0.join("app-link");
    symlink(&outside.0, &link).unwrap();
    assert_eq!(
        ObjectDirectory::open_application_directory(&link)
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::InvalidDirectory
    );
    let object_link = root.0.join("source-objects-v1");
    symlink(&outside.0, &object_link).unwrap();
    assert_eq!(
        ObjectDirectory::open_application_directory(&root.0)
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::InvalidDirectory
    );
    fs::remove_file(object_link).unwrap();
    let vault = directory(&root);
    let write = write(b"synthetic symlink", 0);
    let target = vault.object_path(write.locator());
    let marker = outside.0.join("marker");
    fs::write(&marker, b"synthetic outside marker").unwrap();
    symlink(&marker, &target).unwrap();
    assert_eq!(
        vault.publish(&write, &key()).unwrap_err().code(),
        SourceVaultErrorCode::InvalidFile
    );
    assert_eq!(
        vault
            .read(write.locator(), &write.metadata, &key())
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::InvalidFile
    );
    fs::remove_file(&target).unwrap();
    fs::create_dir(&target).unwrap();
    assert_eq!(
        vault.publish(&write, &key()).unwrap_err().code(),
        SourceVaultErrorCode::InvalidFile
    );
    let stage = vault.staging_path(write.locator(), write.attempt_id());
    fs::remove_dir(&target).unwrap();
    symlink(&marker, &stage).unwrap();
    assert_eq!(
        vault.publish(&write, &key()).unwrap_err().code(),
        SourceVaultErrorCode::ObjectExists
    );
    assert!(fs::read(&marker).unwrap() == b"synthetic outside marker");
}

#[cfg(unix)]
#[test]
fn directory_swap_at_publish_boundary_does_not_escape_or_delete_unknown_files() {
    use std::os::unix::fs::symlink;
    let root = TestDirectory::new();
    let outside = TestDirectory::new();
    let vault = directory(&root);
    let write = write(b"synthetic directory swap", 0);
    let original = root.0.join("retained-original-staging");
    let error = vault
        .publish_with_step(&write, &key(), |step| {
            if step == Step::Publish {
                fs::rename(&vault.staging.path, &original).unwrap();
                symlink(&outside.0, &vault.staging.path).unwrap();
            }
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.code(), SourceVaultErrorCode::InvalidDirectory);
    assert_eq!(count(&outside.0), 0);
    assert_eq!(count(&original), 1);
    assert_eq!(count(&vault.objects.path), 0);
}

#[cfg(unix)]
#[test]
fn replaced_or_tampered_staging_and_final_objects_cannot_return_success() {
    for point in [
        Step::StagingReadBack,
        Step::Publish,
        Step::PublishedReadBack,
        Step::RemoveStaging,
        Step::CleanupSync,
    ] {
        let root = TestDirectory::new();
        let vault = directory(&root);
        let write = write(b"synthetic replacement", 0);
        let error = vault
            .publish_with_step(&write, &key(), |step| {
                if step == point {
                    let path = if matches!(point, Step::StagingReadBack | Step::Publish) {
                        vault.staging_path(write.locator(), write.attempt_id())
                    } else {
                        vault.object_path(write.locator())
                    };
                    fs::remove_file(&path).unwrap();
                    // Same valid envelope, different inode; authenticity alone cannot authorize removal.
                    fs::write(&path, &write.envelope).unwrap();
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.code(), SourceVaultErrorCode::FilesystemChanged);
    }
}

#[cfg(unix)]
#[test]
fn private_modes_are_created_and_permission_revocation_is_not_bypassed() {
    use std::os::unix::fs::PermissionsExt;
    let root = TestDirectory::new();
    let vault = directory(&root);
    let write = write(b"synthetic private permissions", 0);
    vault.publish(&write, &key()).unwrap();
    assert_eq!(
        fs::metadata(vault.object_path(write.locator()))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&vault.objects.path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    fs::set_permissions(&vault.objects.path, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        vault
            .read(write.locator(), &write.metadata, &key())
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::InvalidDirectory
    );
    fs::set_permissions(&vault.objects.path, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(&vault.staging.path, fs::Permissions::from_mode(0o500)).unwrap();
    let candidate = write_other_source();
    let result = vault.publish(&candidate, &key());
    fs::set_permissions(&vault.staging.path, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        result.unwrap_err().io_kind(),
        Some(io::ErrorKind::PermissionDenied)
    );
}

#[test]
fn metadata_preserving_replacement_cannot_authorize_staging_cleanup() {
    let root = TestDirectory::new();
    let vault = directory(&root);
    let write = write(b"synthetic metadata-preserving replacement", 0);
    let stage = vault.staging_path(write.locator(), write.attempt_id());
    let retained = root.0.join("retained-original-staging");
    let result = vault.publish_with_step(&write, &key(), |step| {
        if step == Step::RemoveStaging {
            replace_preserving_metadata(&stage, &retained, &write.envelope);
        }
        Ok(())
    });
    assert_eq!(
        result.unwrap_err().code(),
        SourceVaultErrorCode::FilesystemChanged
    );
    assert!(stage.is_file());
    assert!(retained.is_file());
    assert!(fs::read(&stage).unwrap() == write.envelope);
}

#[test]
fn metadata_preserving_replacements_fail_at_readback_and_publish_boundaries() {
    for (step_to_replace, replace_staging) in [
        (Step::StagingReadBack, true),
        (Step::Publish, true),
        (Step::PublishedReadBack, false),
        (Step::CleanupSync, false),
    ] {
        let root = TestDirectory::new();
        let vault = directory(&root);
        let write = write(b"synthetic boundary replacement", 0);
        let path = if replace_staging {
            vault.staging_path(write.locator(), write.attempt_id())
        } else {
            vault.object_path(write.locator())
        };
        let retained = root.0.join("retained-original-object");
        let result = vault.publish_with_step(&write, &key(), |step| {
            if step == step_to_replace {
                replace_preserving_metadata(&path, &retained, &write.envelope);
            }
            Ok(())
        });
        assert_eq!(
            result.unwrap_err().code(),
            SourceVaultErrorCode::FilesystemChanged
        );
        assert!(retained.is_file());
        assert!(fs::read(&path).unwrap() == write.envelope);
    }
}

fn replace_preserving_metadata(path: &std::path::Path, retained: &std::path::Path, bytes: &[u8]) {
    let before = fs::metadata(path).unwrap();
    // Keep the original file alive: the replacement has a different filesystem identity.
    fs::rename(path, retained).unwrap();
    fs::write(path, bytes).unwrap();
    let times = fs::FileTimes::new().set_modified(before.modified().unwrap());
    #[cfg(windows)]
    let times = {
        use std::os::windows::fs::FileTimesExt;
        times.set_created(before.created().unwrap())
    };
    fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(times)
        .unwrap();
    let after = fs::metadata(path).unwrap();
    assert_eq!(before.len(), after.len());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    #[cfg(windows)]
    assert_eq!(before.created().unwrap(), after.created().unwrap());
}

#[cfg(unix)]
fn write_other_source() -> ObjectWrite {
    let bytes = b"synthetic second private object";
    ObjectWrite::seal(&key(), &metadata(bytes), bytes).unwrap()
}
