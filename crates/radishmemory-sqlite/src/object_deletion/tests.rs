use super::*;
use radishmemory_core::{
    ActorRef, ActorType, ComponentStatus, DeleteRequestParams, DeletionComponentType, EvidenceRef,
    EvidenceType, ObjectRef, RequestedGuarantee, build_local_purge_targets,
};

fn setup() -> (EncryptedCaptureDatabase, SourceCapture, DeleteRequest) {
    let mut connection = Connection::open_in_memory().unwrap();
    crate::configure_connection(&connection).unwrap();
    let tx = connection.transaction().unwrap();
    migration::apply_pending(&tx, 0, 9).unwrap();
    tx.execute("INSERT INTO radishmemory_source_vault_key_profile VALUES(1,'namespace-commit-1','device-synthetic','provider-synthetic','key_ready')",[]).unwrap();
    tx.execute(
        "INSERT INTO radishmemory_source_vault_migration VALUES(1,'objects_ready')",
        [],
    )
    .unwrap();
    tx.commit().unwrap();
    let mut db = EncryptedCaptureDatabase {
        connection,
        namespace: "namespace-commit-1".into(),
        version: 9,
    };
    let req = crate::source_capture::tests::capture(
        "source-commit-1",
        "fragment-commit-1",
        1,
        "Synthetic deletion body",
        "2026-09-26T00:00:00Z",
    );
    db.prepare_capture(&req, &"a".repeat(64), &"b".repeat(64))
        .unwrap();
    db.commit_capture(&req, &[]).unwrap();
    let target = vec![ObjectRef::new(
        CanonicalObjectType::SourceArtifact,
        req.source().params().source_id.clone(),
    )];
    let request = DeleteRequest::new(DeleteRequestParams {
        delete_request_id: source_store::identifier("delete-synthetic".into()).unwrap(),
        namespace_id: req.source().params().namespace_id.clone(),
        requested_by: ActorRef::new(
            ActorType::TestFixture,
            source_store::identifier("actor-synthetic".into()).unwrap(),
            None,
        ),
        authorization_basis: source_store::non_empty_text(
            "Synthetic explicit authorization".into(),
        )
        .unwrap(),
        requested_guarantee: RequestedGuarantee::LocalPurge,
        device_id: source_store::identifier("device-synthetic".into()).unwrap(),
        planned_components: build_local_purge_targets(&target).unwrap(),
        target_refs: target,
        reason_code: source_store::non_empty_text("synthetic-purge".into()).unwrap(),
        requested_at: req.source().params().created_at.clone(),
    })
    .unwrap();
    (db, req, request)
}
fn fail_commit(c: &Connection) -> Result<()> {
    c.execute_batch("PRAGMA defer_foreign_keys=ON; INSERT INTO radishmemory_fragment_heading_path VALUES('missing-fragment',0,'Synthetic heading')").map_err(SqliteError::storage)
}

#[test]
fn real_commit_failures_preserve_reference_and_retirement_checkpoints() {
    let (mut db, capture, request) = setup();
    let sources = vec![capture.source().clone()];
    let err = db
        .begin_with_check(&request, &sources, fail_commit)
        .unwrap_err();
    assert_eq!(
        err.sqlite_extended_code(),
        Some(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY)
    );
    assert_eq!(db.version, 9);
    assert_eq!(
        db.connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        9
    );
    db.reference("source-commit-1").unwrap();
    db.verify_facts(&sources).unwrap();
    db.begin_object_deletion(&request, &sources).unwrap();
    assert!(db.reference("source-commit-1").is_err());
    let err = db
        .finish_with_check(&request, "source-commit-1", fail_commit)
        .unwrap_err();
    assert_eq!(
        err.sqlite_extended_code(),
        Some(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY)
    );
    assert_eq!(
        db.deletion_objects(&request).unwrap()[0].state,
        CaptureObjectState::Deleting
    );
    db.finish_object_retirement(&request, "source-commit-1")
        .unwrap();
    assert_eq!(
        db.deletion_objects(&request).unwrap()[0].state,
        CaptureObjectState::Deleted
    );
}

#[test]
fn actual_component_failure_is_reported_and_retry_does_not_reopen_recall() {
    let (mut db, capture, request) = setup();
    let execution = LocalDeletionExecution::new(
        capture.source().params().created_at.clone(),
        EvidenceRef::new(
            EvidenceType::PolicyBasis,
            source_store::identifier("policy-synthetic".into()).unwrap(),
        ),
    )
    .unwrap();
    db.begin_object_deletion(&request, &[capture.source().clone()])
        .unwrap();
    assert!(db.execute_object_deletion(&request, &execution).is_err());
    db.finish_object_retirement(&request, "source-commit-1")
        .unwrap();
    db.connection.execute_batch("CREATE TEMP TRIGGER fail_metadata BEFORE UPDATE OF title ON radishmemory_source_artifacts BEGIN SELECT RAISE(ABORT,'synthetic component failure'); END").unwrap();
    let results = db.execute_object_deletion(&request, &execution).unwrap();
    assert_eq!(
        results
            .iter()
            .find(|r| r.params().component_type == DeletionComponentType::SourceMetadata)
            .unwrap()
            .params()
            .status,
        ComponentStatus::Failed
    );
    assert_eq!(
        db.connection
            .query_row(
                "SELECT deletion_state FROM radishmemory_source_artifacts",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "failed"
    );
    db.verify_facts(&[]).unwrap();
    db.connection
        .execute_batch("DROP TRIGGER fail_metadata")
        .unwrap();
    assert!(
        db.execute_object_deletion(&request, &execution)
            .unwrap()
            .iter()
            .all(|r| r.params().status == ComponentStatus::Succeeded)
    );
    db.verify_facts(&[]).unwrap();
}
