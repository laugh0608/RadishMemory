use super::*;
use crate::capture::tests::setup_plaintext_confirmed_memory;

fn plaintext(failure: Option<&str>) -> (TestDirectory, DeleteRequest) {
    let root = setup_plaintext_confirmed_memory();
    let req = request(&["legacy-source"], &["memory-synthetic"]);
    let mut db = radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).unwrap();
    db.store_delete_request(&req).unwrap();
    if let Some(target) = failure {
        // Install after open and remove before migration: exercise real component
        // rollback/persisted failure, without teaching production a test schema.
        sql(
            &root,
            &format!(
                "CREATE TRIGGER synthetic_failure BEFORE {target} BEGIN SELECT RAISE(ABORT,'synthetic legacy component failure'); END"
            ),
        );
        let results = db
            .execute_deletion(
                &req.params().namespace_id,
                &req.params().delete_request_id,
                &execution(),
            )
            .unwrap();
        assert!(
            results
                .iter()
                .any(|r| r.params().status == ComponentStatus::Failed)
        );
        sql(&root, "DROP TRIGGER synthetic_failure");
    }
    drop(db);
    (root, req)
}
fn migrate(root: &TestDirectory) -> ObjectDirectory {
    let dir = ObjectDirectory::open_application_directory(&root.0).unwrap();
    crate::bootstrap::initialize(&dir, NS, DEVICE, |_| Ok(key())).unwrap();
    crate::body_migration::migrate(&dir, NS, DEVICE, || Ok(key())).unwrap();
    dir
}
fn assert_unchanged_failure(root: &TestDirectory, dir: &ObjectDirectory, req: &DeleteRequest) {
    let before = files(root);
    let results = count(root, "radishmemory_deletion_execution_results");
    assert!(delete(dir, req).is_err());
    assert_eq!(before, files(root));
    assert_eq!(
        results,
        count(root, "radishmemory_deletion_execution_results")
    );
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        8
    );
}

#[test]
fn legacy_pending_and_real_partial_results_resume_the_same_ten_components() {
    for failure in [
        None,
        Some("DELETE ON radishmemory_source_bodies"),
        Some("UPDATE OF title ON radishmemory_source_artifacts"),
        Some("DELETE ON radishmemory_source_fragments"),
    ] {
        let (root, req) = plaintext(failure);
        let prior_body = count(&root, "radishmemory_source_bodies");
        let dir = migrate(&root);
        assert_eq!(files(&root).len(), prior_body as usize);
        let results = delete(&dir, &req).unwrap();
        assert_eq!(results.len(), 10);
        assert!(
            results
                .iter()
                .all(|r| r.params().status == ComponentStatus::Succeeded)
        );
        assert_eq!(count(&root, "radishmemory_delete_requests"), 1);
        assert_eq!(
            count(&root, "radishmemory_legacy_body_retirements"),
            1 - prior_body
        );
        assert!(files(&root).is_empty());
        let body = results
            .iter()
            .find(|r| r.params().component_type == DeletionComponentType::SourceBody)
            .unwrap();
        assert_eq!(
            body.params().verification_method.as_str(),
            if prior_body == 0 {
                "source-vault-and-legacy-body-absence-v1"
            } else {
                "source-vault-authenticated-absence-v1"
            }
        );
        verify(&dir, true);
        assert_eq!(count(&root, "radishmemory_recall_fts"), 0);
        assert!(
            delete(&dir, &req)
                .unwrap()
                .iter()
                .all(|r| r.params().status == ComponentStatus::Succeeded)
        );
        // v12 also supports later ordinary encrypted capture/deletion and does
        // not downgrade the maintenance session to v9 or v11.
        run(
            &dir,
            &capture_request(
                "later-source",
                "later-lineage",
                1,
                "Synthetic later body",
                None,
            ),
        )
        .unwrap();
        let mut params = request(&["later-source"], &[]).params().clone();
        params.delete_request_id = id("later-delete");
        delete(&dir, &DeleteRequest::new(params).unwrap()).unwrap();
        verify(&dir, true);
        assert_eq!(count(&root, "radishmemory_recall_fts"), 0);
    }
}

#[test]
fn legacy_absence_requires_real_valid_original_component_evidence() {
    for mutation in [
        "DELETE FROM radishmemory_deletion_execution_results WHERE component_key='source-body'",
        "UPDATE radishmemory_deletion_execution_results SET processed_count=0 WHERE component_key='source-body'",
        "UPDATE radishmemory_deletion_execution_results SET verification_method='invented-proof' WHERE component_key='source-body'",
        "UPDATE radishmemory_deletion_execution_results SET checked_at='2026-09-25T00:00:00Z' WHERE component_key='source-body'",
        "UPDATE radishmemory_deletion_execution_results SET status='failed',outcome='not_applicable',error_code='synthetic',retryable=1 WHERE component_key='source-body'",
    ] {
        let (root, req) = plaintext(Some("UPDATE OF title ON radishmemory_source_artifacts"));
        let dir = migrate(&root);
        sql(&root, mutation);
        assert_unchanged_failure(&root, &dir, &req);
    }
    let (root, req) = plaintext(None);
    sql(&root, "DELETE FROM radishmemory_source_bodies");
    let dir = migrate(&root);
    assert_unchanged_failure(&root, &dir, &req);
}

#[test]
fn legacy_closure_drift_is_rejected_before_any_object_is_retired() {
    for mutation in [
        "UPDATE radishmemory_memory_decisions SET namespace_id='unrelated-namespace'",
        "UPDATE radishmemory_memory_state_events SET namespace_id='unrelated-namespace'",
        "UPDATE radishmemory_delete_execution_closure SET object_id='unrelated-source' WHERE component_type='source_body'",
        "DELETE FROM radishmemory_delete_execution_closure WHERE component_type='source_fragment'",
        "UPDATE radishmemory_delete_execution_closure SET object_id='missing-fragment' WHERE component_type IN ('source_fragment','full_text_index') AND object_type='SourceFragment'",
        "DELETE FROM radishmemory_delete_execution_closure WHERE component_type='memory_proposal'",
        "DELETE FROM radishmemory_delete_execution_closure WHERE component_type='memory_decision'",
        "DELETE FROM radishmemory_delete_execution_closure WHERE component_type='memory_state_event'",
        "DELETE FROM radishmemory_delete_execution_closure WHERE component_type='full_text_index'",
        "UPDATE radishmemory_delete_execution_closure SET object_id='unrelated-request' WHERE component_type='minimal_audit' AND object_type='DeleteRequest'",
    ] {
        let (root, req) = plaintext(None);
        let dir = migrate(&root);
        sql(&root, mutation);
        assert_unchanged_failure(&root, &dir, &req);
    }
}

#[test]
fn legacy_receipt_drift_and_reappeared_blob_block_readback_and_rebuild() {
    for mutation in [
        "UPDATE radishmemory_deletion_execution_results SET verification_method='invented-proof' WHERE component_key='source-body' AND attempt_ordinal=1",
        "INSERT INTO radishmemory_source_bodies VALUES('legacy-source',X'73796e746865746963')",
        "DELETE FROM radishmemory_legacy_body_retirements",
    ] {
        let (root, req) = plaintext(Some("UPDATE OF title ON radishmemory_source_artifacts"));
        let dir = migrate(&root);
        delete(&dir, &req).unwrap();
        sql(&root, mutation);
        assert!(crate::maintenance::maintain(&dir, NS, DEVICE, false, || Ok(key())).is_err());
        assert!(crate::maintenance::maintain(&dir, NS, DEVICE, true, || Ok(key())).is_err());
        assert!(delete(&dir, &req).is_err());
    }
}

#[test]
fn legacy_partial_attempt_does_not_need_unexecuted_component_rows() {
    let (root, req) = plaintext(Some("UPDATE OF title ON radishmemory_source_artifacts"));
    // A process can stop after the body result commits, before later components.
    sql(
        &root,
        "DELETE FROM radishmemory_deletion_execution_results WHERE component_key NOT IN ('memory-proposal','memory-record','source-fragment','source-body')",
    );
    let dir = migrate(&root);
    assert_eq!(delete(&dir, &req).unwrap().len(), 10);
    verify(&dir, true);
}

#[test]
fn legacy_adoption_recovers_after_process_exit_with_and_without_residual_objects() {
    for (already_deleted, stop) in [
        (false, "intent"),
        (false, "unlinked"),
        (false, "retired"),
        (true, "intent"),
        (true, "executed"),
    ] {
        let root = TestDirectory::new();
        let mut db =
            radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).unwrap();
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
        if already_deleted {
            db.execute_deletion(&id(NS), &req.params().delete_request_id, &execution())
                .unwrap();
        }
        drop(db);
        let dir = migrate(&root);
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
        assert_eq!(output.status.code(), Some(74), "{output:?}");
        verify(&dir, true);
        assert!(
            delete(&dir, &req)
                .unwrap()
                .iter()
                .all(|r| r.params().status == ComponentStatus::Succeeded)
        );
        assert!(files(&root).is_empty());
        assert_eq!(count(&root, "radishmemory_delete_requests"), 1);
        assert_eq!(count(&root, "radishmemory_recall_fts"), 0);
    }
}

fn completed_evidence(
    req: &DeleteRequest,
    results: Vec<ComponentResult>,
    name: &str,
    previous: Option<Identifier>,
) -> DeletionEvidence {
    let evidence_id = id(name);
    let digest = compute_deletion_evidence_digest(
        &evidence_id,
        &req.params().delete_request_id,
        DeletionOverallStatus::Completed,
        &results,
    )
    .unwrap();
    DeletionEvidence::new(DeletionEvidenceParams {
        deletion_evidence_id: evidence_id,
        delete_request_id: req.params().delete_request_id.clone(),
        previous_evidence_id: previous,
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
    .unwrap()
}

#[test]
fn legacy_evidence_chain_is_preserved_and_absence_is_not_a_new_object_deletion() {
    let (root, req) = plaintext(None);
    let mut db = radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).unwrap();
    let old_results = db
        .execute_deletion(&id(NS), &req.params().delete_request_id, &execution())
        .unwrap();
    let old = completed_evidence(&req, old_results, "old-evidence", None);
    db.store_deletion_evidence(&old).unwrap();
    drop(db);
    let dir = migrate(&root);
    let results = delete(&dir, &req).unwrap();
    assert_eq!(
        results
            .iter()
            .find(|r| r.params().component_type == DeletionComponentType::SourceBody)
            .unwrap()
            .params()
            .outcome,
        ComponentOutcome::NotFound
    );
    let new = completed_evidence(&req, results, "new-evidence", Some(id("old-evidence")));
    store_evidence(&dir, &new, || Ok(key())).unwrap();
    let db = EncryptedCaptureDatabase::open(
        root.0.join("library.sqlite3"),
        NS,
        DEVICE,
        PROVIDER_PROFILE,
    )
    .unwrap();
    assert_eq!(
        db.load_object_deletion_evidence(&id("old-evidence"))
            .unwrap(),
        Some(old)
    );
    assert_eq!(
        db.load_object_deletion_evidence(&id("new-evidence"))
            .unwrap(),
        Some(new)
    );
    assert!(db.objects().unwrap().is_empty());
}

#[test]
fn legacy_deleted_fragments_and_redacted_proposal_require_their_original_results() {
    for mutation in [
        None,
        Some(
            "DELETE FROM radishmemory_deletion_execution_results WHERE component_key='source-fragment'",
        ),
        Some(
            "DELETE FROM radishmemory_deletion_execution_results WHERE component_key='memory-proposal'",
        ),
        Some("UPDATE radishmemory_memory_proposals SET content_text='Unrelated synthetic content'"),
    ] {
        let root = crate::capture::tests::setup_plaintext_memory(false);
        let req = request(&["legacy-source"], &[]);
        let mut db =
            radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).unwrap();
        db.store_delete_request(&req).unwrap();
        db.execute_deletion(&id(NS), &req.params().delete_request_id, &execution())
            .unwrap();
        drop(db);
        let dir = migrate(&root);
        if let Some(mutation) = mutation {
            sql(&root, mutation);
            assert_unchanged_failure(&root, &dir, &req);
        } else {
            assert_eq!(delete(&dir, &req).unwrap().len(), 10);
            verify(&dir, true);
        }
    }
}
