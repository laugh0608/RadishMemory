use super::*;
use crate::capture::tests::{
    DEVICE, NS, count, files, request, run, setup, setup_confirmed_memory,
};
use crate::test_support::{TestDirectory, key};
use radishmemory_core::*;
use radishmemory_file_entry::{FileEntryErrorReason, FileExportRequest, export_managed_source};
use rusqlite::Connection;
use std::fs;
fn id(value: &str) -> Identifier {
    Identifier::new(value).unwrap()
}
fn text(value: &str) -> NonEmptyText {
    NonEmptyText::new(value).unwrap()
}
fn at() -> Timestamp {
    Timestamp::parse("2026-09-26T08:00:00Z").unwrap()
}
fn reader(dir: &ObjectDirectory) -> LibraryReader<'_> {
    open(dir, NS, DEVICE, || Ok(key())).unwrap()
}
fn query(value: &str) -> LocalSearchRequest {
    LocalSearchRequest::new(id(NS), text(value), at(), 20, [Sensitivity::Personal]).unwrap()
}
fn sql(root: &TestDirectory, value: &str) {
    Connection::open(root.0.join("library.sqlite3"))
        .unwrap()
        .execute_batch(value)
        .unwrap();
}
fn object(root: &TestDirectory) -> std::path::PathBuf {
    root.0.join("source-objects-v1").join(&files(root)[0].0)
}

#[test]
fn authenticated_export_preserves_exact_bytes_for_migrated_and_captured_versions() {
    let (root, dir) = setup(true);
    let exports = TestDirectory::new();
    let body = "\u{feff}Synthetic export\r\n萝卜 e\u{301} 🥕\r\n";
    let capture = request(
        "next-source",
        "legacy-lineage",
        2,
        body,
        Some("legacy-source"),
    );
    run(&dir, &capture).unwrap();
    let before = files(&root);
    // A second session proves export does not depend on a capture-time body cache.
    for session in 0..2 {
        let r = reader(&dir);
        for (source_id, expected) in [
            ("legacy-source", "Legacy synthetic bytes"),
            ("next-source", body),
        ] {
            let source = r
                .load_source_artifact(&id(NS), &id(source_id))
                .unwrap()
                .unwrap();
            let target = exports.0.join(format!("{session}-{source_id}.txt"));
            let export = FileExportRequest::new(&target, vec![exports.0.clone()]).unwrap();
            let receipt = export_managed_source(&source, &export).unwrap();
            assert_eq!(fs::read(&target).unwrap(), expected.as_bytes());
            assert_eq!(receipt.source_id(), &source.params().source_id);
            assert_eq!(receipt.namespace_id(), &id(NS));
            assert_eq!(receipt.version(), source.params().version);
            assert_eq!(receipt.content_length(), expected.len() as u64);
            assert_eq!(
                receipt.content_digest(),
                &compute_exact_bytes_digest(expected.as_bytes())
            );
            assert_eq!(
                export_managed_source(&source, &export)
                    .unwrap_err()
                    .reason(),
                FileEntryErrorReason::DestinationExists
            );
            assert_eq!(fs::read(&target).unwrap(), expected.as_bytes());
        }
        crate::test_support::assert_database_writer_locked(&root.0);
    }
    assert_eq!(fs::read_dir(&exports.0).unwrap().count(), 4);
    assert_eq!(files(&root), before);
    assert_eq!(count(&root, "radishmemory_source_bodies"), 0);
}

#[test]
fn authenticated_export_rejects_unapproved_destinations_and_preserves_existing_files() {
    let (root, dir) = setup(true);
    let exports = TestDirectory::new();
    let outside = TestDirectory::new();
    let r = reader(&dir);
    let source = r
        .load_source_artifact(&id(NS), &id("legacy-source"))
        .unwrap()
        .unwrap();
    let target = exports.0.join("existing.txt");
    fs::write(&target, b"synthetic user-owned bytes").unwrap();
    let export = FileExportRequest::new(&target, vec![exports.0.clone()]).unwrap();
    assert_eq!(
        export_managed_source(&source, &export)
            .unwrap_err()
            .reason(),
        FileEntryErrorReason::DestinationExists
    );
    assert_eq!(fs::read(&target).unwrap(), b"synthetic user-owned bytes");
    let forbidden = outside.0.join("forbidden.txt");
    let export = FileExportRequest::new(&forbidden, vec![exports.0.clone()]).unwrap();
    assert_eq!(
        export_managed_source(&source, &export)
            .unwrap_err()
            .reason(),
        FileEntryErrorReason::DestinationNotAllowed
    );
    assert!(!forbidden.exists());
    assert_eq!(fs::read_dir(&exports.0).unwrap().count(), 1);
    assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 0);
    crate::test_support::assert_database_writer_locked(&root.0);
}

#[test]
fn authenticated_export_survives_later_vault_deletion_but_source_cannot_be_read_again() {
    let (root, dir) = setup_confirmed_memory();
    let exports = TestDirectory::new();
    let target = exports.0.join("explicit-export.txt");
    let r = reader(&dir);
    let source = r
        .load_source_artifact(&id(NS), &id("legacy-source"))
        .unwrap()
        .unwrap();
    let expected = source.params().content.as_str().as_bytes().to_vec();
    let export = FileExportRequest::new(&target, vec![exports.0.clone()]).unwrap();
    crate::test_support::assert_database_writer_locked(&root.0);
    export_managed_source(&source, &export).unwrap();
    crate::test_support::assert_database_writer_locked(&root.0);
    drop(source);
    drop(r);
    let execution = LocalDeletionExecution::new(
        at(),
        EvidenceRef::new(EvidenceType::PolicyBasis, id("policy-synthetic")),
    )
    .unwrap();
    crate::deletion::execute(&dir, &purge_request(), &execution, || Ok(key())).unwrap();
    let r = reader(&dir);
    assert!(
        r.load_source_artifact(&id(NS), &id("legacy-source"))
            .unwrap()
            .is_none()
    );
    assert!(r.search(&query("Synthetic")).unwrap().is_empty());
    assert!(files(&root).is_empty());
    // An explicitly exported plaintext copy is outside the managed deletion closure.
    assert_eq!(fs::read(&target).unwrap(), expected);
    assert_eq!(fs::read_dir(&exports.0).unwrap().count(), 1);
}

#[test]
fn reads_migrated_and_captured_versions_with_existing_catalog_and_search_semantics() {
    let (root, dir) = setup(true);
    let capture = request(
        "next-source",
        "legacy-lineage",
        2,
        "\u{feff}Synthetic next version needle\r\n萝卜 e\u{301} 🥕\r\n",
        Some("legacy-source"),
    );
    run(&dir, &capture).unwrap();
    let before = files(&root);
    let r = reader(&dir);
    assert_eq!(
        r.load_source_artifact(&id(NS), &id("next-source")).unwrap(),
        Some(capture.source().clone())
    );
    assert_eq!(
        r.load_source_fragments(&id(NS), &id("next-source"))
            .unwrap(),
        Some(capture.fragments().to_vec())
    );
    assert!(
        r.load_source_artifact(&id(NS), &id("legacy-source"))
            .unwrap()
            .is_some()
    );
    assert!(
        r.load_source_artifact(&id(NS), &id("missing-source"))
            .unwrap()
            .is_none()
    );
    let versions = r
        .list_source_versions(&id(NS), &id("legacy-lineage"))
        .unwrap();
    assert_eq!(versions.len(), 2);
    let lineages = r
        .list_source_lineages(&SourceCatalogRequest::new(id(NS), 0, 20).unwrap())
        .unwrap();
    assert_eq!(lineages.len(), 1);
    assert!(
        r.resolve_source_lineage(&id(NS), &id("legacy-lineage"))
            .unwrap()
            .is_some()
    );
    assert_eq!(r.search(&query("needle")).unwrap().len(), 1);
    assert!(r.search(&query("Legacy")).unwrap().is_empty());
    assert!(r.search(&query("needle OR * - ()")).unwrap().is_empty());
    assert!(
        r.load_source_artifact(&id("other-namespace"), &id("next-source"))
            .is_err()
    );
    assert!(
        r.list_source_lineages(&SourceCatalogRequest::new(id("other-namespace"), 0, 20).unwrap())
            .is_err()
    );
    drop(r);
    assert_eq!(files(&root), before);
    assert_eq!(count(&root, "radishmemory_source_bodies"), 0);
}

#[test]
fn search_returns_confirmed_memory_from_authenticated_provenance_and_filters_before_top_k() {
    let (_root, dir) = setup_confirmed_memory();
    let r = reader(&dir);
    assert!(
        r.search(&query("preference"))
            .unwrap()
            .iter()
            .any(|h| matches!(h, LocalSearchHit::MemoryRecord(_)))
    );
    let denied = LocalSearchRequest::new(
        id(NS),
        text("preference"),
        at(),
        1,
        [Sensitivity::Restricted],
    )
    .unwrap();
    assert!(r.search(&denied).unwrap().is_empty());
    let past = LocalSearchRequest::new(
        id(NS),
        text("preference"),
        Timestamp::parse("2026-09-25T00:00:00Z").unwrap(),
        1,
        [Sensitivity::Personal],
    )
    .unwrap();
    assert!(r.search(&past).unwrap().is_empty());
}

#[test]
fn keys_and_database_preflight_fail_without_creating_or_repairing_data() {
    let (root, dir) = setup(true);
    let before = files(&root);
    assert!(
        open(&dir, NS, DEVICE, || Err(SourceVaultError::new(
            SourceVaultErrorCode::KeyMissing,
            "synthetic key absent"
        )))
        .is_err()
    );
    assert!(open(&dir, NS, DEVICE, || Ok(KeyEncryptionKey::new([0x11; 32]))).is_err());
    assert!(
        open(&dir, "other-namespace", DEVICE, || panic!(
            "key must not be loaded"
        ))
        .is_err()
    );
    assert!(
        open(&dir, NS, "other-device", || panic!(
            "key must not be loaded"
        ))
        .is_err()
    );
    assert_eq!(files(&root), before);
}

#[test]
fn corrupt_objects_and_derived_rows_fail_closed_without_read_repair() {
    for mutation in [
        "UPDATE radishmemory_recall_fts SET content='synthetic drift'",
        "DELETE FROM radishmemory_source_lineage_tips",
    ] {
        let (root, dir) = setup(true);
        sql(&root, mutation);
        let before = files(&root);
        assert!(open(&dir, NS, DEVICE, || Ok(key())).is_err());
        assert_eq!(files(&root), before);
    }
    let (root, dir) = setup(true);
    let path = object(&root);
    fs::write(&path, b"synthetic corrupt object").unwrap();
    assert!(open(&dir, NS, DEVICE, || Ok(key())).is_err());
    assert_eq!(fs::read(path).unwrap(), b"synthetic corrupt object");
}

#[test]
fn late_file_damage_is_rechecked_before_return_and_later_reads_do_not_use_a_cache() {
    let (root, dir) = setup(true);
    let r = reader(&dir);
    let path = object(&root);
    let bytes = fs::read(&path).unwrap();
    assert!(
        r.read_with_check(
            &id(NS),
            |v| v.load_source_artifact(&id(NS), &id("legacy-source")),
            || {
                fs::write(&path, b"synthetic late damage").unwrap();
                Ok(())
            }
        )
        .is_err()
    );
    assert!(
        r.load_source_artifact(&id(NS), &id("legacy-source"))
            .is_err()
    );
    fs::write(&path, bytes).unwrap();
    assert!(
        r.load_source_artifact(&id(NS), &id("legacy-source"))
            .unwrap()
            .is_some()
    );
}

#[test]
fn reader_keeps_an_exclusive_session_and_releases_it_on_drop() {
    let (root, dir) = setup(true);
    let r = reader(&dir);
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    c.busy_timeout(std::time::Duration::ZERO).unwrap();
    assert!(c.execute_batch("BEGIN IMMEDIATE").is_err());
    assert!(
        open(&dir, NS, DEVICE, || panic!(
            "locked library must not load key"
        ))
        .is_err()
    );
    drop(r);
    c.execute_batch("BEGIN IMMEDIATE; ROLLBACK;").unwrap();
    drop(c);
    reader(&dir)
        .load_source_artifact(&id(NS), &id("legacy-source"))
        .unwrap();
}

fn purge_request() -> DeleteRequest {
    let targets = vec![
        ObjectRef::new(CanonicalObjectType::SourceArtifact, id("legacy-source")),
        ObjectRef::new(CanonicalObjectType::MemoryRecord, id("memory-synthetic")),
    ];
    DeleteRequest::new(DeleteRequestParams {
        delete_request_id: id("delete-reader-synthetic"),
        namespace_id: id(NS),
        device_id: id(DEVICE),
        requested_by: ActorRef::new(ActorType::TestFixture, id("actor-synthetic"), None),
        authorization_basis: text("explicit-synthetic-purge"),
        requested_guarantee: RequestedGuarantee::LocalPurge,
        planned_components: build_local_purge_targets(&targets).unwrap(),
        target_refs: targets,
        reason_code: text("synthetic-purge"),
        requested_at: at(),
    })
    .unwrap()
}

#[test]
fn deletion_closed_sources_and_memories_never_reappear_in_reads_or_rebuilt_search() {
    let root = crate::capture::tests::setup_plaintext_confirmed_memory();
    let mut db = radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).unwrap();
    let req = purge_request();
    db.store_delete_request(&req).unwrap();
    drop(db);
    let dir = ObjectDirectory::open_application_directory(&root.0).unwrap();
    crate::bootstrap::initialize(&dir, NS, DEVICE, |_| Ok(key())).unwrap();
    crate::body_migration::migrate(&dir, NS, DEVICE, || Ok(key())).unwrap();
    let check = || {
        let r = reader(&dir);
        assert!(
            r.load_source_artifact(&id(NS), &id("legacy-source"))
                .unwrap()
                .is_none()
        );
        assert!(
            r.load_source_fragments(&id(NS), &id("legacy-source"))
                .unwrap()
                .is_none()
        );
        assert!(
            r.resolve_source_lineage(&id(NS), &id("legacy-lineage"))
                .unwrap()
                .is_none()
        );
        assert!(
            r.list_source_versions(&id(NS), &id("legacy-lineage"))
                .unwrap()
                .is_empty()
        );
        assert!(
            r.list_source_lineages(&SourceCatalogRequest::new(id(NS), 0, 20).unwrap())
                .unwrap()
                .is_empty()
        );
        assert!(r.search(&query("Synthetic")).unwrap().is_empty());
    };
    check();
    let execution = LocalDeletionExecution::new(
        at(),
        EvidenceRef::new(EvidenceType::PolicyBasis, id("policy-synthetic")),
    )
    .unwrap();
    crate::deletion::execute(&dir, &req, &execution, || Ok(key())).unwrap();
    check();
    crate::maintenance::maintain(&dir, NS, DEVICE, true, || Ok(key())).unwrap();
    check();
    assert_eq!(count(&root, "radishmemory_legacy_body_retirements"), 0);
    assert!(files(&root).is_empty());
}

#[test]
fn pagination_and_queries_match_plaintext_baseline_after_migration() {
    let root = TestDirectory::new();
    let mut db = radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).unwrap();
    for n in 0..201 {
        db.capture_source(&request(
            &format!("source-{n:03}"),
            &format!("lineage-{n:03}"),
            1,
            "Synthetic pagination needle",
            None,
        ))
        .unwrap();
    }
    let pages = [(0, 200), (200, 200), (201, 20), (u64::MAX, 1)];
    let expected = pages
        .iter()
        .map(|&(offset, limit)| {
            db.list_source_lineages(&SourceCatalogRequest::new(id(NS), offset, limit).unwrap())
                .unwrap()
        })
        .collect::<Vec<_>>();
    let expected_hits = db.search(&query("needle")).unwrap();
    drop(db);
    let dir = ObjectDirectory::open_application_directory(&root.0).unwrap();
    crate::bootstrap::initialize(&dir, NS, DEVICE, |_| Ok(key())).unwrap();
    crate::body_migration::migrate(&dir, NS, DEVICE, || Ok(key())).unwrap();
    let r = reader(&dir);
    for ((offset, limit), expected) in pages.into_iter().zip(expected) {
        assert_eq!(
            r.list_source_lineages(&SourceCatalogRequest::new(id(NS), offset, limit).unwrap())
                .unwrap(),
            expected
        );
    }
    assert_eq!(r.search(&query("needle")).unwrap(), expected_hits);
}

#[test]
fn search_applies_sensitivity_retention_and_time_before_ranking() {
    let (root, dir) = setup(false);
    for (source, sensitivity, expires) in [
        ("a-restricted", Sensitivity::Restricted, None),
        (
            "b-expired",
            Sensitivity::Personal,
            Some("2026-09-26T01:00:00Z"),
        ),
        ("z-allowed", Sensitivity::Personal, None),
    ] {
        let capture = request(source, source, 1, "Synthetic filter needle", None);
        let governance = Governance::new(
            sensitivity,
            EgressPolicy::LocalOnly,
            RetentionRule::new(
                if expires.is_some() {
                    RetentionMode::UntilTime
                } else {
                    RetentionMode::UntilDeleted
                },
                expires.map(|s| Timestamp::parse(s).unwrap()),
                None,
            )
            .unwrap(),
            DeletionState::Active,
            id("policy-synthetic"),
        )
        .unwrap();
        let mut source_params = capture.source().params().clone();
        source_params.governance = governance.clone();
        let mut fragment_params = capture.fragments()[0].params().clone();
        fragment_params.governance = governance;
        let capture = SourceCapture::new(
            capture.origin_binding_id().clone(),
            SourceArtifact::new(source_params).unwrap(),
            vec![SourceFragment::new(fragment_params).unwrap()],
        )
        .unwrap();
        run(&dir, &capture).unwrap();
    }
    let r = reader(&dir);
    let q =
        LocalSearchRequest::new(id(NS), text("needle"), at(), 1, [Sensitivity::Personal]).unwrap();
    let hits = r.search(&q).unwrap();
    assert_eq!(hits.len(), 1);
    let LocalSearchHit::SourceFragment(fragment) = &hits[0] else {
        panic!("expected a source")
    };
    assert_eq!(fragment.params().source_id, id("z-allowed"));
    drop(r);
    assert_eq!(count(&root, "radishmemory_recall_fts"), 3);
}

#[test]
fn unknown_files_missing_objects_and_replaced_database_block_result_release() {
    let damages = if cfg!(unix) {
        vec!["missing", "unknown", "database"]
    } else {
        vec!["missing", "unknown"]
    };
    for damage in damages {
        let (root, dir) = setup(true);
        let path = object(&root);
        let r = reader(&dir);
        match damage {
            "missing" => fs::rename(&path, root.0.join("held-object")).unwrap(),
            "unknown" => fs::write(
                root.0.join("source-objects-v1/unknown.rmo"),
                b"synthetic unknown bytes",
            )
            .unwrap(),
            "database" => {
                let db = root.0.join("library.sqlite3");
                fs::rename(&db, root.0.join("held-database")).unwrap();
                fs::copy(root.0.join("held-database"), &db).unwrap();
            }
            _ => unreachable!(),
        }
        let error = r
            .load_source_artifact(&id(NS), &id("legacy-source"))
            .unwrap_err();
        let diagnostic = format!("{r:?} {error:?} {error}");
        for secret in [
            NS,
            "legacy-source",
            "Legacy synthetic bytes",
            root.0.to_str().unwrap(),
        ] {
            assert!(!diagnostic.contains(secret));
        }
    }
}

#[test]
fn independent_process_reopens_the_library_after_reader_releases_its_lock() {
    let (root, dir) = setup(true);
    let check = |locked| {
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "reader::tests::reader_process_helper",
                "--ignored",
            ])
            .env("RADISHMEMORY_READER_SYNTHETIC_ROOT", &root.0)
            .env(
                "RADISHMEMORY_READER_EXPECT_LOCK",
                if locked { "yes" } else { "no" },
            )
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
    };
    let r = reader(&dir);
    check(true);
    assert!(
        open(&dir, NS, DEVICE, || panic!(
            "competing open must not load key"
        ))
        .is_err()
    );
    check(true);
    r.load_source_artifact(&id(NS), &id("legacy-source"))
        .unwrap();
    check(true);
    drop(r);
    check(false);
}

#[test]
#[ignore = "invoked only by parent with an isolated synthetic library"]
fn reader_process_helper() {
    let Some(root) = std::env::var_os("RADISHMEMORY_READER_SYNTHETIC_ROOT") else {
        return;
    };
    let locked = std::env::var("RADISHMEMORY_READER_EXPECT_LOCK").unwrap() == "yes";
    let dir = ObjectDirectory::open_application_directory(std::path::PathBuf::from(root)).unwrap();
    let result = open(&dir, NS, DEVICE, || {
        assert!(!locked, "locked library loaded a key");
        Ok(key())
    });
    if locked {
        assert!(result.is_err());
    } else {
        let r = result.unwrap();
        let source = r
            .load_source_artifact(&id(NS), &id("legacy-source"))
            .unwrap()
            .unwrap();
        assert_eq!(source.params().content.as_str(), "Legacy synthetic bytes");
        assert_eq!(r.search(&query("Legacy")).unwrap().len(), 1);
    }
}

#[test]
fn pending_and_abandoned_capture_never_become_readable_sources() {
    let (root, dir) = setup(true);
    let pending = request(
        "pending-source",
        "pending-lineage",
        1,
        "Synthetic pending bytes",
        None,
    );
    crate::capture::tests::interrupt(&dir, &pending, crate::capture::Step::Published);
    let before = files(&root);
    let r = reader(&dir);
    assert!(
        r.load_source_artifact(&id(NS), &id("pending-source"))
            .unwrap()
            .is_none()
    );
    assert!(r.search(&query("pending")).unwrap().is_empty());
    assert_eq!(
        r.list_source_lineages(&SourceCatalogRequest::new(id(NS), 0, 20).unwrap())
            .unwrap()
            .len(),
        1
    );
    drop(r);
    assert_eq!(files(&root), before);
    let target = crate::abandonment::inspect(&dir, NS, DEVICE, || Ok(key()))
        .unwrap()
        .unwrap();
    crate::abandonment::abandon(&dir, NS, DEVICE, &target, || Ok(key())).unwrap();
    let r = reader(&dir);
    assert!(
        r.load_source_artifact(&id(NS), &id("pending-source"))
            .unwrap()
            .is_none()
    );
    assert!(r.search(&query("pending")).unwrap().is_empty());
}

#[test]
fn missing_database_and_unfinished_migration_do_not_load_keys_or_open_read_sessions() {
    let root = TestDirectory::new();
    let dir = ObjectDirectory::open_application_directory(&root.0).unwrap();
    assert!(open(&dir, NS, DEVICE, || panic!("no database must not load key")).is_err());
    assert!(!root.0.join("library.sqlite3").exists());
    crate::bootstrap::initialize(&dir, NS, DEVICE, |_| Ok(key())).unwrap();
    assert!(
        open(&dir, NS, DEVICE, || panic!(
            "incomplete migration must not load key"
        ))
        .is_err()
    );
    assert_eq!(count(&root, "radishmemory_source_vault_key_profile"), 1);
}

#[test]
#[ignore = "invoked by coordinator tests against an isolated locked database"]
fn database_writer_probe() {
    let Some(root) = std::env::var_os("RADISHMEMORY_WRITER_PROBE_ROOT") else {
        return;
    };
    let connection =
        Connection::open(std::path::PathBuf::from(root).join("library.sqlite3")).unwrap();
    connection.busy_timeout(std::time::Duration::ZERO).unwrap();
    let error = connection
        .execute_batch("BEGIN IMMEDIATE")
        .expect_err("coordinator lost writer exclusion");
    assert!(matches!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    ));
}

#[test]
fn database_size_is_not_limited_by_the_single_object_envelope_bound() {
    let root = TestDirectory::new();
    let mut db = radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).unwrap();
    db.capture_source(&request(
        "legacy-source",
        "legacy-lineage",
        1,
        "Synthetic retained source",
        None,
    ))
    .unwrap();
    drop(db);
    // Freed pages are a normal result of earlier deletions. No extra table or
    // changed canonical fact remains when the coordinator verifies the schema.
    sql(
        &root,
        "CREATE TABLE synthetic_padding(bytes BLOB); INSERT INTO synthetic_padding VALUES(zeroblob(9437184)); DROP TABLE synthetic_padding;",
    );
    assert!(
        fs::metadata(root.0.join("library.sqlite3")).unwrap().len()
            > crate::envelope::MAX_ENVELOPE_BYTES as u64
    );
    let dir = ObjectDirectory::open_application_directory(&root.0).unwrap();
    crate::bootstrap::initialize(&dir, NS, DEVICE, |_| Ok(key())).unwrap();
    crate::body_migration::migrate(&dir, NS, DEVICE, || Ok(key())).unwrap();
    assert_eq!(
        reader(&dir)
            .load_source_artifact(&id(NS), &id("legacy-source"))
            .unwrap()
            .unwrap()
            .params()
            .content
            .as_str(),
        "Synthetic retained source"
    );
}
