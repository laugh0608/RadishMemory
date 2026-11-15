use super::*;
use crate::{ApplicationErrorReason, DeletionEvidence, FileCaptureOutcome};

fn input(fixture: &Fixture, body: &str) -> FileReadRequest {
    let path = fixture.root.0.join("write.md");
    fs::write(&path, body).unwrap();
    FileReadRequest::new(&path, vec![fixture.root.0.clone()]).unwrap()
}
fn writer(fixture: &Fixture) -> Library {
    // Continue the host ID sequence after legacy import; never reset provenance.
    fixture
        .location()
        .open(Runtime {
            next: fixture.runtime.next,
            fail_clock: false,
        })
        .unwrap()
}
fn count(fixture: &Fixture, table: &str) -> i64 {
    fixture.scalar(&format!("SELECT count(*) FROM {table}"))
}

fn restore_legacy_receipt(fixture: &Fixture, receipt: &DeletionEvidence) -> DeletionEvidence {
    // Reproduce the old on-disk format around one receipt from real execution.
    let mut params = receipt.params().clone();
    params.evidence_digest = radishmemory_core::compute_deletion_evidence_digest(
        &params.deletion_evidence_id,
        &params.delete_request_id,
        params.overall_status,
        &params.component_results,
    )
    .unwrap();
    let connection = Connection::open(fixture.root.0.join("library.sqlite3")).unwrap();
    assert_eq!(connection.execute(
        "UPDATE radishmemory_deletion_evidence SET evidence_digest_profile=?1, evidence_digest_value=?2 WHERE deletion_evidence_id=?3",
        rusqlite::params![params.evidence_digest.profile().as_str(), params.evidence_digest.value(), params.deletion_evidence_id.as_str()],
    ).unwrap(), 1);
    restore_legacy_receipt_table(&connection);
    DeletionEvidence::new(params).unwrap()
}

fn restore_legacy_receipt_table(connection: &Connection) {
    let legacy = include_str!("../../../../radishmemory-sqlite/migrations/0005_local_deletion.sql");
    let table = &legacy[legacy
        .find("CREATE TABLE radishmemory_deletion_evidence (")
        .unwrap()..];
    connection
        .execute_batch(&format!(
            "BEGIN EXCLUSIVE;
         ALTER TABLE radishmemory_deletion_evidence RENAME TO synthetic_evidence_source;
         {table}
         INSERT INTO radishmemory_deletion_evidence SELECT * FROM synthetic_evidence_source;
         DROP TABLE synthetic_evidence_source;
         DELETE FROM radishmemory_schema_migrations WHERE version=13;
         COMMIT;"
        ))
        .unwrap();
}

#[test]
fn plaintext_receipt_upgrade_verifies_legacy_and_blocks_new_purge_until_confirmed() {
    let fixture = Fixture::legacy();
    let open = |next| {
        LocalLibrary::open_current(
            &fixture.root.0.join("library.sqlite3"),
            Runtime {
                next,
                fail_clock: false,
            },
            fixture.config.clone(),
        )
        .unwrap()
    };
    let mut library = open(fixture.runtime.next);
    let receipt = library
        .delete_source_lineage(fixture.first.lineage_id())
        .unwrap();
    let capture = library
        .import_new_source(&input(&fixture, UPDATED))
        .unwrap();
    let (runtime, _) = library.close();
    let legacy = restore_legacy_receipt(&fixture, &receipt);
    let id = &receipt.params().deletion_evidence_id;
    let mut library = open(runtime.next);
    assert!(library.deletion_evidence_upgrade_required().unwrap());
    assert_eq!(
        library.get_deletion_evidence(id).unwrap(),
        Some(legacy.clone())
    );
    let error = library
        .delete_source_lineage(capture.lineage_id())
        .unwrap_err();
    assert_eq!(
        error.sqlite_failure().unwrap().storage_reason(),
        Some(radishmemory_sqlite::SqliteStorageReason::EvidenceUpgradeRequired)
    );
    assert_eq!(count(&fixture, "radishmemory_delete_requests"), 1);
    assert_eq!(count(&fixture, "radishmemory_source_bodies"), 1);

    // Legacy digests are retained, and an invalid legacy projection aborts maintenance.
    fixture
        .sql("UPDATE radishmemory_deletion_evidence SET evidence_digest_value=printf('%064d',0)");
    assert!(library.get_deletion_evidence(id).is_err());
    assert!(library.upgrade_deletion_evidence().is_err());
    assert!(library.deletion_evidence_upgrade_required().unwrap());
    fixture.sql(&format!(
        "UPDATE radishmemory_deletion_evidence SET evidence_digest_value='{}'",
        legacy.params().evidence_digest.value()
    ));
    library.upgrade_deletion_evidence().unwrap();
    library.upgrade_deletion_evidence().unwrap();
    assert!(!library.deletion_evidence_upgrade_required().unwrap());
    assert_eq!(fixture.scalar("PRAGMA user_version"), 6);
    assert_eq!(library.get_deletion_evidence(id).unwrap(), Some(legacy));
    let new_receipt = library.delete_source_lineage(capture.lineage_id()).unwrap();
    assert_eq!(
        new_receipt.params().evidence_digest.profile(),
        radishmemory_core::DigestProfile::DeletionEvidenceV2
    );
    let (runtime, _) = library.close();
    assert_eq!(
        open(runtime.next)
            .get_deletion_evidence(&new_receipt.params().deletion_evidence_id)
            .unwrap(),
        Some(new_receipt)
    );
}

#[test]
fn encrypted_import_update_and_replay_keep_exact_bytes_and_distinct_provenance() {
    let fixture = Fixture::migrated();
    let mut library = writer(&fixture);
    let request = input(&fixture, BODY);
    let first = library.prepare_import_new_source(&request).unwrap();
    assert_eq!(count(&fixture, "radishmemory_source_artifacts"), 1);
    let receipt = library.capture_source(&first).unwrap();
    assert_eq!(receipt.outcome(), FileCaptureOutcome::Created);
    assert_ne!(receipt.lineage_id(), fixture.first.lineage_id());
    let same = library
        .prepare_update_source(receipt.lineage_id(), &request)
        .unwrap();
    assert_eq!(
        library.capture_source(&same).unwrap().outcome(),
        FileCaptureOutcome::Idempotent
    );
    assert_eq!(count(&fixture, "radishmemory_source_artifacts"), 2);
    let update = library
        .prepare_update_source(receipt.lineage_id(), &input(&fixture, UPDATED))
        .unwrap();
    let updated = library.capture_source(&update).unwrap();
    assert_eq!(updated.version().get(), 2);
    assert_eq!(
        library.capture_source(&first).unwrap().source_id(),
        receipt.source_id()
    );
    assert_eq!(
        library.capture_source(&update).unwrap().source_id(),
        updated.source_id()
    );
    let (_, location) = library.close();
    fs::remove_file(fixture.root.0.join("write.md")).unwrap();
    let library = location.open(Runtime::default()).unwrap();
    assert_eq!(
        library
            .get_source(receipt.source_id())
            .unwrap()
            .unwrap()
            .params()
            .content
            .as_str(),
        BODY
    );
    assert_eq!(
        library
            .get_source(updated.source_id())
            .unwrap()
            .unwrap()
            .params()
            .content
            .as_str(),
        UPDATED
    );
    let export = fixture.root.0.join("exact-export.md");
    let export_request = FileExportRequest::new(&export, vec![fixture.root.0.clone()]).unwrap();
    library
        .export_source(updated.source_id(), &export_request)
        .unwrap();
    assert_eq!(fs::read(export).unwrap(), UPDATED.as_bytes());
    assert_eq!(count(&fixture, "radishmemory_source_bodies"), 0);
    assert_eq!(
        library.verify_library().unwrap().committed_objects_verified,
        3
    );
}

#[test]
fn interrupted_capture_requires_original_request_and_never_rereads_origin() {
    for after_commit in [false, true] {
        let fixture = Fixture::migrated();
        let mut library = writer(&fixture);
        let read = input(&fixture, UPDATED);
        let original = library
            .prepare_update_source(fixture.first.lineage_id(), &read)
            .unwrap();
        let other = library.prepare_import_new_source(&read).unwrap();
        if after_commit {
            fixture.provider.interrupt_next_capture_after_commit();
        } else {
            fixture.provider.interrupt_next_capture_after_publish();
        }
        assert!(library.capture_source(&original).is_err());
        let objects = fixture.objects();
        if !after_commit {
            assert_eq!(
                library
                    .prepare_import_new_source(&read)
                    .err()
                    .unwrap()
                    .reason(),
                ApplicationErrorReason::OriginalRequestRequired
            );
            assert_eq!(
                library
                    .prepare_source_lineage_deletion(fixture.first.lineage_id())
                    .err()
                    .unwrap()
                    .reason(),
                ApplicationErrorReason::OriginalRequestRequired
            );
            assert!(library.capture_source(&other).is_err());
            assert_eq!(library.verify_library().unwrap().pending_captures, 1);
            assert_eq!(library.rebuild_recall().unwrap().pending_captures, 1);
            assert_eq!(
                library
                    .list_source_versions(fixture.first.lineage_id())
                    .unwrap()
                    .len(),
                1
            );
        }
        fs::remove_file(fixture.root.0.join("write.md")).unwrap();
        let (runtime, location) = library.close();
        let library = location.open(runtime).unwrap();
        let receipt = library.capture_source(&original).unwrap();
        assert_eq!(receipt.source_id(), &original.source().params().source_id);
        assert_eq!(fixture.objects(), objects);
        assert_eq!(
            library
                .get_source(receipt.source_id())
                .unwrap()
                .unwrap()
                .params()
                .content
                .as_str(),
            UPDATED
        );
        assert_eq!(count(&fixture, "radishmemory_source_capture_audit"), 2);
        assert_eq!(count(&fixture, "radishmemory_source_artifacts"), 2);
    }
}

#[test]
fn stale_update_and_stale_deletion_cannot_rebase_or_expand_targets() {
    let fixture = Fixture::migrated();
    let mut library = writer(&fixture);
    let request = library
        .prepare_source_lineage_deletion(fixture.first.lineage_id())
        .unwrap();
    let update = library
        .prepare_update_source(fixture.first.lineage_id(), &input(&fixture, UPDATED))
        .unwrap();
    let stale = library
        .prepare_update_source(
            fixture.first.lineage_id(),
            &input(&fixture, "Synthetic concurrent bytes"),
        )
        .unwrap();
    library.capture_source(&update).unwrap();
    assert!(library.capture_source(&stale).is_err());
    assert!(library.execute_source_lineage_deletion(&request).is_err());
    assert_eq!(count(&fixture, "radishmemory_delete_requests"), 0);
    assert_eq!(request.params().target_refs.len(), 1);
    assert_eq!(
        library
            .list_source_versions(fixture.first.lineage_id())
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn deletion_recovers_persisted_request_and_returns_existing_completed_evidence() {
    for phase in 0..4 {
        let fixture = Fixture::migrated();
        let mut library = writer(&fixture);
        let request = library
            .prepare_source_lineage_deletion(fixture.first.lineage_id())
            .unwrap();
        assert!(
            library
                .get_delete_request(&request.params().delete_request_id)
                .unwrap()
                .is_none()
        );
        match phase {
            0 => fixture.provider.interrupt_next_deletion_after_intent(),
            1 => fixture.provider.interrupt_next_deletion_after_execution(),
            2 => fixture.provider.interrupt_next_deletion_evidence(false),
            _ => fixture.provider.interrupt_next_deletion_evidence(true),
        }
        assert!(library.execute_source_lineage_deletion(&request).is_err());
        assert!(library.list_sources(0, 20).unwrap().is_empty());
        assert!(
            library
                .get_source(fixture.first.source_id())
                .unwrap()
                .is_none()
        );
        let attempts_before = count(&fixture, "radishmemory_deletion_execution_attempts");
        let (runtime, location) = library.close();
        let mut library = location.open(runtime).unwrap();
        let persisted = library
            .get_delete_request(&request.params().delete_request_id)
            .unwrap()
            .unwrap();
        assert_eq!(persisted, request);
        if phase == 0 {
            assert_eq!(
                library.verify_library().unwrap().deletion_pending_objects,
                1
            );
            assert_eq!(
                library.rebuild_recall().unwrap().deletion_pending_objects,
                1
            );
            assert_eq!(
                library
                    .prepare_source_lineage_deletion(fixture.first.lineage_id())
                    .err()
                    .unwrap()
                    .reason(),
                ApplicationErrorReason::OriginalRequestRequired
            );
        }
        let evidence = library.execute_source_lineage_deletion(&persisted).unwrap();
        assert_eq!(
            evidence.params().overall_status,
            crate::DeletionOverallStatus::Completed
        );
        assert_eq!(evidence.params().component_results.len(), 10);
        assert_eq!(
            library
                .get_deletion_evidence(&evidence.params().deletion_evidence_id)
                .unwrap(),
            Some(evidence.clone())
        );
        let attempts = count(&fixture, "radishmemory_deletion_execution_attempts");
        if phase == 3 {
            assert_eq!(attempts, attempts_before);
        }
        assert_eq!(
            library.execute_source_lineage_deletion(&persisted).unwrap(),
            evidence
        );
        assert_eq!(
            count(&fixture, "radishmemory_deletion_execution_attempts"),
            attempts
        );
        assert_eq!(count(&fixture, "radishmemory_deletion_evidence"), 1);
        assert_eq!(library.rebuild_recall().unwrap().deleted_objects, 1);
        assert!(library.list_sources(0, 20).unwrap().is_empty());
        assert!(fixture.objects().is_empty());
        let mut altered = persisted.params().clone();
        altered.reason_code = text("Synthetic altered authority");
        assert_eq!(
            library
                .execute_source_lineage_deletion(&DeleteRequest::new(altered).unwrap())
                .unwrap_err()
                .reason(),
            ApplicationErrorReason::RequestProfileMismatch
        );
    }
}

#[test]
fn location_maintenance_recovers_failed_open_without_repairing_canonical_objects() {
    let fixture = Fixture::migrated();
    let objects = fixture.objects();
    fixture.sql("DELETE FROM radishmemory_source_lineage_tips; UPDATE radishmemory_recall_fts SET content='Synthetic index drift'");
    assert!(fixture.location().open(Runtime::default()).is_err());
    assert!(fixture.location().verify_library().is_err());
    assert!(
        fixture
            .location()
            .rebuild_recall()
            .unwrap()
            .derivations_rebuilt
    );
    assert_eq!(fixture.objects(), objects);
    let mut library = fixture.open();
    assert_eq!(
        library
            .search_sources(text("needle"), 10, [Sensitivity::Personal])
            .unwrap()
            .len(),
        1
    );
    assert_eq!(fixture.scalar("PRAGMA user_version"), 8);
    fs::write(&objects[0].0, b"Synthetic ciphertext damage").unwrap();
    assert!(fixture.location().rebuild_recall().is_err());
    assert!(fixture.location().verify_library().is_err());
}

#[test]
fn key_errors_and_profile_mismatch_block_writes_and_maintenance_without_new_keys() {
    let fixture = Fixture::migrated();
    let mut library = writer(&fixture);
    let capture = library
        .prepare_import_new_source(&input(&fixture, UPDATED))
        .unwrap();
    let deletion = library
        .prepare_source_lineage_deletion(fixture.first.lineage_id())
        .unwrap();
    for code in [
        SourceVaultErrorCode::KeyMissing,
        SourceVaultErrorCode::KeyStoreLocked,
        SourceVaultErrorCode::KeyStoreDenied,
    ] {
        fixture.provider.fail_key_access(Some(code));
        for error in [
            library.capture_source(&capture).unwrap_err(),
            library
                .execute_source_lineage_deletion(&deletion)
                .unwrap_err(),
            library.verify_library().unwrap_err(),
            library.rebuild_recall().unwrap_err(),
        ] {
            assert_eq!(vault_code(&error), Some(code));
            assert_redacted(&error, &fixture.root);
        }
    }
    fixture.provider.fail_key_access(None);
    assert_eq!(count(&fixture, "radishmemory_source_artifacts"), 1);
    assert_eq!(count(&fixture, "radishmemory_delete_requests"), 0);
    assert_eq!(fixture.provider.key_creations(), 1);
    let mut params = deletion.params().clone();
    params.device_id = id("device-11111111111111111111111111111111");
    assert_eq!(
        library
            .execute_source_lineage_deletion(&DeleteRequest::new(params).unwrap())
            .unwrap_err()
            .reason(),
        ApplicationErrorReason::RequestProfileMismatch
    );
}

#[test]
fn finishing_clock_failure_preserves_actual_deletion_and_original_recovery_authority() {
    struct ClockFailure {
        inner: Runtime,
        calls: usize,
    }
    impl ApplicationRuntime for ClockFailure {
        type Error = io::Error;
        fn next_identifier(
            &mut self,
            kind: ApplicationIdentifierKind,
        ) -> Result<Identifier, io::Error> {
            self.inner.next_identifier(kind)
        }
        fn now(&mut self) -> Result<Timestamp, io::Error> {
            self.calls += 1;
            if self.calls == 3 {
                Err(io::Error::other("Synthetic finishing clock failure"))
            } else {
                self.inner.now()
            }
        }
    }
    let fixture = Fixture::migrated();
    let mut library = fixture
        .location()
        .open(ClockFailure {
            inner: Runtime {
                next: fixture.runtime.next,
                fail_clock: false,
            },
            calls: 0,
        })
        .unwrap();
    let request = library
        .prepare_source_lineage_deletion(fixture.first.lineage_id())
        .unwrap();
    assert_eq!(
        library
            .execute_source_lineage_deletion(&request)
            .unwrap_err()
            .reason(),
        ApplicationErrorReason::ClockFailed
    );
    assert_eq!(
        count(&fixture, "radishmemory_deletion_execution_attempts"),
        1
    );
    assert_eq!(count(&fixture, "radishmemory_deletion_evidence"), 0);
    assert!(fixture.objects().is_empty());
    assert_eq!(
        library
            .get_delete_request(&request.params().delete_request_id)
            .unwrap(),
        Some(request.clone())
    );
    let evidence = library.execute_source_lineage_deletion(&request).unwrap();
    assert_eq!(
        evidence.params().overall_status,
        crate::DeletionOverallStatus::Completed
    );
    assert_eq!(count(&fixture, "radishmemory_delete_requests"), 1);
}

#[test]
fn failed_legacy_deletion_resumes_original_request_and_extends_evidence_chain() {
    let mut fixture = Fixture::legacy();
    let mut legacy = LocalLibrary::open(
        fixture.root.0.join("library.sqlite3"),
        Runtime {
            next: fixture.runtime.next,
            fail_clock: false,
        },
        fixture.config.clone(),
    )
    .unwrap();
    // Real SQLite component failure after opening; remove this test-only trigger
    // before any encrypted schema validation, without weakening production checks.
    fixture.sql("CREATE TRIGGER synthetic_component_failure BEFORE DELETE ON radishmemory_source_bodies BEGIN SELECT RAISE(ABORT, 'Synthetic body failure'); END");
    let failed = legacy
        .delete_source_lineage(fixture.first.lineage_id())
        .unwrap();
    assert_eq!(
        failed.params().overall_status,
        crate::DeletionOverallStatus::Failed
    );
    fixture.sql("DROP TRIGGER synthetic_component_failure");
    let (runtime, config) = legacy.close();
    let failed = restore_legacy_receipt(&fixture, &failed);
    fixture.runtime = runtime;
    fixture.config = config;
    fixture.location().initialize_key().unwrap();
    fixture.location().migrate_bodies().unwrap();
    let mut library = writer(&fixture);
    let original = library
        .get_delete_request(&failed.params().delete_request_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        library
            .get_deletion_evidence(&failed.params().deletion_evidence_id)
            .unwrap(),
        Some(failed.clone())
    );
    assert_eq!(
        library.unfinished_delete_requests().unwrap(),
        vec![original.clone()]
    );
    let objects = fixture.objects();
    assert!(!objects.is_empty());
    assert!(library.execute_source_lineage_deletion(&original).is_err());
    assert_eq!(objects, fixture.objects());
    assert_eq!(
        count(&fixture, "radishmemory_deletion_execution_attempts"),
        1
    );
    assert_eq!(fixture.scalar("PRAGMA user_version"), 8);
    library.upgrade_deletion_evidence().unwrap();
    assert_eq!(fixture.scalar("PRAGMA user_version"), 8);
    let completed = library.execute_source_lineage_deletion(&original).unwrap();
    assert_eq!(
        completed.params().evidence_digest.profile(),
        radishmemory_core::DigestProfile::DeletionEvidenceV2
    );
    assert!(library.unfinished_delete_requests().unwrap().is_empty());
    assert_eq!(
        completed.params().overall_status,
        crate::DeletionOverallStatus::Completed
    );
    assert_eq!(
        completed.params().previous_evidence_id.as_ref(),
        Some(&failed.params().deletion_evidence_id)
    );
    assert_eq!(count(&fixture, "radishmemory_delete_requests"), 1);
    assert_eq!(count(&fixture, "radishmemory_deletion_evidence"), 2);
    assert!(fixture.objects().is_empty());
    assert!(library.list_sources(0, 20).unwrap().is_empty());
    assert_eq!(
        library.execute_source_lineage_deletion(&original).unwrap(),
        completed
    );
}

#[test]
fn restart_discovers_original_deletion_authority_even_after_execution_before_evidence() {
    for after_execution in [false, true] {
        let fixture = Fixture::migrated();
        let mut library = writer(&fixture);
        let original = library
            .prepare_source_lineage_deletion(fixture.first.lineage_id())
            .unwrap();
        if after_execution {
            fixture.provider.interrupt_next_deletion_after_execution();
        } else {
            fixture.provider.interrupt_next_deletion_after_intent();
        }
        assert!(library.execute_source_lineage_deletion(&original).is_err());
        drop(library);
        let connection = Connection::open(fixture.root.0.join("library.sqlite3")).unwrap();
        restore_legacy_receipt_table(&connection);
        drop(connection);
        let mut reopened = writer(&fixture);
        let requests = reopened.unfinished_delete_requests().unwrap();
        assert_eq!(requests, vec![original]);
        let objects = fixture.objects();
        let attempts = count(&fixture, "radishmemory_deletion_execution_attempts");
        assert_eq!(fixture.scalar("PRAGMA user_version"), 11);
        assert!(
            reopened
                .execute_source_lineage_deletion(&requests[0])
                .is_err()
        );
        reopened.upgrade_deletion_evidence().unwrap();
        assert_eq!(fixture.scalar("PRAGMA user_version"), 11);
        assert_eq!(fixture.objects(), objects);
        assert_eq!(
            count(&fixture, "radishmemory_deletion_execution_attempts"),
            attempts
        );
        reopened
            .execute_source_lineage_deletion(&requests[0])
            .unwrap();
        assert!(reopened.unfinished_delete_requests().unwrap().is_empty());
        assert_eq!(count(&fixture, "radishmemory_delete_requests"), 1);
    }
}

#[test]
fn lost_capture_request_after_restart_requires_explicit_exact_abandonment() {
    let fixture = Fixture::migrated();
    let mut library = writer(&fixture);
    let original = library
        .prepare_update_source(fixture.first.lineage_id(), &input(&fixture, UPDATED))
        .unwrap();
    fixture.provider.interrupt_next_capture_after_publish();
    assert!(library.capture_source(&original).is_err());
    let before = fixture.objects();
    drop(library);
    let mut reopened = writer(&fixture);
    let target = reopened.inspect_capture_abandonment().unwrap().unwrap();
    assert_eq!(fixture.objects(), before);
    assert_eq!(reopened.verify_library().unwrap().pending_captures, 1);
    assert_eq!(
        reopened
            .prepare_import_new_source(&input(&fixture, "Synthetic changed origin"))
            .err()
            .unwrap()
            .reason(),
        ApplicationErrorReason::OriginalRequestRequired
    );
    reopened.abandon_capture(&target).unwrap();
    assert!(reopened.inspect_capture_abandonment().unwrap().is_none());
    assert_eq!(reopened.verify_library().unwrap().abandoned_attempts, 1);
    assert_eq!(fixture.objects().len(), 1);
    assert_eq!(
        reopened
            .get_source(fixture.first.source_id())
            .unwrap()
            .unwrap()
            .params()
            .content
            .as_str(),
        BODY
    );
    assert!(reopened.capture_source(&original).is_err());
    assert!(reopened.abandon_capture(&target).unwrap().already_abandoned);
}

#[test]
fn abandonment_does_not_delete_unknown_files_or_targets_from_another_library() {
    let fixture = Fixture::migrated();
    let mut library = writer(&fixture);
    let request = library
        .prepare_import_new_source(&input(&fixture, UPDATED))
        .unwrap();
    fixture.provider.interrupt_next_capture_after_publish();
    assert!(library.capture_source(&request).is_err());
    let target = library.inspect_capture_abandonment().unwrap().unwrap();
    let other = Fixture::migrated();
    let other_before = other.objects();
    assert!(other.open().abandon_capture(&target).is_err());
    assert_eq!(other.objects(), other_before);
    let unknown = fixture.root.0.join("source-objects-v1/unknown-synthetic");
    fs::write(&unknown, b"Synthetic unknown file").unwrap();
    let before = fixture.objects();
    assert!(library.abandon_capture(&target).is_err());
    assert_eq!(fixture.objects(), before);
    assert!(unknown.exists());
}

#[test]
fn recovery_discovery_rejects_damaged_completed_evidence_instead_of_hiding_request() {
    let fixture = Fixture::migrated();
    let mut library = writer(&fixture);
    let request = library
        .prepare_source_lineage_deletion(fixture.first.lineage_id())
        .unwrap();
    let receipt = library.execute_source_lineage_deletion(&request).unwrap();
    assert!(library.unfinished_delete_requests().unwrap().is_empty());
    fixture.sql("UPDATE radishmemory_deletion_execution_results SET verification_method='synthetic-other-method' WHERE component_key='source-body'");
    assert!(
        library
            .get_deletion_evidence(&receipt.params().deletion_evidence_id)
            .is_err()
    );
    assert!(library.unfinished_delete_requests().is_err());
    assert!(library.execute_source_lineage_deletion(&request).is_err());
    assert_eq!(
        count(&fixture, "radishmemory_deletion_execution_attempts"),
        1
    );
}
