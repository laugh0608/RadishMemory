use super::*;
use crate::SourceVaultErrorCode;
use crate::capture::{
    Step as CaptureStep,
    tests::{BODY, DEVICE, NS, count, files, interrupt, request, run, setup},
};
use crate::test_support::{TestDirectory, key};
use rusqlite::Connection;
use std::{fs, path::PathBuf};

fn check(dir: &ObjectDirectory) -> Result<ReconciliationReport> {
    reconcile(dir, NS, DEVICE, || Ok(key()))
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
fn committed() -> (TestDirectory, ObjectDirectory, PathBuf, PathBuf) {
    let (root, dir) = setup(true);
    run(&dir, &request("source-a", "lineage-a", 1, BODY, None)).unwrap();
    let (object, staging) = paths(&root, "source-a");
    fs::hard_link(&object, &staging).unwrap();
    (root, dir, object, staging)
}
fn failure() -> VaultMaintenanceError {
    SourceVaultError::new(
        SourceVaultErrorCode::AttemptMismatch,
        "synthetic interruption",
    )
    .into()
}

#[test]
fn completed_v8_inventory_is_verified_without_schema_or_object_changes() {
    let (root, dir) = setup(true);
    let before = files(&root);
    assert_eq!(
        check(&dir).unwrap(),
        ReconciliationReport {
            committed_objects_verified: 1,
            pending_capture: None,
            abandonment_pending: false,
            abandoned_attempts: 0,
            deletion_pending_objects: 0,
            deleted_objects: 0,
            committed_staging_links_removed: 0,
        }
    );
    assert_eq!(files(&root), before);
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        8
    );
}

#[test]
fn pending_captures_are_reported_and_preserved_in_all_four_filesystem_states() {
    for state in [
        AttemptState::Absent,
        AttemptState::AuthenticatedStaging,
        AttemptState::AuthenticatedPublishedCandidate,
        AttemptState::AuthenticatedStagingAndPublishedCandidate,
    ] {
        let (root, dir) = setup(false);
        let req = request("source-a", "lineage-a", 1, BODY, None);
        interrupt(
            &dir,
            &req,
            if state == AttemptState::Absent {
                CaptureStep::Prepared
            } else {
                CaptureStep::Published
            },
        );
        let (object, staging) = paths(&root, "source-a");
        match state {
            AttemptState::AuthenticatedStaging => fs::rename(&object, &staging).unwrap(),
            AttemptState::AuthenticatedStagingAndPublishedCandidate => {
                fs::hard_link(&object, &staging).unwrap()
            }
            _ => (),
        }
        let object_before = fs::read(&object).ok();
        let staging_before = fs::read(&staging).ok();
        for _ in 0..2 {
            let report = check(&dir).unwrap();
            assert_eq!(report.pending_capture, Some(state));
            assert_eq!(report.committed_objects_verified, 0);
            assert_eq!(report.committed_staging_links_removed, 0);
        }
        assert_eq!(fs::read(&object).ok(), object_before);
        assert_eq!(fs::read(&staging).ok(), staging_before);
        assert_eq!(count(&root, "radishmemory_source_artifacts"), 0);
        assert_eq!(count(&root, "radishmemory_source_vault_references"), 0);
        run(&dir, &req).unwrap();
        assert_eq!(check(&dir).unwrap().pending_capture, None);
    }
}

#[test]
fn committed_duplicate_staging_is_removed_without_modifying_objects_or_sqlite() {
    let (root, dir, _, staging) = committed();
    let before = files(&root);
    let database = fs::read(root.0.join("library.sqlite3")).unwrap();
    let report = check(&dir).unwrap();
    assert_eq!(report.committed_objects_verified, 2);
    assert_eq!(report.committed_staging_links_removed, 1);
    assert_eq!(report.pending_capture, None);
    assert!(!staging.exists());
    assert_eq!(files(&root), before);
    assert_eq!(fs::read(root.0.join("library.sqlite3")).unwrap(), database);
    assert_eq!(check(&dir).unwrap().committed_staging_links_removed, 0);
}

#[test]
fn unknown_entries_and_broken_facts_block_all_cleanup() {
    for mode in 0..4 {
        let (root, dir, _, staging) = committed();
        let unknown = root.0.join("source-objects-v1").join("unknown-retained");
        match mode {
            0 => fs::write(&unknown, b"synthetic unknown bytes").unwrap(),
            1 => {
                let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
                c.execute(
                    "UPDATE radishmemory_recall_fts SET content='synthetic corrupt index'",
                    [],
                )
                .unwrap();
            }
            2 => {
                let (object, _) = paths(&root, "legacy-source");
                fs::remove_file(object).unwrap();
            }
            _ => {
                let (object, _) = paths(&root, "legacy-source");
                fs::write(object, b"synthetic truncation").unwrap();
            }
        }
        assert!(check(&dir).is_err());
        assert!(staging.exists());
        if mode == 0 {
            assert!(unknown.exists());
        }
    }
}

#[test]
fn missing_committed_object_is_not_republished_from_its_staging_link() {
    let (_root, dir, object, staging) = committed();
    fs::remove_file(&object).unwrap();
    assert!(check(&dir).is_err());
    assert!(!object.exists());
    assert!(staging.exists());
}

#[test]
fn identical_ciphertext_in_a_different_file_is_ambiguous_even_for_capture_retry() {
    let (root, dir, object, staging) = committed();
    fs::remove_file(&staging).unwrap();
    fs::copy(&object, &staging).unwrap();
    assert!(check(&dir).is_err());
    assert!(run(&dir, &request("source-a", "lineage-a", 1, BODY, None)).is_err());
    assert!(staging.exists());
    assert_eq!(count(&root, "radishmemory_source_artifacts"), 2);
}

#[test]
fn replacement_after_preflight_is_retained_and_returns_no_success_report() {
    let (_root, dir, object, staging) = committed();
    let mut swapped = false;
    let result = reconcile_with_step(
        &dir,
        NS,
        DEVICE,
        || Ok(key()),
        |step| {
            if step == Step::BeforeCleanup && !swapped {
                fs::remove_file(&staging).unwrap();
                fs::copy(&object, &staging).unwrap();
                swapped = true;
            }
            Ok(())
        },
    );
    assert!(result.is_err());
    assert!(staging.exists());
}

#[test]
fn key_failures_do_not_clean_and_diagnostics_are_bounded() {
    let (root, dir, _, staging) = committed();
    for code in [
        SourceVaultErrorCode::KeyMissing,
        SourceVaultErrorCode::KeyStoreLocked,
        SourceVaultErrorCode::KeyStoreDenied,
    ] {
        let error = reconcile(&dir, NS, DEVICE, || {
            Err(SourceVaultError::new(code, "synthetic key failure"))
        })
        .unwrap_err();
        let diagnostic = format!("{error:?} {error}");
        for private in [NS, DEVICE, BODY, root.0.to_str().unwrap()] {
            assert!(!diagnostic.contains(private));
        }
        assert!(staging.exists());
    }
    assert!(reconcile(&dir, NS, DEVICE, || Ok(KeyEncryptionKey::new([0x12; 32]))).is_err());
    assert!(staging.exists());
}

#[test]
fn failures_at_each_checkpoint_are_retryable_without_changing_canonical_objects() {
    for stop in [
        Step::Validated,
        Step::BeforeCleanup,
        Step::AfterCleanup,
        Step::BeforeReadBack,
    ] {
        let (root, dir, _, staging) = committed();
        let before = files(&root);
        assert!(
            reconcile_with_step(
                &dir,
                NS,
                DEVICE,
                || Ok(key()),
                |step| { if step == stop { Err(failure()) } else { Ok(()) } }
            )
            .is_err()
        );
        check(&dir).unwrap();
        assert!(!staging.exists());
        assert_eq!(files(&root), before);
        assert_eq!(count(&root, "radishmemory_source_capture_audit"), 2);
    }
}

#[test]
fn exclusive_session_rejects_concurrent_writes_and_readback_detects_late_corruption() {
    let (root, dir, object, _) = committed();
    let second = Connection::open(root.0.join("library.sqlite3")).unwrap();
    second.busy_timeout(std::time::Duration::ZERO).unwrap();
    assert!(
        reconcile_with_step(
            &dir,
            NS,
            DEVICE,
            || Ok(key()),
            |step| {
                if step == Step::Validated {
                    assert!(second.execute_batch("BEGIN IMMEDIATE").is_err());
                }
                if step == Step::BeforeReadBack {
                    fs::write(&object, b"synthetic late corruption").unwrap();
                }
                Ok(())
            }
        )
        .is_err()
    );
    assert!(check(&dir).is_err());
}

#[test]
fn incomplete_migration_is_rejected_before_key_access() {
    let root = TestDirectory::new();
    let dir = ObjectDirectory::open_application_directory(&root.0).unwrap();
    crate::bootstrap::initialize(&dir, NS, DEVICE, |_| Ok(key())).unwrap();
    let called = std::cell::Cell::new(false);
    assert!(
        reconcile(&dir, NS, DEVICE, || {
            called.set(true);
            Ok(key())
        })
        .is_err()
    );
    assert!(!called.get());
}

#[test]
fn process_exit_before_and_after_cleanup_recovers_without_losing_committed_objects() {
    for step in ["before-cleanup", "before-readback"] {
        let (root, dir, _, staging) = committed();
        let before = files(&root);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "reconciliation::tests::subprocess_reconciliation_helper",
                "--ignored",
            ])
            .env("RADISHMEMORY_RECONCILE_SYNTHETIC_ROOT", &root.0)
            .env("RADISHMEMORY_RECONCILE_STEP", step)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(74));
        assert_eq!(staging.exists(), step == "before-cleanup");
        check(&dir).unwrap();
        assert!(!staging.exists());
        assert_eq!(files(&root), before);
        assert_eq!(count(&root, "radishmemory_source_vault_references"), 2);
    }
}

#[test]
#[ignore = "invoked only by parent with an isolated synthetic root"]
fn subprocess_reconciliation_helper() {
    let Some(root) = std::env::var_os("RADISHMEMORY_RECONCILE_SYNTHETIC_ROOT") else {
        return;
    };
    let stop = std::env::var("RADISHMEMORY_RECONCILE_STEP").unwrap();
    let dir = ObjectDirectory::open_application_directory(PathBuf::from(root)).unwrap();
    reconcile_with_step(
        &dir,
        NS,
        DEVICE,
        || Ok(key()),
        |step| {
            if (step == Step::BeforeCleanup && stop == "before-cleanup")
                || (step == Step::BeforeReadBack && stop == "before-readback")
            {
                std::process::exit(74);
            }
            Ok(())
        },
    )
    .unwrap();
    panic!("synthetic interruption point not reached");
}
