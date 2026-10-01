use super::*;
use crate::test_support::{TestDirectory, key};
use rusqlite::{Connection, params, types::Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, process::Command};

const NS: &str = "namespace-0123456789abcdef0123456789abcdef";
const DEVICE: &str = "device-fedcba9876543210fedcba9876543210";
const BODY: &[u8] = "合成长期项目资料\r\n# exact bytes\n".as_bytes();

fn setup() -> (TestDirectory, ObjectDirectory) {
    let root = TestDirectory::new();
    let path = root.0.join("library.sqlite3");
    drop(radishmemory_sqlite::SqliteDatabase::open(&path).unwrap());
    let c = Connection::open(&path).unwrap();
    let digest = Sha256::digest(BODY)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    for (id, lineage, version, binding) in [
        ("source-a", "lineage-a", 1, "origin-binding-a"),
        ("source-b", "lineage-a", 2, "origin-binding-a"),
        ("source-c", "lineage-c", 1, "origin-binding-c"),
    ] {
        c.execute("INSERT INTO radishmemory_source_artifacts VALUES (
            ?1, 'radishmemory.m0/1', 'SourceArtifact', ?2, ?3, ?4, 'markdown', 'text/markdown', ?5,
            'sha256', 'exact-bytes-v1', ?6, 'Synthetic title', 'explicit_user_input', ?7,
            '2026-09-26T00:00:00Z', '2026-09-26T00:00:01Z', 'personal', 'local_only', 'until_deleted', NULL, NULL,
            'active', 'policy-synthetic', 'test_fixture', 'producer-synthetic', '1', '2026-09-26T00:00:01Z')",
            params![id, lineage, version, NS, BODY.len() as i64, digest, binding]).unwrap();
        c.execute(
            "INSERT INTO radishmemory_source_bodies VALUES (?1, ?2)",
            params![id, BODY],
        )
        .unwrap();
        c.execute(
            "INSERT OR REPLACE INTO radishmemory_source_lineage_tips VALUES (?1, ?2, ?3, ?4)",
            params![NS, lineage, id, version],
        )
        .unwrap();
        c.execute(
            "INSERT OR IGNORE INTO radishmemory_source_origin_bindings VALUES (?1, ?2, ?3)",
            params![NS, binding, lineage],
        )
        .unwrap();
        c.execute("INSERT INTO radishmemory_source_capture_audit VALUES (?1, ?2, ?3, ?4, '2026-09-26T00:00:01Z')", params![id, NS, binding, if version == 1 {"created"} else {"versioned"}]).unwrap();
        c.execute("INSERT INTO radishmemory_source_fragments VALUES (
            ?1, 'radishmemory.m0/1', 'SourceFragment', ?2, ?3, 0, 0, ?4, 'sha256', 'exact-bytes-v1', ?5,
            'test_fixture', 'segmenter-synthetic', '1', 'personal', 'local_only', 'until_deleted', NULL, NULL,
            'active', 'policy-synthetic', '2026-09-26T00:00:01Z')", params![format!("fragment-{id}"), NS, id, BODY.len() as i64, digest]).unwrap();
    }
    c.execute(
        "INSERT INTO radishmemory_source_supersedes VALUES ('source-b', 0, 'source-a')",
        [],
    )
    .unwrap();
    for id in ["source-b", "source-c"] {
        c.execute("INSERT INTO radishmemory_recall_fts (object_kind, object_id, namespace_id, sensitivity, content) VALUES ('source_fragment', ?1, ?2, 'personal', ?3)", params![format!("fragment-{id}"), NS, std::str::from_utf8(BODY).unwrap()]).unwrap();
    }
    drop(c);
    let dir = ObjectDirectory::open_application_directory(&root.0).unwrap();
    crate::bootstrap::initialize(&dir, NS, DEVICE, |_| Ok(key())).unwrap();
    (root, dir)
}
fn run(dir: &ObjectDirectory) -> Result<BodyMigrationReport> {
    migrate(dir, NS, DEVICE, || Ok(key()))
}
fn interrupt(dir: &ObjectDirectory, point: Step) {
    let error = migrate_with_step(
        dir,
        NS,
        DEVICE,
        || Ok(key()),
        |s| if s == point { Err(invalid()) } else { Ok(()) },
    )
    .unwrap_err();
    assert!(matches!(error, VaultMaintenanceError::Vault(_)));
}
fn count(root: &TestDirectory, table: &str) -> i64 {
    Connection::open(root.0.join("library.sqlite3"))
        .unwrap()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
fn canonical_snapshot(root: &TestDirectory) -> BTreeMap<String, Vec<Vec<Value>>> {
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    let mut stmt = c.prepare("SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap();
    let names = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();
    let mut result = BTreeMap::new();
    for name in names {
        if name == "radishmemory_source_bodies"
            || name == "radishmemory_schema_migrations"
            || name.starts_with("radishmemory_source_vault_")
        {
            continue;
        }
        let mut stmt = c
            .prepare(&format!("SELECT * FROM {name} ORDER BY 1"))
            .unwrap();
        let width = stmt.column_count();
        let rows = stmt
            .query_map([], |r| {
                (0..width)
                    .map(|i| r.get::<_, Value>(i))
                    .collect::<std::result::Result<Vec<_>, _>>()
            })
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        result.insert(name, rows);
    }
    result
}
fn object_files(root: &TestDirectory) -> BTreeMap<std::ffi::OsString, Vec<u8>> {
    fs::read_dir(root.0.join("source-objects-v1"))
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (e.file_name(), fs::read(e.path()).unwrap())
        })
        .collect()
}

#[test]
fn healthy_migration_preserves_canonical_facts_and_exact_bytes_and_is_idempotent() {
    let (root, dir) = setup();
    let before = canonical_snapshot(&root);
    assert_eq!(run(&dir).unwrap().objects_verified, 3);
    assert_eq!(count(&root, "radishmemory_source_bodies"), 0);
    assert_eq!(before, canonical_snapshot(&root));
    let objects = object_files(&root);
    assert_eq!(objects.len(), 3); // Same bytes retain independent provenance and ciphertext.
    assert!(
        objects
            .values()
            .all(|v| !v.windows(BODY.len()).any(|w| w == BODY))
    );
    assert_eq!(run(&dir).unwrap().objects_verified, 3);
    assert_eq!(objects, object_files(&root));
    assert_eq!(count(&root, "radishmemory_source_vault_attempts"), 3);
    assert!(radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).is_err());
}

#[test]
fn all_checkpoint_interruptions_resume_without_losing_unverified_inline_bytes() {
    for point in [
        Step::Inventory,
        Step::Prepared,
        Step::Published,
        Step::ReferenceCommitted,
        Step::ReadBack,
        Step::InlineRetired,
    ] {
        let (root, dir) = setup();
        let before = canonical_snapshot(&root);
        interrupt(&dir, point);
        assert_eq!(
            count(&root, "radishmemory_source_bodies"),
            if point == Step::InlineRetired { 2 } else { 3 }
        );
        let existing = object_files(&root);
        run(&dir).unwrap();
        for (name, bytes) in existing {
            assert_eq!(object_files(&root).get(&name), Some(&bytes));
        }
        assert_eq!(canonical_snapshot(&root), before);
        assert_eq!(count(&root, "radishmemory_source_bodies"), 0);
    }
}

#[test]
fn corrupt_historical_body_blocks_before_any_object_is_published() {
    let (root, dir) = setup();
    interrupt(&dir, Step::Inventory);
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    c.execute(
        "UPDATE radishmemory_source_bodies SET content = X'41' WHERE source_id = 'source-a'",
        [],
    )
    .unwrap();
    drop(c);
    assert!(run(&dir).is_err());
    assert!(object_files(&root).is_empty());
    assert_eq!(count(&root, "radishmemory_source_bodies"), 3);
}

#[test]
fn missing_wrong_or_denied_key_never_replaces_objects_or_retires_bodies() {
    for point in [
        Step::Published,
        Step::ReferenceCommitted,
        Step::InlineRetired,
    ] {
        let (root, dir) = setup();
        interrupt(&dir, point);
        let objects = object_files(&root);
        let bodies = count(&root, "radishmemory_source_bodies");
        for code in [
            SourceVaultErrorCode::KeyMissing,
            SourceVaultErrorCode::KeyStoreDenied,
            SourceVaultErrorCode::KeyStoreLocked,
        ] {
            assert!(
                migrate(&dir, NS, DEVICE, || Err(SourceVaultError::new(
                    code,
                    "synthetic key failure"
                )))
                .is_err()
            );
        }
        assert!(migrate(&dir, NS, DEVICE, || Ok(KeyEncryptionKey::new([0x17; 32]))).is_err());
        assert_eq!(object_files(&root), objects);
        assert_eq!(count(&root, "radishmemory_source_bodies"), bodies);
    }
}

#[test]
fn committed_missing_or_tampered_object_never_falls_back_to_inline_body() {
    for remove in [true, false] {
        let (root, dir) = setup();
        interrupt(&dir, Step::ReferenceCommitted);
        let path = fs::read_dir(root.0.join("source-objects-v1"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        if remove {
            fs::remove_file(path).unwrap();
        } else {
            let mut bytes = fs::read(&path).unwrap();
            let last = bytes.len() - 1;
            bytes[last] ^= 1;
            fs::write(path, bytes).unwrap();
        }
        assert!(run(&dir).is_err());
        assert_eq!(count(&root, "radishmemory_source_bodies"), 3);
    }
}

#[test]
fn unknown_files_and_ambiguous_migration_state_fail_closed_without_cleanup() {
    for child in ["source-objects-v1", "source-staging-v1"] {
        let (root, dir) = setup();
        interrupt(&dir, Step::Inventory);
        let path = root.0.join(child).join("synthetic-unknown");
        fs::write(&path, b"retain unknown").unwrap();
        assert!(run(&dir).is_err());
        assert!(path.exists());
        assert_eq!(count(&root, "radishmemory_source_bodies"), 3);
    }
    for sql in [
        "UPDATE radishmemory_source_vault_attempts SET digest_value = printf('%064d', 0) WHERE source_id = 'source-a'",
        "DELETE FROM radishmemory_source_vault_references WHERE source_id = 'source-a'",
        "UPDATE radishmemory_source_vault_migration SET state = 'objects_ready'",
        "CREATE TRIGGER synthetic_drift AFTER DELETE ON radishmemory_source_bodies BEGIN DELETE FROM radishmemory_source_artifacts; END",
    ] {
        let (root, dir) = setup();
        interrupt(&dir, Step::ReferenceCommitted);
        Connection::open(root.0.join("library.sqlite3"))
            .unwrap()
            .execute_batch(sql)
            .unwrap();
        assert!(run(&dir).is_err());
        assert_eq!(count(&root, "radishmemory_source_bodies"), 3);
    }
}

#[test]
fn authenticated_staging_and_publish_links_resume_without_resealing() {
    for staging_only in [true, false] {
        let (root, dir) = setup();
        interrupt(&dir, Step::Published);
        let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
        let (locator, attempt): (String, String) = c.query_row("SELECT locator, attempt_id FROM radishmemory_source_vault_attempts WHERE state = 'prepared'", [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
        drop(c);
        let target = root
            .0
            .join("source-objects-v1")
            .join(format!("{locator}.rmo"));
        let stage = root
            .0
            .join("source-staging-v1")
            .join(format!("{locator}.{attempt}.stage"));
        let bytes = fs::read(&target).unwrap();
        fs::hard_link(&target, &stage).unwrap();
        if staging_only {
            fs::remove_file(&target).unwrap();
        }
        run(&dir).unwrap();
        assert!(!stage.exists());
        assert_eq!(fs::read(target).unwrap(), bytes);
    }
}

#[test]
fn truncated_staging_is_retained_and_blocks_recovery() {
    let (root, dir) = setup();
    interrupt(&dir, Step::Prepared);
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    let (l,a): (String,String) = c.query_row("SELECT locator, attempt_id FROM radishmemory_source_vault_attempts WHERE state='prepared'", [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
    drop(c);
    let stage = root
        .0
        .join("source-staging-v1")
        .join(format!("{l}.{a}.stage"));
    fs::write(&stage, b"partial encrypted envelope").unwrap();
    assert!(run(&dir).is_err());
    assert!(stage.exists());
    assert_eq!(count(&root, "radishmemory_source_bodies"), 3);
}

#[test]
fn exclusive_session_blocks_other_connections_between_checkpoints() {
    let (root, dir) = setup();
    migrate_with_step(
        &dir,
        NS,
        DEVICE,
        || Ok(key()),
        |_| {
            let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
            c.busy_timeout(std::time::Duration::ZERO).unwrap();
            let error = c.execute_batch("BEGIN IMMEDIATE").unwrap_err();
            assert_eq!(
                error.sqlite_error_code(),
                Some(rusqlite::ErrorCode::DatabaseBusy)
            );
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn abrupt_process_exit_at_commit_boundaries_recovers_from_disk() {
    for point in [
        "Published",
        "ReferenceCommitted",
        "ReadBack",
        "InlineRetired",
    ] {
        let (root, dir) = setup();
        let before = canonical_snapshot(&root);
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "body_migration::tests::crash_helper",
                "--ignored",
            ])
            .env("RADISHMEMORY_SYNTHETIC_MIGRATION_ROOT", &root.0)
            .env("RADISHMEMORY_SYNTHETIC_MIGRATION_STEP", point)
            .output()
            .unwrap()
            .status;
        assert_eq!(status.code(), Some(73));
        run(&dir).unwrap();
        assert_eq!(canonical_snapshot(&root), before);
    }
}
#[test]
#[ignore = "executed by parent with isolated synthetic fixture"]
fn crash_helper() {
    let Some(root) = std::env::var_os("RADISHMEMORY_SYNTHETIC_MIGRATION_ROOT") else {
        return;
    };
    let point = std::env::var("RADISHMEMORY_SYNTHETIC_MIGRATION_STEP").unwrap();
    let dir = ObjectDirectory::open_application_directory(root).unwrap();
    migrate_with_step(
        &dir,
        NS,
        DEVICE,
        || Ok(key()),
        |s| {
            if format!("{s:?}") == point {
                std::process::exit(73);
            }
            Ok(())
        },
    )
    .unwrap();
    panic!("requested checkpoint not reached");
}

#[test]
fn deletion_residuals_migrate_without_resurrecting_deleted_sources() {
    for keep_residual in [true, false] {
        let (root, dir) = setup();
        let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
        c.execute("UPDATE radishmemory_source_artifacts SET deletion_state = 'deleted' WHERE source_id = 'source-a'", []).unwrap();
        c.execute("UPDATE radishmemory_source_fragments SET deletion_state = 'deleted' WHERE source_id = 'source-a'", []).unwrap();
        if !keep_residual {
            c.execute(
                "DELETE FROM radishmemory_source_bodies WHERE source_id = 'source-a'",
                [],
            )
            .unwrap();
        }
        drop(c);
        let before = canonical_snapshot(&root);
        assert_eq!(
            run(&dir).unwrap().objects_verified,
            if keep_residual { 3 } else { 2 }
        );
        assert_eq!(before, canonical_snapshot(&root));
        assert_eq!(count(&root, "radishmemory_source_bodies"), 0);
    }
}

#[test]
fn fresh_or_v6_library_cannot_enter_recovery_or_access_provider() {
    let root = TestDirectory::new();
    let dir = ObjectDirectory::open_application_directory(&root.0).unwrap();
    assert!(
        migrate(&dir, NS, DEVICE, || panic!(
            "must not create a database or key"
        ))
        .is_err()
    );
    assert!(!root.0.join("library.sqlite3").exists());
    drop(radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).unwrap());
    assert!(
        migrate(&dir, NS, DEVICE, || panic!(
            "must initialize key separately"
        ))
        .is_err()
    );
}

#[test]
fn missing_key_before_inventory_leaves_v7_and_all_plaintext_intact() {
    let (root, dir) = setup();
    assert!(
        migrate(&dir, NS, DEVICE, || Err(SourceVaultError::new(
            SourceVaultErrorCode::KeyMissing,
            "synthetic missing key"
        )))
        .is_err()
    );
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        7
    );
    assert_eq!(count(&root, "radishmemory_source_bodies"), 3);
    assert!(object_files(&root).is_empty());
}

#[test]
fn partial_resume_still_validates_remaining_body_and_retired_object() {
    for corrupt_inline in [true, false] {
        let (root, dir) = setup();
        interrupt(&dir, Step::InlineRetired);
        if corrupt_inline {
            Connection::open(root.0.join("library.sqlite3")).unwrap().execute("UPDATE radishmemory_source_bodies SET content=X'41' WHERE source_id='source-c'",[]).unwrap();
        } else {
            let path = fs::read_dir(root.0.join("source-objects-v1"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            fs::remove_file(path).unwrap();
        }
        assert!(run(&dir).is_err());
        assert_eq!(count(&root, "radishmemory_source_bodies"), 2);
    }
}

#[cfg(unix)]
#[test]
fn replaced_database_and_symlinked_object_cannot_complete_migration() {
    use std::os::unix::fs::symlink;
    let (root, dir) = setup();
    let path = root.0.join("library.sqlite3");
    assert!(
        migrate_with_step(
            &dir,
            NS,
            DEVICE,
            || Ok(key()),
            |s| {
                if s == Step::Published {
                    fs::rename(&path, root.0.join("synthetic-original.sqlite3")).unwrap();
                    fs::write(&path, b"").unwrap();
                }
                Ok(())
            }
        )
        .is_err()
    );
    let (root, dir) = setup();
    interrupt(&dir, Step::ReferenceCommitted);
    let object = fs::read_dir(root.0.join("source-objects-v1"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let saved = root.0.join("synthetic-saved.rmo");
    fs::rename(&object, &saved).unwrap();
    symlink(&saved, &object).unwrap();
    assert!(run(&dir).is_err());
    assert_eq!(count(&root, "radishmemory_source_bodies"), 3);
}

#[test]
fn diagnostics_redact_database_paths_identifiers_sql_and_tokens() {
    use std::error::Error;
    let (root, dir) = setup();
    interrupt(&dir, Step::ReferenceCommitted);
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    c.execute_batch("CREATE TABLE synthetic_private_drift (secret TEXT)")
        .unwrap();
    drop(c);
    let error = run(&dir).unwrap_err();
    let text = format!("{error} {error:?}");
    for forbidden in [
        root.0.to_str().unwrap(),
        NS,
        DEVICE,
        "synthetic_private_drift",
        "CREATE TABLE",
    ] {
        assert!(!text.contains(forbidden));
    }
    assert!(error.source().is_none());
}

#[test]
fn object_change_after_readback_cannot_remove_the_inline_body() {
    let (root, dir) = setup();
    assert!(
        migrate_with_step(
            &dir,
            NS,
            DEVICE,
            || Ok(key()),
            |s| {
                if s == Step::ReadBack {
                    let object = fs::read_dir(root.0.join("source-objects-v1"))
                        .unwrap()
                        .next()
                        .unwrap()
                        .unwrap()
                        .path();
                    let mut bytes = fs::read(&object).unwrap();
                    let last = bytes.len() - 1;
                    bytes[last] ^= 1;
                    fs::write(object, bytes).unwrap();
                }
                Ok(())
            }
        )
        .is_err()
    );
    assert_eq!(count(&root, "radishmemory_source_bodies"), 3);
}
