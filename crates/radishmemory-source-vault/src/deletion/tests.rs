use super::*;
use crate::capture::{
    Step as CaptureStep,
    tests::{
        BODY, DEVICE, NS, count, files, interrupt, request as capture_request, run, setup,
        setup_confirmed_memory,
    },
};
use crate::test_support::{TestDirectory, key};
use radishmemory_core::*;
use rusqlite::Connection;
use std::{fs, path::PathBuf};

fn id(s: &str) -> Identifier {
    Identifier::new(s).unwrap()
}
fn text(s: &str) -> NonEmptyText {
    NonEmptyText::new(s).unwrap()
}
fn at() -> Timestamp {
    Timestamp::parse("2026-09-26T08:00:00Z").unwrap()
}
fn request(sources: &[&str], memories: &[&str]) -> DeleteRequest {
    let targets: Vec<_> = sources
        .iter()
        .map(|s| ObjectRef::new(CanonicalObjectType::SourceArtifact, id(s)))
        .chain(
            memories
                .iter()
                .map(|s| ObjectRef::new(CanonicalObjectType::MemoryRecord, id(s))),
        )
        .collect();
    DeleteRequest::new(DeleteRequestParams {
        delete_request_id: id("delete-synthetic"),
        namespace_id: id(NS),
        requested_by: ActorRef::new(ActorType::TestFixture, id("actor-synthetic"), None),
        authorization_basis: text("explicit-synthetic-deletion"),
        requested_guarantee: RequestedGuarantee::LocalPurge,
        device_id: id(DEVICE),
        planned_components: build_local_purge_targets(&targets).unwrap(),
        target_refs: targets,
        reason_code: text("synthetic-local-purge"),
        requested_at: at(),
    })
    .unwrap()
}
fn execution() -> LocalDeletionExecution {
    LocalDeletionExecution::new(
        at(),
        EvidenceRef::new(EvidenceType::PolicyBasis, id("policy-synthetic")),
    )
    .unwrap()
}
fn delete(dir: &ObjectDirectory, req: &DeleteRequest) -> Result<Vec<ComponentResult>> {
    execute(dir, req, &execution(), || Ok(key()))
}
fn failure() -> VaultMaintenanceError {
    SourceVaultError::new(
        crate::SourceVaultErrorCode::AttemptMismatch,
        "synthetic interruption",
    )
    .into()
}
fn sql(root: &TestDirectory, text: &str) {
    Connection::open(root.0.join("library.sqlite3"))
        .unwrap()
        .execute_batch(text)
        .unwrap();
}
fn paths(root: &TestDirectory, source: &str) -> (PathBuf, PathBuf) {
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    let (locator, attempt): (String, String) = c
        .query_row(
            "SELECT locator,attempt_id FROM radishmemory_source_vault_attempts WHERE source_id=?1",
            [source],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    (
        root.0
            .join("source-objects-v1")
            .join(format!("{locator}.rmo")),
        root.0
            .join("source-staging-v1")
            .join(format!("{locator}.{attempt}.stage")),
    )
}
fn verify(dir: &ObjectDirectory, rebuild: bool) -> crate::VerificationReport {
    crate::maintenance::maintain(dir, NS, DEVICE, rebuild, || Ok(key())).unwrap()
}

#[test]
fn lineage_deletion_has_ten_real_results_and_preserves_same_bytes_other_provenance() {
    let (root, dir) = setup_confirmed_memory();
    run(
        &dir,
        &capture_request(
            "next-source",
            "legacy-lineage",
            2,
            "Synthetic next version",
            Some("legacy-source"),
        ),
    )
    .unwrap();
    run(
        &dir,
        &capture_request("other-source", "other-lineage", 1, BODY, None),
    )
    .unwrap();
    let other_path = paths(&root, "other-source").0;
    let other = fs::read(&other_path).unwrap();
    let (object, staging) = paths(&root, "legacy-source");
    fs::hard_link(&object, &staging).unwrap();
    let req = request(&["legacy-source", "next-source"], &["memory-synthetic"]);
    let result = delete(&dir, &req).unwrap();
    assert_eq!(result.len(), 10);
    assert!(
        result
            .iter()
            .all(|r| r.params().status == ComponentStatus::Succeeded)
    );
    let body = result
        .iter()
        .find(|r| r.params().component_type == DeletionComponentType::SourceBody)
        .unwrap();
    assert_eq!(body.params().outcome, ComponentOutcome::Deleted);
    assert_eq!(
        body.params().verification_method.as_str(),
        "source-vault-authenticated-absence-v1"
    );
    assert!(!object.exists());
    assert!(!staging.exists());
    assert_eq!(fs::read(other_path).unwrap(), other);
    assert_eq!(count(&root, "radishmemory_source_vault_references"), 1);
    assert_eq!(count(&root, "radishmemory_recall_fts"), 1);
    assert_eq!(verify(&dir, true).deleted_objects, 2);
    assert_eq!(count(&root, "radishmemory_recall_fts"), 1);
    assert!(
        run(
            &dir,
            &capture_request("legacy-source", "legacy-lineage", 1, BODY, None)
        )
        .is_err()
    );
    let again = delete(&dir, &req).unwrap();
    assert!(
        again
            .iter()
            .all(|r| r.params().status == ComponentStatus::Succeeded)
    );
    assert!(radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).is_err());
}

#[test]
fn v8_request_checkpoint_and_every_interruption_resume_without_recall_resurrection() {
    for stop in [
        Step::Validated,
        Step::IntentCommitted,
        Step::Filesystem(RetirementStep::BeforeRemoval),
        Step::Filesystem(RetirementStep::StagingRemoved),
        Step::Filesystem(RetirementStep::ObjectRemoved),
        Step::Filesystem(RetirementStep::DirectoriesSynced),
        Step::BeforeRetirementCommit,
        Step::RetirementCommitted,
        Step::BeforeExecution,
        Step::Executed,
        Step::BeforeReadBack,
    ] {
        let (root, dir) = setup(true);
        let req = request(&["legacy-source"], &[]);
        assert!(
            execute_with_step(
                &dir,
                &req,
                &execution(),
                || Ok(key()),
                |s| if s == stop { Err(failure()) } else { Ok(()) }
            )
            .is_err()
        );
        if stop != Step::Validated {
            assert_eq!(count(&root, "radishmemory_source_vault_references"), 0);
            assert_eq!(count(&root, "radishmemory_recall_fts"), 0);
            verify(&dir, true);
            assert_eq!(count(&root, "radishmemory_recall_fts"), 0);
        }
        assert!(
            delete(&dir, &req)
                .unwrap()
                .iter()
                .all(|r| r.params().status == ComponentStatus::Succeeded)
        );
        assert!(files(&root).is_empty());
        assert_eq!(verify(&dir, false).deleted_objects, 1);
    }
}

#[test]
fn partial_lineage_missing_memory_targets_and_pending_capture_block_intent() {
    for mode in 0..3 {
        let (root, dir) = setup_confirmed_memory();
        if mode == 0 {
            run(
                &dir,
                &capture_request(
                    "next-source",
                    "legacy-lineage",
                    2,
                    "Synthetic next",
                    Some("legacy-source"),
                ),
            )
            .unwrap();
        }
        if mode == 2 {
            interrupt(
                &dir,
                &capture_request("pending-source", "other-lineage", 1, BODY, None),
                CaptureStep::Published,
            );
        }
        let req = request(
            &["legacy-source"],
            if mode == 1 {
                &[]
            } else {
                &["memory-synthetic"]
            },
        );
        let before = files(&root);
        assert!(delete(&dir, &req).is_err());
        assert_eq!(count(&root, "radishmemory_delete_requests"), 0);
        assert_eq!(files(&root), before);
    }
}

#[test]
fn bad_objects_unknown_files_wrong_keys_and_ambiguous_staging_never_authorize_cleanup() {
    for mode in 0..5 {
        let (root, dir) = setup(true);
        let req = request(&["legacy-source"], &[]);
        let (object, staging) = paths(&root, "legacy-source");
        match mode {
            0 => fs::write(&object, b"Synthetic bad object").unwrap(),
            1 => fs::remove_file(&object).unwrap(),
            2 => fs::write(
                root.0.join("source-objects-v1/unknown"),
                b"Synthetic unknown",
            )
            .unwrap(),
            3 => {
                fs::copy(&object, &staging).unwrap();
            }
            _ => (),
        }
        let before = files(&root);
        assert!(
            execute(&dir, &req, &execution(), || Ok(if mode == 4 {
                KeyEncryptionKey::new([0x12; 32])
            } else {
                key()
            }))
            .is_err()
        );
        assert_eq!(count(&root, "radishmemory_delete_requests"), 0);
        assert_eq!(files(&root), before);
    }
}

#[test]
fn plan_or_closure_tamper_after_intent_is_rejected_before_removal() {
    for mutation in [
        "UPDATE radishmemory_delete_requests SET authorization_basis='Synthetic tamper'",
        "DELETE FROM radishmemory_delete_execution_closure WHERE component_type='source_body'",
    ] {
        let (root, dir) = setup(true);
        let req = request(&["legacy-source"], &[]);
        assert!(
            execute_with_step(
                &dir,
                &req,
                &execution(),
                || Ok(key()),
                |s| if s == Step::IntentCommitted {
                    Err(failure())
                } else {
                    Ok(())
                }
            )
            .is_err()
        );
        let before = files(&root);
        sql(&root, mutation);
        assert!(delete(&dir, &req).is_err());
        assert_eq!(files(&root), before);
    }
}

#[test]
fn reappeared_retired_object_blocks_verify_rebuild_and_retry() {
    let (root, dir) = setup(true);
    let req = request(&["legacy-source"], &[]);
    let object = paths(&root, "legacy-source").0;
    let bytes = fs::read(&object).unwrap();
    delete(&dir, &req).unwrap();
    fs::write(&object, &bytes).unwrap();
    for rebuild in [false, true] {
        assert!(crate::maintenance::maintain(&dir, NS, DEVICE, rebuild, || Ok(key())).is_err());
    }
    assert!(delete(&dir, &req).is_err());
    assert_eq!(fs::read(object).unwrap(), bytes);
}

#[test]
fn canonical_evidence_matches_persisted_results_and_round_trips() {
    let (root, dir) = setup(true);
    let req = request(&["legacy-source"], &[]);
    let results = delete(&dir, &req).unwrap();
    let evidence_id = id("evidence-synthetic");
    let digest = compute_deletion_evidence_digest(
        &evidence_id,
        &req.params().delete_request_id,
        DeletionOverallStatus::Completed,
        &results,
    )
    .unwrap();
    let evidence = DeletionEvidence::new(DeletionEvidenceParams {
        deletion_evidence_id: evidence_id.clone(),
        delete_request_id: req.params().delete_request_id.clone(),
        previous_evidence_id: None,
        namespace_id: id(NS),
        device_id: id(DEVICE),
        overall_status: DeletionOverallStatus::Completed,
        component_results: results,
        started_at: at(),
        finished_at: Some(at()),
        verified_by: ProducerRef::new(
            ProducerType::TestFixture,
            id("verifier-synthetic"),
            text("1"),
        ),
        evidence_digest: digest,
    })
    .unwrap();
    store_evidence(&dir, &evidence, || Ok(key())).unwrap();
    let db = EncryptedCaptureDatabase::open(
        root.0.join("library.sqlite3"),
        NS,
        DEVICE,
        PROVIDER_PROFILE,
    )
    .unwrap();
    assert_eq!(
        db.load_object_deletion_evidence(&evidence_id).unwrap(),
        Some(evidence)
    );
}

#[test]
fn new_capture_and_abandonment_keep_v11_tombstones_after_completed_deletion() {
    let (root, dir) = setup(true);
    delete(&dir, &request(&["legacy-source"], &[])).unwrap();
    let req = capture_request("new-source", "new-lineage", 1, BODY, None);
    interrupt(&dir, &req, CaptureStep::Published);
    let target = crate::abandonment::inspect(&dir, NS, DEVICE, || Ok(key()))
        .unwrap()
        .unwrap();
    crate::abandonment::abandon(&dir, NS, DEVICE, &target, || Ok(key())).unwrap();
    let report = verify(&dir, false);
    assert_eq!(report.deleted_objects, 1);
    assert_eq!(report.abandoned_attempts, 1);
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        11
    );
}

#[test]
fn late_file_replacement_and_reappearance_are_retained_without_success() {
    for stop in [
        Step::Filesystem(RetirementStep::BeforeRemoval),
        Step::BeforeRetirementCommit,
        Step::BeforeReadBack,
    ] {
        let (root, dir) = setup(true);
        let req = request(&["legacy-source"], &[]);
        let object = paths(&root, "legacy-source").0;
        let bytes = fs::read(&object).unwrap();
        let held = fs::File::open(&object).unwrap();
        assert!(
            execute_with_step(
                &dir,
                &req,
                &execution(),
                || Ok(key()),
                |s| {
                    if s == stop {
                        if object.exists() {
                            fs::remove_file(&object).unwrap();
                        }
                        fs::write(&object, &bytes).unwrap();
                    }
                    Ok(())
                }
            )
            .is_err()
        );
        assert_eq!(fs::read(&object).unwrap(), bytes);
        if stop != Step::BeforeReadBack {
            assert_eq!(count(&root, "radishmemory_deletion_execution_results"), 0);
        }
        drop(held);
    }
}

#[test]
fn exclusive_session_blocks_other_writers_and_pending_deletion_blocks_capture() {
    let (root, dir) = setup(true);
    let req = request(&["legacy-source"], &[]);
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    c.busy_timeout(std::time::Duration::ZERO).unwrap();
    assert!(
        execute_with_step(
            &dir,
            &req,
            &execution(),
            || Ok(key()),
            |s| {
                if s == Step::IntentCommitted {
                    assert!(c.execute_batch("BEGIN IMMEDIATE").is_err());
                    return Err(failure());
                }
                Ok(())
            }
        )
        .is_err()
    );
    drop(c);
    assert!(
        run(
            &dir,
            &capture_request("new-source", "new-lineage", 1, BODY, None)
        )
        .is_err()
    );
    delete(&dir, &req).unwrap();
    run(
        &dir,
        &capture_request("new-source", "new-lineage", 1, BODY, None),
    )
    .unwrap();
}

#[test]
fn missing_key_is_bounded_and_does_not_commit_a_request() {
    let (root, dir) = setup(true);
    for code in [
        crate::SourceVaultErrorCode::KeyMissing,
        crate::SourceVaultErrorCode::KeyStoreLocked,
        crate::SourceVaultErrorCode::KeyStoreDenied,
    ] {
        let err = execute(
            &dir,
            &request(&["legacy-source"], &[]),
            &execution(),
            || Err(SourceVaultError::new(code, "synthetic key failure")),
        )
        .unwrap_err();
        let message = format!("{err:?} {err}");
        for private in [NS, DEVICE, BODY, root.0.to_str().unwrap()] {
            assert!(!message.contains(private));
        }
        assert_eq!(count(&root, "radishmemory_delete_requests"), 0);
    }
}

#[test]
fn migrated_legacy_pending_request_is_adopted_with_original_scope() {
    let root = TestDirectory::new();
    let mut db = radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).unwrap();
    db.capture_source(&capture_request(
        "legacy-source",
        "legacy-lineage",
        1,
        BODY,
        None,
    ))
    .unwrap();
    let req = request(&["legacy-source"], &[]);
    db.store_delete_request(&req).unwrap();
    drop(db);
    let dir = ObjectDirectory::open_application_directory(&root.0).unwrap();
    crate::bootstrap::initialize(&dir, NS, DEVICE, |_| Ok(key())).unwrap();
    crate::body_migration::migrate(&dir, NS, DEVICE, || Ok(key())).unwrap();
    let before = files(&root);
    assert_eq!(before.len(), 1);
    assert!(
        delete(&dir, &req)
            .unwrap()
            .iter()
            .all(|r| r.params().status == ComponentStatus::Succeeded)
    );
    assert!(files(&root).is_empty());
    assert_eq!(verify(&dir, true).deleted_objects, 1);
    assert_eq!(count(&root, "radishmemory_delete_requests"), 1);
    assert_eq!(count(&root, "radishmemory_deletion_execution_results"), 10);
    assert_eq!(count(&root, "radishmemory_recall_fts"), 0);
}

#[test]
fn subprocess_exit_at_durable_boundaries_resumes_the_same_request() {
    for stop in ["intent", "unlinked", "retired", "executed"] {
        let (root, dir) = setup(true);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "deletion::tests::subprocess_deletion_helper",
                "--ignored",
            ])
            .env("RADISHMEMORY_DELETION_SYNTHETIC_ROOT", &root.0)
            .env("RADISHMEMORY_DELETION_STEP", stop)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(74));
        assert_eq!(count(&root, "radishmemory_source_vault_references"), 0);
        assert_eq!(count(&root, "radishmemory_recall_fts"), 0);
        verify(&dir, true);
        delete(&dir, &request(&["legacy-source"], &[])).unwrap();
        assert!(files(&root).is_empty());
        assert_eq!(verify(&dir, false).deleted_objects, 1);
    }
}

#[test]
#[ignore = "invoked only by parent with an isolated synthetic root"]
fn subprocess_deletion_helper() {
    let Some(root) = std::env::var_os("RADISHMEMORY_DELETION_SYNTHETIC_ROOT") else {
        return;
    };
    let stop = std::env::var("RADISHMEMORY_DELETION_STEP").unwrap();
    let dir = ObjectDirectory::open_application_directory(PathBuf::from(root)).unwrap();
    execute_with_step(
        &dir,
        &request(&["legacy-source"], &[]),
        &execution(),
        || Ok(key()),
        |s| {
            if (s == Step::IntentCommitted && stop == "intent")
                || (s == Step::Filesystem(RetirementStep::ObjectRemoved) && stop == "unlinked")
                || (s == Step::RetirementCommitted && stop == "retired")
                || (s == Step::Executed && stop == "executed")
            {
                std::process::exit(74);
            }
            Ok(())
        },
    )
    .unwrap();
    panic!("synthetic checkpoint not reached");
}

#[path = "legacy_tests.rs"]
mod legacy;

#[test]
fn deletion_identity_checks_preserve_process_exclusion_through_retirement() {
    let (root, dir) = setup(true);
    execute_with_step(
        &dir,
        &request(&["legacy-source"], &[]),
        &execution(),
        || Ok(key()),
        |step| {
            if matches!(
                step,
                Step::IntentCommitted | Step::RetirementCommitted | Step::Executed
            ) {
                crate::test_support::assert_database_writer_locked(&root.0);
            }
            Ok(())
        },
    )
    .unwrap();
}
