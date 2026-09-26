use super::*;
use crate::SourceVaultErrorCode;
use crate::capture::{
    Step as CaptureStep,
    tests::{
        BODY, DEVICE, NS, count, files, interrupt, request, run, setup, setup_confirmed_memory,
    },
};
use crate::test_support::{TestDirectory, key};
use rusqlite::Connection;
use std::{fs, path::PathBuf};

fn check(dir: &ObjectDirectory, rebuild: bool) -> Result<VerificationReport> {
    maintain(dir, NS, DEVICE, rebuild, || Ok(key()))
}
fn execute(root: &TestDirectory, sql: &str) {
    Connection::open(root.0.join("library.sqlite3"))
        .unwrap()
        .execute_batch(sql)
        .unwrap();
}
fn corrupt(root: &TestDirectory) {
    execute(
        root,
        "DELETE FROM radishmemory_source_lineage_tips; DELETE FROM radishmemory_memory_current_projection; UPDATE radishmemory_recall_fts SET content='Synthetic corrupt index'",
    );
}
fn snapshot(root: &TestDirectory, canonical_only: bool) -> Vec<(String, Vec<String>)> {
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    let tables: Vec<String> = c
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(std::result::Result::unwrap)
        .collect();
    // FTS shadow segment layout may change during rebuild; compare logical rows.
    tables
        .into_iter()
        .filter(|t| !t.starts_with("radishmemory_recall_fts_"))
        .filter(|t| {
            !canonical_only
                || (!t.starts_with("radishmemory_recall_fts")
                    && t != "radishmemory_source_lineage_tips"
                    && t != "radishmemory_memory_current_projection")
        })
        .map(|t| {
            let mut stmt = c.prepare(&format!("SELECT * FROM \"{t}\"")).unwrap();
            let n = stmt.column_count();
            let mut rows: Vec<String> = stmt
                .query_map([], |r| {
                    let values = (0..n)
                        .map(|i| r.get::<_, rusqlite::types::Value>(i))
                        .collect::<std::result::Result<Vec<_>, _>>()?;
                    Ok(format!("{values:?}"))
                })
                .unwrap()
                .map(std::result::Result::unwrap)
                .collect();
            rows.sort();
            (t, rows)
        })
        .collect()
}
fn object(root: &TestDirectory) -> PathBuf {
    fs::read_dir(root.0.join("source-objects-v1"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path()
}
fn failure() -> VaultMaintenanceError {
    SourceVaultError::new(
        SourceVaultErrorCode::AttemptMismatch,
        "synthetic interruption",
    )
    .into()
}

#[test]
fn rebuild_repairs_all_three_derivations_from_history_and_confirmed_memory() {
    let (root, dir) = setup_confirmed_memory();
    run(
        &dir,
        &request(
            "new-source",
            "legacy-lineage",
            2,
            "Synthetic next version",
            Some("legacy-source"),
        ),
    )
    .unwrap();
    let original = snapshot(&root, false);
    let canonical = snapshot(&root, true);
    let objects = files(&root);
    corrupt(&root);
    assert!(check(&dir, false).is_err());
    let report = check(&dir, true).unwrap();
    assert!(report.derivations_rebuilt);
    assert_eq!(report.committed_objects_verified, 2);
    assert_eq!(snapshot(&root, true), canonical);
    assert_eq!(snapshot(&root, false), original);
    assert_eq!(files(&root), objects);
    assert_eq!(count(&root, "radishmemory_source_bodies"), 0);
    assert!(!check(&dir, false).unwrap().derivations_rebuilt);
    check(&dir, true).unwrap();
    assert_eq!(snapshot(&root, false), original);
}

#[test]
fn v8_verification_is_observational_and_rebuild_keeps_the_schema() {
    let (root, dir) = setup(true);
    let before = fs::read(root.0.join("library.sqlite3")).unwrap();
    assert_eq!(check(&dir, false).unwrap().committed_objects_verified, 1);
    assert_eq!(fs::read(root.0.join("library.sqlite3")).unwrap(), before);
    let canonical = snapshot(&root, true);
    corrupt(&root);
    check(&dir, true).unwrap();
    assert_eq!(snapshot(&root, true), canonical);
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        8
    );
    drop(c);
    assert!(radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).is_err());
}

#[test]
fn pending_and_abandoned_attempts_are_preserved_without_cleanup_or_replay() {
    for abandon in [false, true] {
        let (root, dir) = setup(true);
        let req = request("pending-source", "pending-lineage", 1, BODY, None);
        interrupt(&dir, &req, CaptureStep::Published);
        if abandon {
            let target = crate::abandonment::inspect(&dir, NS, DEVICE, || Ok(key()))
                .unwrap()
                .unwrap();
            crate::abandonment::abandon(&dir, NS, DEVICE, &target, || Ok(key())).unwrap();
        }
        let canonical = snapshot(&root, true);
        let objects = files(&root);
        corrupt(&root);
        let report = check(&dir, true).unwrap();
        assert_eq!(report.pending_captures, usize::from(!abandon));
        assert_eq!(report.abandoned_attempts, usize::from(abandon));
        assert_eq!(snapshot(&root, true), canonical);
        assert_eq!(files(&root), objects);
        assert_eq!(run(&dir, &req).is_err(), abandon);
    }
}

#[test]
fn bad_objects_canonical_facts_and_unknown_files_never_get_repaired() {
    for mode in 0..8 {
        let (root, dir) = setup_confirmed_memory();
        corrupt(&root);
        match mode {
            0 => fs::remove_file(object(&root)).unwrap(),
            1 => fs::write(object(&root), b"Synthetic truncation").unwrap(),
            2 => fs::write(
                root.0.join("source-objects-v1/unknown-retained"),
                b"Synthetic unknown",
            )
            .unwrap(),
            3 => execute(
                &root,
                "UPDATE radishmemory_memory_proposals SET content_text='Synthetic tamper'",
            ),
            4 => execute(&root, "DELETE FROM radishmemory_source_capture_audit"),
            5 => execute(
                &root,
                "UPDATE radishmemory_source_artifacts SET content_length=1",
            ),
            6 => execute(&root, "CREATE TABLE unexpected_schema(value TEXT)"),
            _ => execute(&root, "DELETE FROM radishmemory_source_origin_bindings"),
        }
        let before = snapshot(&root, false);
        for rebuild in [false, true] {
            assert!(check(&dir, rebuild).is_err(), "mode {mode}");
            assert_eq!(snapshot(&root, false), before);
        }
    }
}

#[test]
fn abandoning_intent_survives_rebuild_and_requires_explicit_cleanup() {
    let (root, dir) = setup(true);
    let req = request("pending-source", "pending-lineage", 1, BODY, None);
    interrupt(&dir, &req, CaptureStep::Published);
    let target = crate::abandonment::inspect(&dir, NS, DEVICE, || Ok(key()))
        .unwrap()
        .unwrap();
    let mut db = EncryptedCaptureDatabase::open(
        root.0.join("library.sqlite3"),
        NS,
        DEVICE,
        PROVIDER_PROFILE,
    )
    .unwrap();
    db.begin_abandonment(&target).unwrap();
    drop(db);
    let canonical = snapshot(&root, true);
    let objects = files(&root);
    corrupt(&root);
    assert!(check(&dir, true).unwrap().abandonment_pending);
    assert_eq!(snapshot(&root, true), canonical);
    assert_eq!(files(&root), objects);
    assert!(run(&dir, &req).is_err());
    crate::abandonment::abandon(&dir, NS, DEVICE, &target, || Ok(key())).unwrap();
    assert!(!check(&dir, false).unwrap().abandonment_pending);
}

#[test]
fn incomplete_migration_is_rejected_before_key_access() {
    let root = TestDirectory::new();
    let dir = ObjectDirectory::open_application_directory(&root.0).unwrap();
    crate::bootstrap::initialize(&dir, NS, DEVICE, |_| Ok(key())).unwrap();
    for rebuild in [false, true] {
        let called = std::cell::Cell::new(false);
        assert!(
            maintain(&dir, NS, DEVICE, rebuild, || {
                called.set(true);
                Ok(key())
            })
            .is_err()
        );
        assert!(!called.get());
    }
}

#[test]
fn damaged_fts_internal_index_is_rejected_even_when_content_rows_are_intact() {
    let (root, dir) = setup(true);
    execute(
        &root,
        "UPDATE radishmemory_recall_fts_data SET block=zeroblob(length(block)) WHERE id>10",
    );
    assert_eq!(count(&root, "radishmemory_recall_fts"), 1);
    let before = fs::read(root.0.join("library.sqlite3")).unwrap();
    for rebuild in [false, true] {
        assert!(check(&dir, rebuild).is_err());
        assert_eq!(fs::read(root.0.join("library.sqlite3")).unwrap(), before);
    }
}

#[test]
fn wrong_or_missing_keys_are_bounded_and_leave_damage_unchanged() {
    let (root, dir) = setup(true);
    corrupt(&root);
    let before = snapshot(&root, false);
    for rebuild in [false, true] {
        assert!(
            maintain(&dir, NS, DEVICE, rebuild, || Ok(KeyEncryptionKey::new(
                [0x12; 32]
            )))
            .is_err()
        );
        for code in [
            SourceVaultErrorCode::KeyMissing,
            SourceVaultErrorCode::KeyStoreLocked,
            SourceVaultErrorCode::KeyStoreDenied,
        ] {
            let err = maintain(&dir, NS, DEVICE, rebuild, || {
                Err(SourceVaultError::new(code, "synthetic key failure"))
            })
            .unwrap_err();
            let diagnostic = format!("{err:?} {err}");
            for private in [NS, DEVICE, BODY, root.0.to_str().unwrap()] {
                assert!(!diagnostic.contains(private));
            }
        }
        assert_eq!(snapshot(&root, false), before);
    }
}

#[test]
fn checkpoints_roll_back_before_commit_and_allow_retry_after_commit() {
    for stop in [
        Step::Authenticated,
        Step::BeforeCommit,
        Step::Committed,
        Step::BeforeReadBack,
    ] {
        let (root, dir) = setup(true);
        let canonical = snapshot(&root, true);
        corrupt(&root);
        let damaged = snapshot(&root, false);
        assert!(
            maintain_with_step(
                &dir,
                NS,
                DEVICE,
                true,
                || Ok(key()),
                |s| if s == stop { Err(failure()) } else { Ok(()) }
            )
            .is_err()
        );
        if matches!(stop, Step::Authenticated | Step::BeforeCommit) {
            assert_eq!(snapshot(&root, false), damaged);
        } else {
            check(&dir, false).unwrap();
        }
        check(&dir, true).unwrap();
        assert_eq!(snapshot(&root, true), canonical);
    }
}

#[test]
fn exclusive_session_and_reauthentication_prevent_false_success() {
    for stop in [Step::BeforeCommit, Step::BeforeReadBack] {
        let (root, dir) = setup(true);
        corrupt(&root);
        let damaged = snapshot(&root, false);
        let path = object(&root);
        let second = Connection::open(root.0.join("library.sqlite3")).unwrap();
        second.busy_timeout(std::time::Duration::ZERO).unwrap();
        assert!(
            maintain_with_step(
                &dir,
                NS,
                DEVICE,
                true,
                || Ok(key()),
                |s| {
                    if s == Step::Authenticated {
                        assert!(second.execute_batch("BEGIN IMMEDIATE").is_err());
                    }
                    if s == stop {
                        fs::write(&path, b"Synthetic late damage").unwrap();
                    }
                    Ok(())
                }
            )
            .is_err()
        );
        drop(second);
        if stop == Step::BeforeCommit {
            assert_eq!(snapshot(&root, false), damaged);
        }
        assert!(check(&dir, false).is_err());
    }
}

#[test]
fn subprocess_exit_before_and_after_commit_recovers_atomically() {
    for stop in ["before-commit", "committed"] {
        let (root, dir) = setup(true);
        let canonical = snapshot(&root, true);
        corrupt(&root);
        let damaged = snapshot(&root, false);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "maintenance::tests::subprocess_maintenance_helper",
                "--ignored",
            ])
            .env("RADISHMEMORY_MAINTENANCE_SYNTHETIC_ROOT", &root.0)
            .env("RADISHMEMORY_MAINTENANCE_STEP", stop)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(74));
        if stop == "before-commit" {
            assert_eq!(snapshot(&root, false), damaged);
        } else {
            check(&dir, false).unwrap();
        }
        check(&dir, true).unwrap();
        assert_eq!(snapshot(&root, true), canonical);
    }
}

#[test]
#[ignore = "invoked only by parent with an isolated synthetic root"]
fn subprocess_maintenance_helper() {
    let Some(root) = std::env::var_os("RADISHMEMORY_MAINTENANCE_SYNTHETIC_ROOT") else {
        return;
    };
    let stop = std::env::var("RADISHMEMORY_MAINTENANCE_STEP").unwrap();
    let dir = ObjectDirectory::open_application_directory(PathBuf::from(root)).unwrap();
    maintain_with_step(
        &dir,
        NS,
        DEVICE,
        true,
        || Ok(key()),
        |s| {
            if (s == Step::BeforeCommit && stop == "before-commit")
                || (s == Step::Committed && stop == "committed")
            {
                std::process::exit(74);
            }
            Ok(())
        },
    )
    .unwrap();
    panic!("synthetic checkpoint not reached");
}
