use super::*;
use crate::test_support::{TestDirectory, key};
use radishmemory_core::{
    DeletionState, EgressPolicy, Governance, Identifier, MediaType, NonEmptyText, ProducerRef,
    ProducerType, RetentionMode, RetentionRule, Sensitivity, SourceArtifactParams,
    SourceCaptureStore, SourceFragment, SourceFragmentParams, SourceKind, SourceOriginKind,
    Timestamp, Version, compute_exact_bytes_digest,
};
use rusqlite::Connection;
use std::{fs, process::Command};

const NS: &str = "namespace-0123456789abcdef0123456789abcdef";
const DEVICE: &str = "device-fedcba9876543210fedcba9876543210";
const BODY: &str = "合成 capture 原文\r\n# exact bytes\n";
fn id(s: &str) -> Identifier {
    Identifier::new(s).unwrap()
}
fn text(s: &str) -> NonEmptyText {
    NonEmptyText::new(s).unwrap()
}
fn request(
    source: &str,
    lineage: &str,
    version: u64,
    body: &str,
    previous: Option<&str>,
) -> SourceCapture {
    let governance = Governance::new(
        Sensitivity::Personal,
        EgressPolicy::LocalOnly,
        RetentionRule::new(RetentionMode::UntilDeleted, None, None).unwrap(),
        DeletionState::Active,
        id("policy-synthetic"),
    )
    .unwrap();
    let producer = ProducerRef::new(
        ProducerType::TestFixture,
        id("producer-synthetic"),
        text("1"),
    );
    let timestamp = Timestamp::parse("2026-09-26T00:00:00Z").unwrap();
    let binding = id(&format!("origin-binding-{lineage}"));
    let artifact = SourceArtifact::new(SourceArtifactParams {
        source_id: id(source),
        lineage_id: id(lineage),
        version: Version::new(version).unwrap(),
        namespace_id: id(NS),
        source_kind: SourceKind::Markdown,
        media_type: MediaType::TextMarkdown,
        content: text(body),
        content_length: body.len() as u64,
        content_digest: compute_exact_bytes_digest(body.as_bytes()),
        title: Some(text("Synthetic title")),
        origin_kind: SourceOriginKind::ExplicitUserInput,
        origin_ref: Some(text(binding.as_str())),
        observed_at: timestamp.clone(),
        captured_at: timestamp.clone(),
        supersedes_source_ids: previous.into_iter().map(id).collect(),
        governance: governance.clone(),
        producer: producer.clone(),
        created_at: timestamp.clone(),
    })
    .unwrap();
    let fragment = SourceFragment::new(SourceFragmentParams {
        fragment_id: id(&format!("fragment-{source}")),
        namespace_id: id(NS),
        source_id: id(source),
        ordinal: 0,
        byte_start: 0,
        byte_end: body.len() as u64,
        heading_path: Some(vec![text("Synthetic heading")]),
        content: text(body),
        content_digest: compute_exact_bytes_digest(body.as_bytes()),
        segmenter: producer,
        governance,
        created_at: timestamp,
    })
    .unwrap();
    SourceCapture::new(binding, artifact, vec![fragment]).unwrap()
}
fn setup(legacy: bool) -> (TestDirectory, ObjectDirectory) {
    let root = TestDirectory::new();
    if legacy {
        let mut db =
            radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).unwrap();
        db.capture_source(&request(
            "legacy-source",
            "legacy-lineage",
            1,
            "Legacy synthetic bytes",
            None,
        ))
        .unwrap();
    }
    let directory = ObjectDirectory::open_application_directory(&root.0).unwrap();
    crate::bootstrap::initialize(&directory, NS, DEVICE, |_| Ok(key())).unwrap();
    crate::body_migration::migrate(&directory, NS, DEVICE, || Ok(key())).unwrap();
    (root, directory)
}
fn run(dir: &ObjectDirectory, req: &SourceCapture) -> Result<SourceCaptureResult> {
    capture(dir, NS, DEVICE, req, || Ok(key()))
}
fn count(root: &TestDirectory, table: &str) -> i64 {
    Connection::open(root.0.join("library.sqlite3"))
        .unwrap()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
fn files(root: &TestDirectory) -> Vec<(std::ffi::OsString, Vec<u8>)> {
    let mut files: Vec<_> = fs::read_dir(root.0.join("source-objects-v1"))
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (e.file_name(), fs::read(e.path()).unwrap())
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}
fn interrupt(dir: &ObjectDirectory, req: &SourceCapture, point: Step) {
    assert!(
        capture_with_step(
            dir,
            NS,
            DEVICE,
            req,
            || Ok(key()),
            |s| if s == point { Err(invalid()) } else { Ok(()) }
        )
        .is_err()
    );
}

#[test]
fn migrated_confirmed_memory_keeps_object_backed_provenance_during_new_capture() {
    use radishmemory_core::{
        ActorRef, ActorType, Decision, EvidenceRef, EvidenceType, MemoryDecision,
        MemoryDecisionParams, MemoryEventType, MemoryProposal, MemoryProposalParams, MemoryRecord,
        MemoryRecordParams, MemoryState, MemoryStateEvent, MemoryStateEventParams, MemoryStore,
        MemoryType, MemoryValue, ProposalOperation, TimePrecision, UnitInterval, ValidTime,
        ValidTimeMode,
    };
    let root = TestDirectory::new();
    let legacy = request("legacy-source", "legacy-lineage", 1, BODY, None);
    let p = legacy.source().params();
    let actor = ActorRef::new(ActorType::TestFixture, id("actor-synthetic"), None);
    let proposal = MemoryProposal::new(MemoryProposalParams {
        proposal_id: id("proposal-synthetic"),
        namespace_id: id(NS),
        operation: ProposalOperation::Create,
        memory_type: MemoryType::Preference,
        subject_ref: id("subject-synthetic"),
        proposed_content: MemoryValue::from_text(text("Synthetic confirmed preference")),
        source_fragment_refs: vec![legacy.fragments()[0].params().fragment_id.clone()],
        target_memory_ids: vec![],
        observed_at: p.observed_at.clone(),
        valid_time: ValidTime::new(
            ValidTimeMode::OpenEnded,
            Some(p.observed_at.clone()),
            None,
            TimePrecision::Exact,
        )
        .unwrap(),
        confidence: UnitInterval::new(0.9).unwrap(),
        importance: UnitInterval::new(0.7).unwrap(),
        governance: p.governance.clone(),
        producer: p.producer.clone(),
        reason_code: text("synthetic-proposal"),
        proposed_at: p.created_at.clone(),
    })
    .unwrap();
    let decision = MemoryDecision::new(MemoryDecisionParams {
        decision_id: id("decision-synthetic"),
        namespace_id: id(NS),
        proposal_id: proposal.params().proposal_id.clone(),
        previous_decision_id: None,
        decision: Decision::Accept,
        decided_by: actor.clone(),
        authorization_basis: text("explicit-synthetic-acceptance"),
        reason_code: text("synthetic-accept"),
        reason_text: None,
        result_memory_id: Some(id("memory-synthetic")),
        decided_at: p.created_at.clone(),
    })
    .unwrap();
    let value = proposal.params();
    let memory = MemoryRecord::new(MemoryRecordParams {
        memory_id: id("memory-synthetic"),
        lineage_id: id("memory-lineage-synthetic"),
        version: Version::new(1).unwrap(),
        namespace_id: id(NS),
        memory_type: value.memory_type,
        subject_ref: value.subject_ref.clone(),
        content: value.proposed_content.clone(),
        source_fragment_refs: value.source_fragment_refs.clone(),
        origin_proposal_id: value.proposal_id.clone(),
        accepted_by_decision_id: decision.params().decision_id.clone(),
        observed_at: value.observed_at.clone(),
        valid_time: value.valid_time.clone(),
        confidence: value.confidence,
        importance: value.importance,
        governance: value.governance.clone(),
        current_state: MemoryState::Confirmed,
        last_state_event_id: id("event-synthetic"),
        supersedes_memory_ids: vec![],
        contradicts_memory_ids: vec![],
        content_digest: value.proposed_content.content_digest().clone(),
        created_at: p.created_at.clone(),
    })
    .unwrap();
    let event = MemoryStateEvent::new(MemoryStateEventParams {
        event_id: id("event-synthetic"),
        namespace_id: id(NS),
        memory_id: memory.params().memory_id.clone(),
        previous_event_id: None,
        event_type: MemoryEventType::Confirmed,
        from_state: None,
        cause_ref: EvidenceRef::new(
            EvidenceType::MemoryDecision,
            decision.params().decision_id.clone(),
        ),
        related_memory_ids: vec![],
        actor,
        reason_code: text("synthetic-confirmed"),
        effective_at: None,
        occurred_at: p.created_at.clone(),
    })
    .unwrap();
    let mut db = radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).unwrap();
    db.capture_source(&legacy).unwrap();
    db.store_memory_proposal(&proposal).unwrap();
    db.store_memory_decision(&decision).unwrap();
    db.materialize_accepted_memory(&memory, &event, &[])
        .unwrap();
    drop(db);
    let dir = ObjectDirectory::open_application_directory(&root.0).unwrap();
    crate::bootstrap::initialize(&dir, NS, DEVICE, |_| Ok(key())).unwrap();
    crate::body_migration::migrate(&dir, NS, DEVICE, || Ok(key())).unwrap();
    let next = request("new-source", "new-lineage", 1, "Synthetic new bytes", None);
    assert_eq!(
        run(&dir, &next).unwrap().outcome(),
        SourceCaptureOutcome::Created
    );
    assert_eq!(
        run(&dir, &next).unwrap().outcome(),
        SourceCaptureOutcome::Idempotent
    );
    assert_eq!(count(&root, "radishmemory_memory_records"), 1);
    assert_eq!(count(&root, "radishmemory_recall_fts"), 3);
    assert_eq!(count(&root, "radishmemory_source_bodies"), 0);
    let raw = Connection::open(root.0.join("library.sqlite3")).unwrap();
    raw.execute(
        "UPDATE radishmemory_memory_proposals SET content_text='Tampered synthetic proposal'",
        [],
    )
    .unwrap();
    drop(raw);
    assert!(run(&dir, &next).is_err());
}

#[test]
fn capture_commits_no_inline_body_and_reopens_idempotently() {
    let (root, dir) = setup(false);
    let req = request("source-a", "lineage-a", 1, BODY, None);
    let result = run(&dir, &req).unwrap();
    assert_eq!(result.outcome(), SourceCaptureOutcome::Created);
    for table in [
        "radishmemory_source_artifacts",
        "radishmemory_source_fragments",
        "radishmemory_source_origin_bindings",
        "radishmemory_source_lineage_tips",
        "radishmemory_source_capture_audit",
        "radishmemory_source_vault_references",
        "radishmemory_source_vault_attempts",
    ] {
        assert_eq!(count(&root, table), 1);
    }
    assert_eq!(count(&root, "radishmemory_source_bodies"), 0);
    let objects = files(&root);
    assert_eq!(objects.len(), 1);
    assert!(
        !objects[0]
            .1
            .windows(BODY.len())
            .any(|w| w == BODY.as_bytes())
    );
    let retried = run(&dir, &request("new-allocation", "lineage-a", 1, BODY, None)).unwrap();
    assert_eq!(retried.outcome(), SourceCaptureOutcome::Idempotent);
    assert_eq!(retried.source_id(), result.source_id());
    assert_eq!(files(&root), objects);
    assert_eq!(count(&root, "radishmemory_source_vault_attempts"), 1);
    assert!(radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3")).is_err());
}

#[test]
fn changed_bytes_advance_version_and_distinct_bindings_keep_separate_objects() {
    let (root, dir) = setup(true);
    let first = request("source-a", "lineage-a", 1, BODY, None);
    run(&dir, &first).unwrap();
    let second = request(
        "source-b",
        "lineage-a",
        2,
        "Changed synthetic content",
        Some("source-a"),
    );
    assert_eq!(
        run(&dir, &second).unwrap().outcome(),
        SourceCaptureOutcome::Versioned
    );
    run(&dir, &request("source-c", "lineage-c", 1, BODY, None)).unwrap();
    assert_eq!(files(&root).len(), 4);
    assert_eq!(count(&root, "radishmemory_recall_fts"), 3);
    let replay = run(&dir, &first).unwrap();
    assert_eq!(replay.outcome(), SourceCaptureOutcome::Idempotent);
    assert_eq!(replay.source_id().as_str(), "source-a");
    assert_eq!(count(&root, "radishmemory_source_capture_audit"), 4);
    assert_eq!(count(&root, "radishmemory_source_bodies"), 0);
}

#[test]
fn interruption_boundaries_preserve_uncommitted_and_committed_truth() {
    for point in [
        Step::Prepared,
        Step::Published,
        Step::Committed,
        Step::ReadBack,
    ] {
        let (root, dir) = setup(false);
        let req = request("source-a", "lineage-a", 1, BODY, None);
        interrupt(&dir, &req, point);
        let committed = matches!(point, Step::Committed | Step::ReadBack);
        assert_eq!(
            count(&root, "radishmemory_source_artifacts"),
            i64::from(committed)
        );
        assert_eq!(
            count(&root, "radishmemory_source_capture_audit"),
            i64::from(committed)
        );
        assert_eq!(count(&root, "radishmemory_source_bodies"), 0);
        let published = files(&root);
        let result = run(&dir, &req).unwrap();
        assert_eq!(
            result.outcome(),
            if committed {
                SourceCaptureOutcome::Idempotent
            } else {
                SourceCaptureOutcome::Created
            }
        );
        if !published.is_empty() {
            assert_eq!(files(&root), published);
        }
        assert_eq!(count(&root, "radishmemory_source_vault_attempts"), 1);
    }
}

#[test]
fn pending_attempt_rejects_changed_request_and_unrelated_capture_without_resealing() {
    for point in [Step::Prepared, Step::Published] {
        let (root, dir) = setup(false);
        let req = request("source-a", "lineage-a", 1, BODY, None);
        interrupt(&dir, &req, point);
        let before = files(&root);
        assert!(run(&dir, &request("source-new", "lineage-a", 1, BODY, None)).is_err());
        assert!(
            run(
                &dir,
                &request("source-other", "lineage-other", 1, BODY, None)
            )
            .is_err()
        );
        let mut source = req.source().params().clone();
        source.title = Some(text("Changed retry title"));
        let changed = SourceCapture::new(
            req.origin_binding_id().clone(),
            SourceArtifact::new(source).unwrap(),
            req.fragments().to_vec(),
        )
        .unwrap();
        assert!(run(&dir, &changed).is_err());
        let mut fragment = req.fragments()[0].params().clone();
        fragment.segmenter = ProducerRef::new(ProducerType::Rule, id("other-segmenter"), text("2"));
        let changed = SourceCapture::new(
            req.origin_binding_id().clone(),
            req.source().clone(),
            vec![SourceFragment::new(fragment).unwrap()],
        )
        .unwrap();
        assert!(run(&dir, &changed).is_err());
        assert_eq!(files(&root), before);
        assert_eq!(count(&root, "radishmemory_source_artifacts"), 0);
        run(&dir, &req).unwrap();
    }
}

#[test]
fn corrupt_committed_object_blocks_new_capture_and_idempotent_retry() {
    for remove in [false, true] {
        let (root, dir) = setup(false);
        let req = request("source-a", "lineage-a", 1, BODY, None);
        run(&dir, &req).unwrap();
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
        assert!(run(&dir, &req).is_err());
        assert!(run(&dir, &request("source-b", "lineage-b", 1, BODY, None)).is_err());
        assert_eq!(count(&root, "radishmemory_source_artifacts"), 1);
        assert_eq!(count(&root, "radishmemory_source_vault_attempts"), 1);
    }
}

#[test]
fn key_failures_unknown_files_and_truncated_staging_do_not_commit_capture() {
    let (root, dir) = setup(false);
    let req = request("source-a", "lineage-a", 1, BODY, None);
    for code in [
        SourceVaultErrorCode::KeyMissing,
        SourceVaultErrorCode::KeyStoreDenied,
        SourceVaultErrorCode::KeyStoreLocked,
    ] {
        assert!(
            capture(&dir, NS, DEVICE, &req, || Err(SourceVaultError::new(
                code,
                "synthetic provider failure"
            )))
            .is_err()
        );
    }
    interrupt(&dir, &req, Step::Published);
    assert!(
        capture(&dir, NS, DEVICE, &req, || Ok(KeyEncryptionKey::new(
            [0x18; 32]
        )))
        .is_err()
    );
    let unknown = root.0.join("source-objects-v1").join("unknown");
    fs::write(&unknown, b"synthetic retained").unwrap();
    assert!(run(&dir, &req).is_err());
    assert!(unknown.exists());
    assert_eq!(count(&root, "radishmemory_source_artifacts"), 0);
    let (root, dir) = setup(false);
    interrupt(&dir, &req, Step::Prepared);
    let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
    let (l, a): (String, String) = c
        .query_row(
            "SELECT locator,attempt_id FROM radishmemory_source_vault_attempts",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    drop(c);
    let path = root
        .0
        .join("source-staging-v1")
        .join(format!("{l}.{a}.stage"));
    fs::write(&path, b"incomplete ciphertext").unwrap();
    assert!(run(&dir, &req).is_err());
    assert!(path.exists());
    assert_eq!(count(&root, "radishmemory_source_artifacts"), 0);
}

#[test]
fn pending_staging_and_published_links_are_reused_exactly() {
    for only_staging in [false, true] {
        let (root, dir) = setup(false);
        let req = request("source-a", "lineage-a", 1, BODY, None);
        interrupt(&dir, &req, Step::Published);
        let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
        let (l, a): (String, String) = c
            .query_row(
                "SELECT locator,attempt_id FROM radishmemory_source_vault_attempts",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        drop(c);
        let object = root.0.join("source-objects-v1").join(format!("{l}.rmo"));
        let stage = root
            .0
            .join("source-staging-v1")
            .join(format!("{l}.{a}.stage"));
        let before = fs::read(&object).unwrap();
        fs::hard_link(&object, &stage).unwrap();
        if only_staging {
            fs::remove_file(&object).unwrap();
        }
        run(&dir, &req).unwrap();
        assert_eq!(fs::read(&object).unwrap(), before);
        assert!(!stage.exists());
    }
}

#[test]
fn derived_binding_and_request_metadata_drift_block_even_idempotent_capture() {
    for sql in [
        "DELETE FROM radishmemory_recall_fts",
        "DELETE FROM radishmemory_source_origin_bindings",
        "UPDATE radishmemory_source_artifacts SET title='changed persisted title'",
        "UPDATE radishmemory_source_fragments SET segmenter_version='changed'",
        "UPDATE radishmemory_source_vault_attempts SET request_digest=printf('%064d',0)",
    ] {
        let (root, dir) = setup(false);
        let req = request("source-a", "lineage-a", 1, BODY, None);
        run(&dir, &req).unwrap();
        Connection::open(root.0.join("library.sqlite3"))
            .unwrap()
            .execute_batch(sql)
            .unwrap();
        assert!(run(&dir, &req).is_err());
        assert_eq!(count(&root, "radishmemory_source_vault_attempts"), 1);
    }
}

#[test]
fn invalid_new_version_or_fragment_collision_never_reserves_an_attempt() {
    let (root, dir) = setup(false);
    let first = request("source-a", "lineage-a", 1, BODY, None);
    run(&dir, &first).unwrap();
    assert!(
        run(
            &dir,
            &request("source-c", "lineage-a", 3, "changed", Some("source-a"))
        )
        .is_err()
    );
    let other = request("source-other", "lineage-other", 1, BODY, None);
    let mut f = other.fragments()[0].params().clone();
    f.fragment_id = first.fragments()[0].params().fragment_id.clone();
    let collision = SourceCapture::new(
        other.origin_binding_id().clone(),
        other.source().clone(),
        vec![SourceFragment::new(f).unwrap()],
    )
    .unwrap();
    assert!(run(&dir, &collision).is_err());
    assert_eq!(count(&root, "radishmemory_source_vault_attempts"), 1);
}

#[test]
fn process_exit_after_publish_or_commit_retries_without_duplicate_provenance() {
    for point in ["Prepared", "Published", "Committed", "ReadBack"] {
        let (root, dir) = setup(false);
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "capture::tests::crash_helper", "--ignored"])
            .env("RADISHMEMORY_CAPTURE_SYNTHETIC_ROOT", &root.0)
            .env("RADISHMEMORY_CAPTURE_SYNTHETIC_STEP", point)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(74));
        run(&dir, &request("source-a", "lineage-a", 1, BODY, None)).unwrap();
        assert_eq!(count(&root, "radishmemory_source_artifacts"), 1);
        assert_eq!(count(&root, "radishmemory_source_capture_audit"), 1);
        assert_eq!(count(&root, "radishmemory_source_vault_attempts"), 1);
    }
}
#[test]
#[ignore = "executed by parent against isolated synthetic fixture"]
fn crash_helper() {
    let Some(path) = std::env::var_os("RADISHMEMORY_CAPTURE_SYNTHETIC_ROOT") else {
        return;
    };
    let point = std::env::var("RADISHMEMORY_CAPTURE_SYNTHETIC_STEP").unwrap();
    let dir = ObjectDirectory::open_application_directory(path).unwrap();
    capture_with_step(
        &dir,
        NS,
        DEVICE,
        &request("source-a", "lineage-a", 1, BODY, None),
        || Ok(key()),
        |s| {
            if format!("{s:?}") == point {
                std::process::exit(74);
            }
            Ok(())
        },
    )
    .unwrap();
    panic!("checkpoint not reached");
}

#[test]
fn incomplete_migration_never_loads_key_or_reserves_capture() {
    let (root, dir) = setup(false);
    Connection::open(root.0.join("library.sqlite3"))
        .unwrap()
        .execute(
            "UPDATE radishmemory_source_vault_migration SET state='migrating'",
            [],
        )
        .unwrap();
    assert!(
        capture(
            &dir,
            NS,
            DEVICE,
            &request("source-a", "lineage-a", 1, BODY, None),
            || panic!("must reject before provider access")
        )
        .is_err()
    );
    assert_eq!(count(&root, "radishmemory_source_vault_attempts"), 0);
}

#[test]
fn publish_collision_keeps_prepared_attempt_without_canonical_success() {
    let (root, dir) = setup(false);
    let req = request("source-a", "lineage-a", 1, BODY, None);
    let write = ObjectWrite::seal(
        &key(),
        &metadata_for_source(req.source()).unwrap(),
        BODY.as_bytes(),
    )
    .unwrap();
    let object = root
        .0
        .join("source-objects-v1")
        .join(format!("{}.rmo", write.locator().token()));
    assert!(
        capture_with_step(
            &dir,
            NS,
            DEVICE,
            &req,
            || Ok(key()),
            |s| {
                if s == Step::Prepared {
                    fs::write(&object, b"synthetic collision retained").unwrap();
                }
                Ok(())
            }
        )
        .is_err()
    );
    assert_eq!(count(&root, "radishmemory_source_artifacts"), 0);
    assert_eq!(count(&root, "radishmemory_source_vault_attempts"), 1);
    assert_eq!(fs::read(object).unwrap(), b"synthetic collision retained");
}

#[test]
fn altered_pending_metadata_is_rejected_before_publication() {
    for sql in [
        "UPDATE radishmemory_source_vault_attempts SET content_length=1",
        "UPDATE radishmemory_source_vault_attempts SET origin_binding_id='origin-binding-other'",
    ] {
        let (root, dir) = setup(false);
        let req = request("source-a", "lineage-a", 1, BODY, None);
        interrupt(&dir, &req, Step::Prepared);
        Connection::open(root.0.join("library.sqlite3"))
            .unwrap()
            .execute_batch(sql)
            .unwrap();
        assert!(run(&dir, &req).is_err());
        assert_eq!(count(&root, "radishmemory_source_artifacts"), 0);
        assert!(files(&root).is_empty());
    }
}

#[test]
fn checkpoint_lock_and_diagnostics_keep_their_boundaries() {
    use std::error::Error;
    let (root, dir) = setup(false);
    let req = request("source-a", "lineage-a", 1, BODY, None);
    capture_with_step(
        &dir,
        NS,
        DEVICE,
        &req,
        || Ok(key()),
        |_| {
            let c = Connection::open(root.0.join("library.sqlite3")).unwrap();
            c.busy_timeout(std::time::Duration::ZERO).unwrap();
            assert_eq!(
                c.execute_batch("BEGIN IMMEDIATE")
                    .unwrap_err()
                    .sqlite_error_code(),
                Some(rusqlite::ErrorCode::DatabaseBusy)
            );
            Ok(())
        },
    )
    .unwrap();
    Connection::open(root.0.join("library.sqlite3"))
        .unwrap()
        .execute_batch("CREATE TABLE synthetic_private_drift(value TEXT)")
        .unwrap();
    let error = run(&dir, &req).unwrap_err();
    let text = format!("{error} {error:?}");
    for forbidden in [
        root.0.to_str().unwrap(),
        NS,
        DEVICE,
        BODY,
        "synthetic_private_drift",
        "CREATE TABLE",
    ] {
        assert!(!text.contains(forbidden));
    }
    assert!(error.source().is_none());
}
