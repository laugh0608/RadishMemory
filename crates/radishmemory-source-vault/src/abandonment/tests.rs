use super::*;
use crate::AttemptState;
use crate::capture::{
    Step as CaptureStep,
    tests::{BODY, DEVICE, NS, count, files, interrupt, request, run, setup},
};
use crate::test_support::{TestDirectory, key};
use rusqlite::Connection;
use std::{fs, path::PathBuf, process::Command};

fn inspect_target(dir: &ObjectDirectory) -> CaptureAbandonmentTarget {
    inspect(dir, NS, DEVICE, || Ok(key())).unwrap().unwrap()
}
fn execute(dir: &ObjectDirectory, target: &CaptureAbandonmentTarget) -> Result<AbandonmentReport> {
    abandon(dir, NS, DEVICE, target, || Ok(key()))
}
fn paths(root: &TestDirectory) -> (PathBuf, PathBuf) {
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    let (l,a):(String,String)=c.query_row("SELECT locator,attempt_id FROM radishmemory_source_vault_attempts WHERE source_id='source-a'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
    (
        root.0.join("source-objects-v1").join(format!("{l}.rmo")),
        root.0
            .join("source-staging-v1")
            .join(format!("{l}.{a}.stage")),
    )
}
fn state(root: &TestDirectory) -> String {
    Connection::open(root.0.join("library.sqlite3"))
        .unwrap()
        .query_row(
            "SELECT state FROM radishmemory_source_vault_attempts WHERE source_id='source-a'",
            [],
            |r| r.get(0),
        )
        .unwrap()
}
fn pending(state: AttemptState) -> (TestDirectory, ObjectDirectory, CaptureAbandonmentTarget) {
    let (root, dir) = setup(true);
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
    let (object, stage) = paths(&root);
    match state {
        AttemptState::AuthenticatedStaging => fs::rename(object, stage).unwrap(),
        AttemptState::AuthenticatedStagingAndPublishedCandidate => {
            fs::hard_link(object, stage).unwrap()
        }
        _ => (),
    }
    let target = inspect_target(&dir);
    (root, dir, target)
}
fn failure() -> VaultMaintenanceError {
    SourceVaultError::new(
        SourceVaultErrorCode::AttemptMismatch,
        "synthetic interruption",
    )
    .into()
}
fn points() -> [Step; 9] {
    [
        Step::Validated,
        Step::IntentCommitted,
        Step::Filesystem(RetirementStep::BeforeRemoval),
        Step::Filesystem(RetirementStep::StagingRemoved),
        Step::Filesystem(RetirementStep::ObjectRemoved),
        Step::Filesystem(RetirementStep::DirectoriesSynced),
        Step::BeforeTerminalCommit,
        Step::TerminalCommitted,
        Step::ReadBack,
    ]
}

#[test]
fn all_four_file_states_retire_idempotently_and_new_source_can_use_the_binding() {
    for initial in [
        AttemptState::Absent,
        AttemptState::AuthenticatedStaging,
        AttemptState::AuthenticatedPublishedCandidate,
        AttemptState::AuthenticatedStagingAndPublishedCandidate,
    ] {
        let (root, dir, target) = pending(initial);
        let (object, stage) = paths(&root);
        assert!(!execute(&dir, &target).unwrap().already_abandoned);
        assert_eq!(state(&root), "abandoned");
        assert!(!object.exists() && !stage.exists());
        assert!(execute(&dir, &target).unwrap().already_abandoned);
        assert!(inspect(&dir, NS, DEVICE, || Ok(key())).unwrap().is_none());
        assert!(run(&dir, &request("source-a", "lineage-a", 1, BODY, None)).is_err());
        assert_eq!(count(&root, "radishmemory_source_artifacts"), 1);
        run(&dir, &request("source-new", "lineage-a", 1, BODY, None)).unwrap();
        assert_eq!(count(&root, "radishmemory_source_artifacts"), 2);
        assert_eq!(count(&root, "radishmemory_source_capture_audit"), 2);
        assert_eq!(count(&root, "radishmemory_source_vault_attempts"), 3);
        let report = crate::reconciliation::reconcile(&dir, NS, DEVICE, || Ok(key())).unwrap();
        assert_eq!(report.abandoned_attempts, 1);
        assert!(!report.abandonment_pending);
        assert_eq!(report.committed_objects_verified, 2);
        assert!(radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).is_err());
    }
}

#[test]
fn each_checkpoint_recovers_and_intent_prevents_capture_until_terminal_commit() {
    for stop in points() {
        let (root, dir, target) = pending(AttemptState::AuthenticatedStagingAndPublishedCandidate);
        assert!(
            abandon_with_step(
                &dir,
                NS,
                DEVICE,
                &target,
                || Ok(key()),
                |s| if s == stop { Err(failure()) } else { Ok(()) }
            )
            .is_err()
        );
        let current = state(&root);
        if stop == Step::Validated {
            assert_eq!(current, "prepared");
        } else if matches!(stop, Step::TerminalCommitted | Step::ReadBack) {
            assert_eq!(current, "abandoned");
        } else {
            assert_eq!(current, "abandoning");
            assert!(run(&dir, &request("source-a", "lineage-a", 1, BODY, None)).is_err());
            assert!(run(&dir, &request("source-new", "lineage-new", 1, BODY, None)).is_err());
            let report = crate::reconciliation::reconcile(&dir, NS, DEVICE, || Ok(key())).unwrap();
            assert!(report.abandonment_pending);
            assert_eq!(report.pending_capture, None);
        }
        execute(&dir, &target).unwrap();
        assert_eq!(state(&root), "abandoned");
        assert_eq!(count(&root, "radishmemory_source_artifacts"), 1);
    }
}

#[test]
fn committed_capture_cannot_be_abandoned_with_a_precommit_target() {
    let (root, dir, target) = pending(AttemptState::AuthenticatedPublishedCandidate);
    run(&dir, &request("source-a", "lineage-a", 1, BODY, None)).unwrap();
    let before = files(&root);
    assert!(execute(&dir, &target).is_err());
    assert_eq!(state(&root), "committed");
    assert_eq!(files(&root), before);
}

#[test]
fn stale_resealed_target_and_wrong_library_profile_are_rejected() {
    let (root, dir, old) = pending(AttemptState::Absent);
    interrupt(
        &dir,
        &request("source-a", "lineage-a", 1, BODY, None),
        CaptureStep::Prepared,
    );
    assert!(execute(&dir, &old).is_err());
    assert_eq!(state(&root), "prepared");
    let fresh = inspect_target(&dir);
    assert!(abandon(&dir, NS, "device-other", &fresh, || Ok(key())).is_err());
    let (_other, dir_other, _target_other) = pending(AttemptState::Absent);
    assert!(execute(&dir_other, &fresh).is_err());
    execute(&dir, &fresh).unwrap();
}

#[test]
fn unknown_truncated_copied_objects_and_corrupt_fts_are_retained_before_intent() {
    for mode in 0..4 {
        let (root, dir, target) = pending(AttemptState::AuthenticatedPublishedCandidate);
        let (object, stage) = paths(&root);
        match mode {
            0 => fs::write(
                root.0.join("source-objects-v1/unknown"),
                b"Synthetic unknown",
            )
            .unwrap(),
            1 => fs::write(&object, b"Synthetic truncation").unwrap(),
            2 => {
                fs::copy(&object, &stage).unwrap();
            }
            _ => {
                Connection::open(root.0.join("library.sqlite3"))
                    .unwrap()
                    .execute(
                        "UPDATE radishmemory_recall_fts SET content='Synthetic corruption'",
                        [],
                    )
                    .unwrap();
            }
        }
        assert!(execute(&dir, &target).is_err());
        assert_eq!(state(&root), "prepared");
        assert!(object.exists());
        let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
        assert_eq!(
            c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            9
        );
    }
}

#[test]
fn key_errors_and_redacted_diagnostics_preserve_prepared_files() {
    let (root, dir, target) = pending(AttemptState::AuthenticatedPublishedCandidate);
    let before = files(&root);
    for code in [
        SourceVaultErrorCode::KeyMissing,
        SourceVaultErrorCode::KeyStoreLocked,
        SourceVaultErrorCode::KeyStoreDenied,
    ] {
        let error = abandon(&dir, NS, DEVICE, &target, || {
            Err(SourceVaultError::new(code, "synthetic key failure"))
        })
        .unwrap_err();
        let diagnostic = format!("{target:?} {error:?} {error}");
        for value in [NS, DEVICE, BODY, root.0.to_str().unwrap(), "source-a"] {
            assert!(!diagnostic.contains(value));
        }
    }
    assert!(
        abandon(&dir, NS, DEVICE, &target, || Ok(KeyEncryptionKey::new(
            [0x34; 32]
        )))
        .is_err()
    );
    assert_eq!(state(&root), "prepared");
    assert_eq!(files(&root), before);
}

#[test]
fn file_replacement_before_unlink_and_between_unlinks_is_not_deleted() {
    for point in [
        RetirementStep::BeforeRemoval,
        RetirementStep::StagingRemoved,
    ] {
        let (root, dir, target) = pending(AttemptState::AuthenticatedStagingAndPublishedCandidate);
        let (object, _stage) = paths(&root);
        let saved = root.0.join("retained-synthetic-object");
        assert!(
            abandon_with_step(
                &dir,
                NS,
                DEVICE,
                &target,
                || Ok(key()),
                |s| {
                    if s == Step::Filesystem(point) {
                        fs::rename(&object, &saved).unwrap();
                        fs::write(&object, b"Synthetic replacement").unwrap();
                    }
                    Ok(())
                }
            )
            .is_err()
        );
        assert_eq!(fs::read(&object).unwrap(), b"Synthetic replacement");
        assert_eq!(state(&root), "abandoning");
        assert!(saved.exists());
    }
}

#[test]
fn objects_reappearing_after_sync_or_terminal_state_are_never_silently_removed() {
    for terminal in [false, true] {
        let (root, dir, target) = pending(AttemptState::AuthenticatedPublishedCandidate);
        let (object, _stage) = paths(&root);
        let bytes = fs::read(&object).unwrap();
        let stop = if terminal {
            Step::TerminalCommitted
        } else {
            Step::Filesystem(RetirementStep::DirectoriesSynced)
        };
        assert!(
            abandon_with_step(
                &dir,
                NS,
                DEVICE,
                &target,
                || Ok(key()),
                |s| {
                    if s == stop {
                        fs::write(&object, &bytes).unwrap();
                    }
                    Ok(())
                }
            )
            .is_err()
        );
        assert_eq!(
            state(&root),
            if terminal { "abandoned" } else { "abandoning" }
        );
        assert!(object.exists());
        if terminal {
            assert!(execute(&dir, &target).is_err());
            assert!(crate::reconciliation::reconcile(&dir, NS, DEVICE, || Ok(key())).is_err());
            assert_eq!(fs::read(&object).unwrap(), bytes);
        }
    }
}

#[test]
fn inspection_does_not_upgrade_schema_and_empty_libraries_have_no_target() {
    let (root, dir) = setup(false);
    assert!(inspect(&dir, NS, DEVICE, || Ok(key())).unwrap().is_none());
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        8
    );
}

#[test]
fn abandoning_a_version_preserves_history_and_new_identity_can_advance_the_tip() {
    let (root, dir) = setup(false);
    let first = request(
        "source-original",
        "lineage-a",
        1,
        "Synthetic original",
        None,
    );
    run(&dir, &first).unwrap();
    let committed = files(&root);
    let next = request("source-a", "lineage-a", 2, BODY, Some("source-original"));
    interrupt(&dir, &next, CaptureStep::Published);
    let target = inspect_target(&dir);
    execute(&dir, &target).unwrap();
    assert_eq!(files(&root), committed);
    assert!(run(&dir, &next).is_err());
    let replacement = request(
        "source-replacement",
        "lineage-a",
        2,
        BODY,
        Some("source-original"),
    );
    run(&dir, &replacement).unwrap();
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    let tip: String = c
        .query_row(
            "SELECT source_id FROM radishmemory_source_lineage_tips",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tip, "source-replacement");
    assert_eq!(count(&root, "radishmemory_source_capture_audit"), 2);
    assert_eq!(count(&root, "radishmemory_source_bodies"), 0);
}

#[test]
fn database_replacement_after_intent_cannot_authorize_file_removal() {
    let (root, dir, target) = pending(AttemptState::AuthenticatedPublishedCandidate);
    let (object, _stage) = paths(&root);
    let result = abandon_with_step(
        &dir,
        NS,
        DEVICE,
        &target,
        || Ok(key()),
        |point| {
            if point == Step::IntentCommitted {
                let path = root.0.join("library.sqlite3");
                fs::rename(&path, root.0.join("retained-synthetic-database")).unwrap();
                fs::write(&path, b"Synthetic unrelated database replacement").unwrap();
            }
            Ok(())
        },
    );
    assert!(result.is_err());
    assert!(object.exists());
    assert_eq!(
        fs::read(root.0.join("library.sqlite3")).unwrap(),
        b"Synthetic unrelated database replacement"
    );
}

#[test]
fn preterminal_reappearance_keeps_abandoning_instead_of_claiming_completion() {
    let (root, dir, target) = pending(AttemptState::AuthenticatedPublishedCandidate);
    let (object, _) = paths(&root);
    let bytes = fs::read(&object).unwrap();
    assert!(
        abandon_with_step(
            &dir,
            NS,
            DEVICE,
            &target,
            || Ok(key()),
            |point| {
                if point == Step::BeforeTerminalCommit {
                    fs::write(&object, &bytes).unwrap();
                }
                Ok(())
            }
        )
        .is_err()
    );
    assert_eq!(state(&root), "abandoning");
    assert_eq!(fs::read(&object).unwrap(), bytes);
}

#[cfg(unix)]
#[test]
fn filesystem_permission_failure_preserves_intent_for_retry() {
    use std::os::unix::fs::PermissionsExt;
    let (root, dir, target) = pending(AttemptState::AuthenticatedStagingAndPublishedCandidate);
    let objects = root.0.join("source-objects-v1");
    let original = fs::metadata(&objects).unwrap().permissions();
    let result = abandon_with_step(
        &dir,
        NS,
        DEVICE,
        &target,
        || Ok(key()),
        |point| {
            if point == Step::Filesystem(RetirementStep::BeforeRemoval) {
                fs::set_permissions(&objects, fs::Permissions::from_mode(0o500)).unwrap();
            }
            Ok(())
        },
    );
    fs::set_permissions(&objects, original).unwrap();
    assert!(result.is_err());
    assert_eq!(state(&root), "abandoning");
    assert!(paths(&root).0.exists());
    execute(&dir, &target).unwrap();
}

#[test]
fn exclusive_session_blocks_other_writers_during_cleanup() {
    let (root, dir, target) = pending(AttemptState::AuthenticatedPublishedCandidate);
    let other = Connection::open(root.0.join("library.sqlite3")).unwrap();
    other.busy_timeout(std::time::Duration::ZERO).unwrap();
    abandon_with_step(
        &dir,
        NS,
        DEVICE,
        &target,
        || Ok(key()),
        |_| {
            assert!(other.execute_batch("BEGIN IMMEDIATE").is_err());
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn process_exit_at_each_boundary_resumes_from_persisted_truth() {
    for (index, _) in points().iter().enumerate() {
        let (root, dir, target) = pending(AttemptState::AuthenticatedStagingAndPublishedCandidate);
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "abandonment::tests::subprocess_abandonment_helper",
                "--ignored",
            ])
            .env("RADISHMEMORY_ABANDON_SYNTHETIC_ROOT", &root.0)
            .env("RADISHMEMORY_ABANDON_POINT", index.to_string())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(74));
        execute(&dir, &target).unwrap();
        assert_eq!(state(&root), "abandoned");
        assert_eq!(count(&root, "radishmemory_source_artifacts"), 1);
        assert_eq!(files(&root).len(), 1);
    }
}

#[test]
#[ignore = "parent invokes this helper with an isolated synthetic library"]
fn subprocess_abandonment_helper() {
    let Some(root) = std::env::var_os("RADISHMEMORY_ABANDON_SYNTHETIC_ROOT") else {
        return;
    };
    let index: usize = std::env::var("RADISHMEMORY_ABANDON_POINT")
        .unwrap()
        .parse()
        .unwrap();
    let dir = ObjectDirectory::open_application_directory(PathBuf::from(root)).unwrap();
    let target = inspect_target(&dir);
    abandon_with_step(
        &dir,
        NS,
        DEVICE,
        &target,
        || Ok(key()),
        |s| {
            if s == points()[index] {
                std::process::exit(74);
            }
            Ok(())
        },
    )
    .unwrap();
    panic!("synthetic interruption did not happen");
}
