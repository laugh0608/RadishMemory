use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use radishmemory_core::{
    ActorRef, ActorType, CanonicalObjectType, ComponentStatus, DeleteRequest, DeleteRequestParams,
    DeletionComponentType, DeletionEvidence, DeletionEvidenceParams, DeletionOverallStatus,
    DeletionState, DeletionStore, DeletionTarget, DeletionTargetRef, EgressPolicy, EvidenceRef,
    EvidenceType, FrozenTargetClosure, Governance, Identifier, LocalDeletionExecution, LocalSearch,
    LocalSearchHit, LocalSearchRequest, NonEmptyText, ObjectRef, ProducerRef, ProducerType,
    RequestedGuarantee, RequiredAction, RetentionMode, RetentionRule, Sensitivity, SourceCapture,
    SourceCaptureOutcome, SourceCaptureStore, SourceVault, Timestamp, Version, compute_digest,
};
use radishmemory_file_entry::{
    FileCapturePlan, FileCaptureReceipt, FileEntryError, FileEntryErrorReason, FileExportRequest,
    FileReadRequest, MAX_FILE_BYTES, build_source_capture, export_managed_source,
    read_file_snapshot,
};
use radishmemory_sqlite::{SqliteDatabase, SqliteErrorCode, SqliteStorageReason};
use rusqlite::{Connection, params};

mod support;

use support::SyntheticDatabase;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

const DELETION_PROFILE: [(&str, DeletionComponentType, RequiredAction); 10] = [
    (
        "source-body",
        DeletionComponentType::SourceBody,
        RequiredAction::Delete,
    ),
    (
        "source-metadata",
        DeletionComponentType::SourceMetadata,
        RequiredAction::RetainMinimal,
    ),
    (
        "source-fragment",
        DeletionComponentType::SourceFragment,
        RequiredAction::Delete,
    ),
    (
        "memory-proposal",
        DeletionComponentType::MemoryProposal,
        RequiredAction::Redact,
    ),
    (
        "memory-decision",
        DeletionComponentType::MemoryDecision,
        RequiredAction::RetainMinimal,
    ),
    (
        "memory-record",
        DeletionComponentType::MemoryRecord,
        RequiredAction::Redact,
    ),
    (
        "memory-state-event",
        DeletionComponentType::MemoryStateEvent,
        RequiredAction::RetainMinimal,
    ),
    (
        "full-text-index",
        DeletionComponentType::FullTextIndex,
        RequiredAction::Delete,
    ),
    (
        "context-cache",
        DeletionComponentType::ContextCache,
        RequiredAction::Delete,
    ),
    (
        "minimal-audit",
        DeletionComponentType::MinimalAudit,
        RequiredAction::RetainMinimal,
    ),
];

struct SyntheticDirectory {
    path: PathBuf,
}

impl SyntheticDirectory {
    fn new(label: &str) -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "radishmemory-source-capture-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("synthetic directory must be created");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn file(&self) -> PathBuf {
        self.path.join("selected-note.txt")
    }

    fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn assert_no_export_temporary(&self) {
        let has_temporary = fs::read_dir(&self.path)
            .expect("synthetic directory must be readable")
            .any(|entry| {
                entry
                    .expect("synthetic directory entry must be readable")
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".radishmemory-export-")
            });
        assert!(!has_temporary, "export temporary file must be cleaned");
    }
}

impl Drop for SyntheticDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).expect("synthetic directory must be removed");
    }
}

fn id(value: &str) -> Identifier {
    Identifier::new(value).expect("synthetic identifier must be valid")
}

fn text(value: &str) -> NonEmptyText {
    NonEmptyText::new(value).expect("synthetic text must be nonempty")
}

fn timestamp(value: &str) -> Timestamp {
    Timestamp::parse(value).expect("synthetic timestamp must be valid")
}

fn governance() -> Governance {
    Governance::new(
        Sensitivity::Personal,
        EgressPolicy::LocalOnly,
        RetentionRule::new(RetentionMode::UntilDeleted, None, None)
            .expect("retention must be valid"),
        DeletionState::Active,
        id("policy-local-only"),
    )
    .expect("governance must be valid")
}

fn producer(producer_type: ProducerType, producer_id: &str) -> ProducerRef {
    ProducerRef::new(producer_type, id(producer_id), text("1"))
}

fn actor() -> ActorRef {
    ActorRef::new(ActorType::TestFixture, id("fixture-actor"), Some(text("1")))
}

fn lineage_delete_request(request_id: &str, source_ids: &[&str]) -> DeleteRequest {
    let target_refs = source_ids
        .iter()
        .map(|source_id| ObjectRef::new(CanonicalObjectType::SourceArtifact, id(source_id)))
        .collect::<Vec<_>>();
    let target_ref = if target_refs.len() == 1 {
        DeletionTargetRef::Object(target_refs[0].clone())
    } else {
        DeletionTargetRef::FrozenClosure(
            FrozenTargetClosure::freeze(target_refs.clone()).expect("closure must freeze"),
        )
    };
    let target_count = u64::try_from(target_refs.len()).expect("target count must fit");
    let planned_components = DELETION_PROFILE
        .iter()
        .map(|(key, component_type, action)| {
            DeletionTarget::new(
                id(key),
                *component_type,
                target_ref.clone(),
                target_count,
                *action,
            )
            .expect("deletion component must be valid")
        })
        .collect();
    DeleteRequest::new(DeleteRequestParams {
        delete_request_id: id(request_id),
        namespace_id: id("namespace-1"),
        requested_by: actor(),
        authorization_basis: text("explicit-synthetic-lineage-deletion"),
        requested_guarantee: RequestedGuarantee::LocalPurge,
        device_id: id("device-local-1"),
        target_refs,
        planned_components,
        reason_code: text("synthetic-lineage-local-purge"),
        requested_at: timestamp("2026-08-29T10:00:00Z"),
    })
    .expect("lineage delete request must be valid")
}

fn deletion_execution() -> LocalDeletionExecution {
    LocalDeletionExecution::new(
        timestamp("2026-08-29T10:01:00Z"),
        EvidenceRef::new(EvidenceType::PolicyBasis, id("policy-local-deletion-v1")),
    )
    .expect("deletion execution must be valid")
}

fn deletion_evidence(
    request: &DeleteRequest,
    results: Vec<radishmemory_core::ComponentResult>,
) -> DeletionEvidence {
    DeletionEvidence::new(DeletionEvidenceParams {
        deletion_evidence_id: id("deletion-evidence-lineage-1"),
        delete_request_id: request.params().delete_request_id.clone(),
        previous_evidence_id: None,
        namespace_id: request.params().namespace_id.clone(),
        device_id: request.params().device_id.clone(),
        overall_status: DeletionOverallStatus::Completed,
        component_results: results,
        started_at: timestamp("2026-08-29T10:01:00Z"),
        finished_at: Some(timestamp("2026-08-29T10:02:00Z")),
        verified_by: producer(ProducerType::TestFixture, "deletion-verifier"),
        evidence_digest: compute_digest(
            "deletion-evidence-v1",
            r#"{"evidence":"synthetic-lineage-1"}"#,
        )
        .expect("evidence digest must compute"),
    })
    .expect("deletion evidence must be valid")
}

struct CaptureSpec<'a> {
    source_id: &'a str,
    lineage_id: &'a str,
    fragment_id: &'a str,
    version: u64,
    supersedes: Vec<Identifier>,
    captured_at: &'a str,
}

fn capture_from_file(directory: &SyntheticDirectory, spec: CaptureSpec<'_>) -> SourceCapture {
    capture_from_file_with_binding(directory, spec, "origin-binding-1")
        .expect("capture candidate must be valid")
}

fn capture_from_file_with_binding(
    directory: &SyntheticDirectory,
    spec: CaptureSpec<'_>,
    origin_binding_id: &str,
) -> Result<SourceCapture, radishmemory_core::CoreError> {
    let file = directory.file();
    capture_from_path_with_binding(directory, &file, spec, origin_binding_id)
}

fn capture_from_path_with_binding(
    directory: &SyntheticDirectory,
    file: &Path,
    spec: CaptureSpec<'_>,
    origin_binding_id: &str,
) -> Result<SourceCapture, radishmemory_core::CoreError> {
    let snapshot = read_file_snapshot(
        &FileReadRequest::new(file, vec![directory.path().to_path_buf()])
            .expect("read request must be valid"),
    )
    .expect("synthetic file must produce a snapshot");
    build_source_capture(snapshot, capture_plan(spec, origin_binding_id))
}

fn capture_plan(spec: CaptureSpec<'_>, origin_binding_id: &str) -> FileCapturePlan {
    FileCapturePlan {
        namespace_id: id("namespace-1"),
        origin_binding_id: id(origin_binding_id),
        source_id: id(spec.source_id),
        lineage_id: id(spec.lineage_id),
        version: Version::new(spec.version).expect("version must be positive"),
        supersedes_source_ids: spec.supersedes,
        fragment_id: id(spec.fragment_id),
        observed_at: timestamp(spec.captured_at),
        captured_at: timestamp(spec.captured_at),
        governance: governance(),
        source_producer: producer(ProducerType::Parser, "file-entry-parser"),
        segmenter: producer(ProducerType::Rule, "whole-file-segmenter"),
    }
}

fn capture_selected_file(
    database: &mut SqliteDatabase,
    request: &FileReadRequest,
    spec: CaptureSpec<'_>,
    origin_binding_id: &str,
) -> Result<FileCaptureReceipt, FileEntryError> {
    let snapshot = read_file_snapshot(request)?;
    let capture = build_source_capture(snapshot, capture_plan(spec, origin_binding_id))
        .expect("validated snapshot and synthetic plan must build a canonical capture");
    let result = database
        .capture_source(&capture)
        .expect("canonical capture must commit before a receipt is returned");
    FileCaptureReceipt::from_capture_result(&result)
}

fn seed_two_version_file_lineage(
    database: &mut SqliteDatabase,
    directory: &SyntheticDirectory,
    historical_bytes: &[u8],
    current_bytes: &[u8],
) {
    fs::write(directory.file(), historical_bytes).expect("version one file must be written");
    let first = capture_from_file(
        directory,
        CaptureSpec {
            source_id: "source-1",
            lineage_id: "lineage-1",
            fragment_id: "fragment-1",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
    );
    database
        .capture_source(&first)
        .expect("version one capture must commit");

    fs::write(directory.file(), current_bytes).expect("version two file must be written");
    let second = capture_from_file(
        directory,
        CaptureSpec {
            source_id: "source-2",
            lineage_id: "lineage-1",
            fragment_id: "fragment-2",
            version: 2,
            supersedes: vec![id("source-1")],
            captured_at: "2026-08-29T09:00:00Z",
        },
    );
    database
        .capture_source(&second)
        .expect("version two capture must commit");
}

fn search(database: &SqliteDatabase, query: &str) -> Vec<LocalSearchHit> {
    database
        .search(
            &LocalSearchRequest::new(
                id("namespace-1"),
                text(query),
                timestamp("2026-08-30T00:00:00Z"),
                5,
                [Sensitivity::Personal],
            )
            .expect("search request must be valid"),
        )
        .expect("search must succeed")
}

fn table_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("synthetic table count must be queryable")
}

#[derive(Debug, Eq, PartialEq)]
struct SourceEntryCounts {
    sources: i64,
    bodies: i64,
    fragments: i64,
    tips: i64,
    bindings: i64,
    audits: i64,
    full_text_rows: i64,
}

fn source_entry_counts(database_path: &Path) -> SourceEntryCounts {
    let connection = Connection::open(database_path).expect("database must open for inspection");
    SourceEntryCounts {
        sources: table_count(&connection, "radishmemory_source_artifacts"),
        bodies: table_count(&connection, "radishmemory_source_bodies"),
        fragments: table_count(&connection, "radishmemory_source_fragments"),
        tips: table_count(&connection, "radishmemory_source_lineage_tips"),
        bindings: table_count(&connection, "radishmemory_source_origin_bindings"),
        audits: table_count(&connection, "radishmemory_source_capture_audit"),
        full_text_rows: table_count(&connection, "radishmemory_recall_fts"),
    }
}

fn seed_rejection_baseline(
    database: &mut SqliteDatabase,
    directory: &SyntheticDirectory,
) -> FileCaptureReceipt {
    let file = directory.file();
    fs::write(&file, b"Stable rejection baseline marker.\n")
        .expect("baseline source must be written");
    let request = FileReadRequest::new(&file, vec![directory.path().to_path_buf()])
        .expect("baseline request must be valid");
    capture_selected_file(
        database,
        &request,
        CaptureSpec {
            source_id: "source-rejection-baseline",
            lineage_id: "lineage-rejection-baseline",
            fragment_id: "fragment-rejection-baseline",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
        "origin-binding-rejection-baseline",
    )
    .expect("baseline capture must return a receipt")
}

fn rejected_capture(
    database: &mut SqliteDatabase,
    request: &FileReadRequest,
    expected_reason: FileEntryErrorReason,
) -> FileEntryError {
    let error = capture_selected_file(
        database,
        request,
        CaptureSpec {
            source_id: "source-must-not-exist",
            lineage_id: "lineage-must-not-exist",
            fragment_id: "fragment-must-not-exist",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T09:00:00Z",
        },
        "origin-binding-must-not-exist",
    )
    .expect_err("rejected file must not return a capture receipt");
    assert_eq!(error.reason(), expected_reason);
    error
}

#[test]
fn p1_f01_first_capture_commits_complete_source_and_path_free_receipt() {
    let directory = SyntheticDirectory::new("first");
    fs::write(directory.file(), b"Orchard alpha is the current note.\r\n")
        .expect("synthetic file must be written");
    let synthetic = SyntheticDatabase::new("atomic-first");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let capture = capture_from_file(
        &directory,
        CaptureSpec {
            source_id: "source-1",
            lineage_id: "lineage-1",
            fragment_id: "fragment-1",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
    );

    let result = database
        .capture_source(&capture)
        .expect("first capture must commit");
    assert_eq!(result.outcome(), SourceCaptureOutcome::Created);
    assert!(!format!("{result:?}").contains(result.content_digest().value()));
    let receipt = FileCaptureReceipt::from_capture_result(&result)
        .expect("successful result must map to a receipt");
    assert_eq!(receipt.source_id(), &id("source-1"));
    assert!(!format!("{receipt:?}").contains("selected-note"));
    assert!(!format!("{receipt:?}").contains("Orchard alpha"));

    let source = database
        .load_source_artifact(&id("namespace-1"), &id("source-1"))
        .expect("source lookup must succeed")
        .expect("source must exist");
    assert_eq!(
        source.params().content.as_str().as_bytes(),
        b"Orchard alpha is the current note.\r\n"
    );
    let fragments = database
        .load_source_fragments(&id("namespace-1"), &id("source-1"))
        .expect("fragment lookup must succeed")
        .expect("fragment set must exist");
    assert_eq!(fragments.len(), 1);
    assert_eq!(fragments[0].params().byte_start, 0);
    assert_eq!(
        fragments[0].params().byte_end,
        source.params().content_length
    );
    assert_eq!(search(&database, "Orchard alpha").len(), 1);

    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    for table in [
        "radishmemory_source_artifacts",
        "radishmemory_source_bodies",
        "radishmemory_source_fragments",
        "radishmemory_source_lineage_tips",
        "radishmemory_source_origin_bindings",
        "radishmemory_source_capture_audit",
        "radishmemory_recall_fts",
    ] {
        assert_eq!(table_count(&raw, table), 1, "unexpected count for {table}");
    }
}

#[test]
fn p1_f02_capture_reload_and_rebuild_preserve_every_utf8_byte() {
    let directory = SyntheticDirectory::new("byte-preservation");
    let file = directory.child("preserved-note.md");
    let exact_bytes =
        "\u{feff}# Byte preservation\r\nCafe\u{301} / Caf\u{e9}\r\n尾行\r\n".as_bytes();
    fs::write(&file, exact_bytes).expect("synthetic markdown must be written");
    let synthetic = SyntheticDatabase::new("byte-preservation");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let capture = capture_from_path_with_binding(
        &directory,
        &file,
        CaptureSpec {
            source_id: "source-byte-preserved",
            lineage_id: "lineage-byte-preserved",
            fragment_id: "fragment-byte-preserved",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
        "origin-binding-byte-preserved",
    )
    .expect("byte-preserving capture candidate must be valid");

    assert_eq!(
        capture.source().params().content.as_str().as_bytes(),
        exact_bytes
    );
    assert_eq!(
        capture.source().params().content_length,
        u64::try_from(exact_bytes.len()).expect("synthetic byte length must fit")
    );
    assert_eq!(capture.fragments().len(), 1);
    assert_eq!(capture.fragments()[0].params().byte_start, 0);
    assert_eq!(
        capture.fragments()[0].params().byte_end,
        capture.source().params().content_length
    );
    assert_eq!(
        capture.fragments()[0].params().content.as_str().as_bytes(),
        exact_bytes
    );
    assert_eq!(
        &capture.fragments()[0].params().content_digest,
        &capture.source().params().content_digest
    );

    let result = database
        .capture_source(&capture)
        .expect("byte-preserving capture must commit");
    assert_eq!(result.outcome(), SourceCaptureOutcome::Created);
    assert_eq!(
        result.content_length(),
        u64::try_from(exact_bytes.len()).expect("synthetic byte length must fit")
    );
    assert_eq!(
        result.content_digest(),
        &capture.source().params().content_digest
    );
    drop(database);

    let raw = Connection::open(synthetic.path()).expect("database must reopen for raw inspection");
    let stored_body: Vec<u8> = raw
        .query_row(
            "SELECT content FROM radishmemory_source_bodies WHERE source_id = ?1",
            params!["source-byte-preserved"],
            |row| row.get(0),
        )
        .expect("managed body must be queryable as exact bytes");
    assert_eq!(stored_body, exact_bytes);
    drop(raw);

    let mut database =
        SqliteDatabase::open(synthetic.path()).expect("database with exact bytes must reopen");
    let source = database
        .load_source_artifact(&id("namespace-1"), &id("source-byte-preserved"))
        .expect("reopened source lookup must succeed")
        .expect("reopened source must exist");
    let fragments = database
        .load_source_fragments(&id("namespace-1"), &id("source-byte-preserved"))
        .expect("reopened fragment lookup must succeed")
        .expect("reopened fragment set must exist");
    assert_eq!(source.params().content.as_str().as_bytes(), exact_bytes);
    assert_eq!(source.params().content_length, exact_bytes.len() as u64);
    assert_eq!(fragments.len(), 1);
    assert_eq!(fragments[0].params().byte_start, 0);
    assert_eq!(fragments[0].params().byte_end, exact_bytes.len() as u64);
    assert_eq!(
        fragments[0].params().content.as_str().as_bytes(),
        exact_bytes
    );
    assert_eq!(
        fragments[0].params().content_digest,
        source.params().content_digest
    );

    database
        .rebuild_recall_derivations()
        .expect("rebuild must preserve canonical exact bytes");
    let rebuilt = database
        .load_source_artifact(&id("namespace-1"), &id("source-byte-preserved"))
        .expect("rebuilt source lookup must succeed")
        .expect("rebuilt source must exist");
    assert_eq!(rebuilt.params().content.as_str().as_bytes(), exact_bytes);
}

#[test]
fn path_like_origin_binding_is_rejected_before_persistence() {
    let directory = SyntheticDirectory::new("binding-redaction");
    fs::write(directory.file(), b"Binding validation marker.\n")
        .expect("synthetic file must be written");
    let error = capture_from_file_with_binding(
        &directory,
        CaptureSpec {
            source_id: "source-1",
            lineage_id: "lineage-1",
            fragment_id: "fragment-1",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
        "/private/synthetic-note.txt",
    )
    .expect_err("path-like binding must not enter canonical metadata");
    assert_eq!(
        error.cross_object_invariant_reason(),
        Some(radishmemory_core::CrossObjectInvariantReason::OriginBindingMismatch)
    );
    assert!(!format!("{error:?}").contains("synthetic-note"));
}

#[test]
fn p1_f03_same_binding_and_exact_bytes_are_idempotent() {
    let directory = SyntheticDirectory::new("idempotent");
    fs::write(directory.file(), b"Idempotent orchard note.\n")
        .expect("synthetic file must be written");
    let synthetic = SyntheticDatabase::new("atomic-idempotent");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let first = capture_from_file(
        &directory,
        CaptureSpec {
            source_id: "source-1",
            lineage_id: "lineage-1",
            fragment_id: "fragment-1",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
    );
    database
        .capture_source(&first)
        .expect("first capture must commit");
    let repeated = capture_from_file(
        &directory,
        CaptureSpec {
            source_id: "source-unused",
            lineage_id: "lineage-1",
            fragment_id: "fragment-unused",
            version: 2,
            supersedes: vec![id("source-1")],
            captured_at: "2026-08-29T08:05:00Z",
        },
    );

    let result = database
        .capture_source(&repeated)
        .expect("exact repeated capture must be idempotent");
    assert_eq!(result.outcome(), SourceCaptureOutcome::Idempotent);
    assert_eq!(result.source_id(), &id("source-1"));
    assert_eq!(
        result.version(),
        Version::new(1).expect("version must be valid")
    );

    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    for table in [
        "radishmemory_source_artifacts",
        "radishmemory_source_fragments",
        "radishmemory_source_capture_audit",
        "radishmemory_recall_fts",
    ] {
        assert_eq!(table_count(&raw, table), 1, "unexpected count for {table}");
    }
}

#[test]
fn p1_f04_and_f06_changed_bytes_create_new_tip_and_recall_only_current_version() {
    let directory = SyntheticDirectory::new("version");
    fs::write(directory.file(), b"Legacy alpha marker.\n")
        .expect("version one file must be written");
    let synthetic = SyntheticDatabase::new("atomic-version");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let first = capture_from_file(
        &directory,
        CaptureSpec {
            source_id: "source-1",
            lineage_id: "lineage-1",
            fragment_id: "fragment-1",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
    );
    database
        .capture_source(&first)
        .expect("first capture must commit");

    fs::write(directory.file(), b"Current beta marker.\n")
        .expect("version two file must be written");
    let second = capture_from_file(
        &directory,
        CaptureSpec {
            source_id: "source-2",
            lineage_id: "lineage-1",
            fragment_id: "fragment-2",
            version: 2,
            supersedes: vec![id("source-1")],
            captured_at: "2026-08-29T09:00:00Z",
        },
    );
    let result = database
        .capture_source(&second)
        .expect("changed bytes must create a version");
    assert_eq!(result.outcome(), SourceCaptureOutcome::Versioned);
    assert_eq!(result.source_id(), &id("source-2"));
    assert!(search(&database, "Legacy alpha").is_empty());
    let hits = search(&database, "Current beta");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].object_id(), &id("fragment-2"));
    let LocalSearchHit::SourceFragment(fragment) = &hits[0] else {
        panic!("file capture search must return a source fragment");
    };
    assert_eq!(fragment.params().source_id, id("source-2"));
    assert_eq!(fragment.params().byte_start, 0);
    assert_eq!(fragment.params().byte_end, result.content_length());
    assert_eq!(&fragment.params().content_digest, result.content_digest());
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-1"))
            .expect("history lookup must succeed")
            .is_some()
    );

    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    assert_eq!(table_count(&raw, "radishmemory_source_artifacts"), 2);
    assert_eq!(table_count(&raw, "radishmemory_source_fragments"), 2);
    assert_eq!(table_count(&raw, "radishmemory_source_capture_audit"), 2);
    assert_eq!(table_count(&raw, "radishmemory_recall_fts"), 1);
    let tip: (String, i64) = raw
        .query_row(
            "SELECT source_id, version FROM radishmemory_source_lineage_tips
             WHERE namespace_id = ?1 AND lineage_id = ?2",
            params!["namespace-1", "lineage-1"],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("tip must be queryable");
    assert_eq!(tip, ("source-2".to_owned(), 2));
}

#[test]
fn p1_f05_hardlink_bindings_keep_independent_lineages_and_deletion_scopes() {
    let directory = SyntheticDirectory::new("hardlink-provenance");
    let original = directory.file();
    let alias = directory.child("hardlink-alias.txt");
    let exact_bytes = b"Independent hardlink provenance marker.\r\n";
    fs::write(&original, exact_bytes).expect("synthetic origin must be written");
    fs::hard_link(&original, &alias).expect("synthetic hardlink alias must be created");
    let synthetic = SyntheticDatabase::new("hardlink-provenance");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");

    let first = capture_from_path_with_binding(
        &directory,
        &original,
        CaptureSpec {
            source_id: "source-hardlink-a",
            lineage_id: "lineage-hardlink-a",
            fragment_id: "fragment-hardlink-a",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
        "origin-binding-hardlink-a",
    )
    .expect("first hardlink capture candidate must be valid");
    let second = capture_from_path_with_binding(
        &directory,
        &alias,
        CaptureSpec {
            source_id: "source-hardlink-b",
            lineage_id: "lineage-hardlink-b",
            fragment_id: "fragment-hardlink-b",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:01:00Z",
        },
        "origin-binding-hardlink-b",
    )
    .expect("second hardlink capture candidate must be valid");
    assert_eq!(
        first.source().params().content_digest,
        second.source().params().content_digest
    );

    assert_eq!(
        database
            .capture_source(&first)
            .expect("first hardlink capture must commit")
            .outcome(),
        SourceCaptureOutcome::Created
    );
    assert_eq!(
        database
            .capture_source(&second)
            .expect("second hardlink capture must commit independently")
            .outcome(),
        SourceCaptureOutcome::Created
    );
    let hits = search(&database, "hardlink provenance");
    assert_eq!(hits.len(), 2);
    assert!(
        hits.iter()
            .any(|hit| hit.object_id() == &id("fragment-hardlink-a"))
    );
    assert!(
        hits.iter()
            .any(|hit| hit.object_id() == &id("fragment-hardlink-b"))
    );

    let raw = Connection::open(synthetic.path()).expect("database must reopen for inspection");
    assert_eq!(table_count(&raw, "radishmemory_source_artifacts"), 2);
    assert_eq!(table_count(&raw, "radishmemory_source_bodies"), 2);
    assert_eq!(table_count(&raw, "radishmemory_source_lineage_tips"), 2);
    assert_eq!(table_count(&raw, "radishmemory_source_origin_bindings"), 2);
    assert_eq!(table_count(&raw, "radishmemory_source_capture_audit"), 2);
    drop(raw);

    let request = lineage_delete_request("delete-request-hardlink-a", &["source-hardlink-a"]);
    database
        .store_delete_request(&request)
        .expect("one hardlink lineage must delete without expanding to the other");
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-hardlink-a"))
            .expect("closed first lineage lookup must succeed")
            .is_none()
    );
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-hardlink-b"))
            .expect("independent second lineage lookup must succeed")
            .is_some()
    );
    let hits = search(&database, "hardlink provenance");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].object_id(), &id("fragment-hardlink-b"));

    let results = database
        .execute_deletion(
            &id("namespace-1"),
            &id("delete-request-hardlink-a"),
            &deletion_execution(),
        )
        .expect("first hardlink lineage deletion must execute");
    assert_eq!(results.len(), 10);
    assert!(
        results
            .iter()
            .all(|result| result.params().status == ComponentStatus::Succeeded)
    );
    let evidence = deletion_evidence(&request, results);
    database
        .store_deletion_evidence(&evidence)
        .expect("first hardlink lineage evidence must persist");
    drop(database);

    assert_eq!(
        fs::read(&original).expect("external origin must remain readable"),
        exact_bytes
    );
    assert_eq!(
        fs::read(&alias).expect("external hardlink alias must remain readable"),
        exact_bytes
    );

    let raw = Connection::open(synthetic.path()).expect("database must reopen for inspection");
    let remaining: (i64, i64, i64, i64, i64, String, String) = raw
        .query_row(
            "SELECT
                 (SELECT COUNT(*) FROM radishmemory_source_artifacts
                  WHERE source_id = 'source-hardlink-a' AND deletion_state = 'deleted'),
                 (SELECT COUNT(*) FROM radishmemory_source_artifacts
                  WHERE source_id = 'source-hardlink-b' AND deletion_state = 'active'),
                 (SELECT COUNT(*) FROM radishmemory_source_bodies),
                 (SELECT COUNT(*) FROM radishmemory_source_fragments),
                 (SELECT COUNT(*) FROM radishmemory_source_capture_audit),
                 (SELECT origin_binding_id FROM radishmemory_source_origin_bindings),
                 (SELECT lineage_id FROM radishmemory_source_lineage_tips)",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .expect("independent remaining lineage must be queryable");
    assert_eq!(
        remaining,
        (
            1,
            1,
            1,
            1,
            1,
            "origin-binding-hardlink-b".to_owned(),
            "lineage-hardlink-b".to_owned(),
        )
    );
    drop(raw);

    let mut database =
        SqliteDatabase::open(synthetic.path()).expect("independent lineage database must reopen");
    database
        .rebuild_recall_derivations()
        .expect("rebuild must retain only the independent active lineage");
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-hardlink-a"))
            .expect("deleted lineage lookup must succeed")
            .is_none()
    );
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-hardlink-b"))
            .expect("active lineage lookup must succeed")
            .is_some()
    );
    let hits = search(&database, "hardlink provenance");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].object_id(), &id("fragment-hardlink-b"));
}

#[test]
fn p1_f07_current_and_historical_sources_export_exact_managed_bytes() {
    let directory = SyntheticDirectory::new("export-round-trip");
    let historical_bytes = "\u{feff}Legacy heading\r\nCafe\u{301}\r\n".as_bytes();
    fs::write(directory.file(), historical_bytes).expect("version one file must be written");
    let synthetic = SyntheticDatabase::new("export-round-trip");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let first = capture_from_file(
        &directory,
        CaptureSpec {
            source_id: "source-1",
            lineage_id: "lineage-1",
            fragment_id: "fragment-1",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
    );
    database
        .capture_source(&first)
        .expect("first capture must commit");

    let current_bytes = "\u{feff}Current heading\r\nCaf\u{e9}\r\nFinal line".as_bytes();
    fs::write(directory.file(), current_bytes).expect("version two file must be written");
    let second = capture_from_file(
        &directory,
        CaptureSpec {
            source_id: "source-2",
            lineage_id: "lineage-1",
            fragment_id: "fragment-2",
            version: 2,
            supersedes: vec![id("source-1")],
            captured_at: "2026-08-29T09:00:00Z",
        },
    );
    database
        .capture_source(&second)
        .expect("changed bytes must create a version");

    let historical = database
        .load_source_artifact(&id("namespace-1"), &id("source-1"))
        .expect("historical lookup must succeed")
        .expect("historical source must remain readable");
    let current = database
        .load_source_artifact(&id("namespace-1"), &id("source-2"))
        .expect("current lookup must succeed")
        .expect("current source must remain readable");
    let historical_target = directory.child("historical-export.txt");
    let current_target = directory.child("current-export.txt");
    let historical_receipt = export_managed_source(
        &historical,
        &FileExportRequest::new(&historical_target, vec![directory.path().to_path_buf()])
            .expect("historical export request must be valid"),
    )
    .expect("historical source must export");
    let current_receipt = export_managed_source(
        &current,
        &FileExportRequest::new(&current_target, vec![directory.path().to_path_buf()])
            .expect("current export request must be valid"),
    )
    .expect("current source must export");

    assert_eq!(
        fs::read(&historical_target).expect("historical export must be readable"),
        historical_bytes
    );
    assert_eq!(
        fs::read(&current_target).expect("current export must be readable"),
        current_bytes
    );
    assert_eq!(historical_receipt.source_id(), &id("source-1"));
    assert_eq!(current_receipt.source_id(), &id("source-2"));
    assert_eq!(historical_receipt.version().get(), 1);
    assert_eq!(current_receipt.version().get(), 2);
    for rendered in [
        format!("{historical_receipt:?}"),
        format!("{current_receipt:?}"),
    ] {
        assert!(!rendered.contains("export-round-trip"));
        assert!(!rendered.contains("heading"));
    }
    directory.assert_no_export_temporary();

    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    assert_eq!(table_count(&raw, "radishmemory_source_artifacts"), 2);
    assert_eq!(table_count(&raw, "radishmemory_source_bodies"), 2);
    assert_eq!(table_count(&raw, "radishmemory_source_capture_audit"), 2);
}

#[test]
fn p1_f07_existing_target_is_not_overwritten_or_treated_as_success() {
    let directory = SyntheticDirectory::new("export-existing");
    fs::write(directory.file(), b"Managed source bytes.\n").expect("source file must be written");
    let synthetic = SyntheticDatabase::new("export-existing");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let capture = capture_from_file(
        &directory,
        CaptureSpec {
            source_id: "source-1",
            lineage_id: "lineage-1",
            fragment_id: "fragment-1",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
    );
    database
        .capture_source(&capture)
        .expect("source capture must commit");
    let source = database
        .load_source_artifact(&id("namespace-1"), &id("source-1"))
        .expect("source lookup must succeed")
        .expect("source must remain readable");
    let target = directory.child("protected-target.txt");
    fs::write(&target, b"Independent owner bytes.\n").expect("target must be precreated");

    let error = export_managed_source(
        &source,
        &FileExportRequest::new(&target, vec![directory.path().to_path_buf()])
            .expect("export request must be valid"),
    )
    .expect_err("existing target must reject export");

    assert_eq!(error.reason(), FileEntryErrorReason::DestinationExists);
    assert_eq!(
        fs::read(&target).expect("existing target must remain readable"),
        b"Independent owner bytes.\n"
    );
    assert_eq!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-1"))
            .expect("source lookup after failure must succeed")
            .expect("source must remain stored")
            .params()
            .content
            .as_str()
            .as_bytes(),
        b"Managed source bytes.\n"
    );
    directory.assert_no_export_temporary();
}

#[cfg(unix)]
#[test]
fn p1_f07_symlink_target_is_rejected_without_following_it() {
    use std::os::unix::fs::symlink;

    let directory = SyntheticDirectory::new("export-symlink");
    fs::write(directory.file(), b"Managed source bytes.\n").expect("source file must be written");
    let synthetic = SyntheticDatabase::new("export-symlink");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let capture = capture_from_file(
        &directory,
        CaptureSpec {
            source_id: "source-1",
            lineage_id: "lineage-1",
            fragment_id: "fragment-1",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
    );
    database
        .capture_source(&capture)
        .expect("source capture must commit");
    let source = database
        .load_source_artifact(&id("namespace-1"), &id("source-1"))
        .expect("source lookup must succeed")
        .expect("source must remain readable");
    let outside = directory.child("symlink-owned-target.txt");
    let target = directory.child("symlink-export.txt");
    fs::write(&outside, b"Independent symlink target.\n").expect("symlink target must be written");
    symlink(&outside, &target).expect("synthetic symlink must be created");

    let error = export_managed_source(
        &source,
        &FileExportRequest::new(&target, vec![directory.path().to_path_buf()])
            .expect("export request must be valid"),
    )
    .expect_err("symlink target must reject export");

    assert_eq!(error.reason(), FileEntryErrorReason::DestinationNotAllowed);
    assert_eq!(
        fs::read(&outside).expect("symlink target must remain readable"),
        b"Independent symlink target.\n"
    );
    directory.assert_no_export_temporary();
}

#[test]
fn p1_f08_rebuild_uses_managed_facts_and_rejects_missing_capture_fragment() {
    let directory = SyntheticDirectory::new("rebuild-managed");
    let historical_bytes = b"Historical rebuild marker.\r\n";
    let current_bytes = b"Current rebuild marker.\r\n";
    let synthetic = SyntheticDatabase::new("rebuild-managed");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_two_version_file_lineage(&mut database, &directory, historical_bytes, current_bytes);
    fs::remove_file(directory.file()).expect("origin file must be removed from the synthetic root");

    database
        .rebuild_recall_derivations()
        .expect("rebuild must use managed canonical facts only");
    assert_eq!(search(&database, "Current rebuild").len(), 1);
    assert!(search(&database, "Historical rebuild").is_empty());
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-1"))
            .expect("historical source lookup must succeed")
            .is_some()
    );

    let raw = Connection::open(synthetic.path()).expect("database must reopen for corruption");
    raw.execute(
        "DELETE FROM radishmemory_recall_fts
         WHERE object_kind = 'source_fragment' AND object_id = 'fragment-2'",
        [],
    )
    .expect("derived row must be removed before canonical corruption");
    raw.execute(
        "DELETE FROM radishmemory_source_fragments WHERE fragment_id = 'fragment-2'",
        [],
    )
    .expect("synthetic canonical fragment corruption must be installed");
    drop(raw);

    let error = database
        .rebuild_recall_derivations()
        .expect_err("rebuild must not invent a missing canonical fragment");
    assert_eq!(error.code(), SqliteErrorCode::InvalidStoredData);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::StoredIntegrityMismatch)
    );
}

#[test]
fn p1_f09_partial_lineage_plan_is_rejected_without_closing_any_version() {
    let directory = SyntheticDirectory::new("delete-partial-lineage");
    let synthetic = SyntheticDatabase::new("delete-partial-lineage");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_two_version_file_lineage(
        &mut database,
        &directory,
        b"Historical partial-delete marker.\n",
        b"Current partial-delete marker.\n",
    );
    let incomplete = lineage_delete_request("delete-request-partial", &["source-2"]);

    let error = database
        .store_delete_request(&incomplete)
        .expect_err("one version must not stand in for the complete lineage");

    assert_eq!(error.code(), SqliteErrorCode::DeletionInvariant);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::DeletionPlan)
    );
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-1"))
            .expect("historical lookup must succeed")
            .is_some()
    );
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-2"))
            .expect("current lookup must succeed")
            .is_some()
    );
    assert_eq!(search(&database, "Current partial-delete").len(), 1);
    let raw = Connection::open(synthetic.path()).expect("database must reopen for inspection");
    assert_eq!(table_count(&raw, "radishmemory_delete_requests"), 0);
    assert_eq!(table_count(&raw, "radishmemory_source_lineage_tips"), 1);
    assert_eq!(table_count(&raw, "radishmemory_source_origin_bindings"), 1);
    assert_eq!(table_count(&raw, "radishmemory_source_capture_audit"), 2);
}

#[test]
fn p1_f09_and_f10_lineage_purge_closes_all_versions_and_never_touches_user_files() {
    let directory = SyntheticDirectory::new("delete-lineage");
    let historical_bytes = b"Historical lineage delete marker.\r\n";
    let current_bytes = b"Current lineage delete marker.\r\n";
    let synthetic = SyntheticDatabase::new("delete-lineage");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_two_version_file_lineage(&mut database, &directory, historical_bytes, current_bytes);
    let historical = database
        .load_source_artifact(&id("namespace-1"), &id("source-1"))
        .expect("historical lookup must succeed")
        .expect("historical source must exist");
    let user_export = directory.child("user-owned-export.txt");
    export_managed_source(
        &historical,
        &FileExportRequest::new(&user_export, vec![directory.path().to_path_buf()])
            .expect("user export request must be valid"),
    )
    .expect("historical source must export before deletion");
    let request = lineage_delete_request("delete-request-lineage", &["source-1", "source-2"]);

    database
        .store_delete_request(&request)
        .expect("complete lineage plan must persist");
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-1"))
            .expect("historical lookup after plan must succeed")
            .is_none()
    );
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-2"))
            .expect("current lookup after plan must succeed")
            .is_none()
    );
    assert!(search(&database, "lineage delete").is_empty());
    database
        .rebuild_recall_derivations()
        .expect("pending lineage must remain closed during rebuild");

    let raw = Connection::open(synthetic.path()).expect("database must reopen for inspection");
    let pending: (i64, i64, i64, i64, i64, i64) = raw
        .query_row(
            "SELECT
                 (SELECT COUNT(*) FROM radishmemory_source_artifacts
                  WHERE lineage_id = 'lineage-1' AND deletion_state = 'pending'),
                 (SELECT COUNT(*) FROM radishmemory_source_fragments
                  WHERE source_id IN ('source-1', 'source-2') AND deletion_state = 'pending'),
                 (SELECT COUNT(*) FROM radishmemory_source_lineage_tips),
                 (SELECT COUNT(*) FROM radishmemory_source_origin_bindings),
                 (SELECT COUNT(*) FROM radishmemory_source_capture_audit),
                 (SELECT COUNT(*) FROM radishmemory_delete_execution_closure
                  WHERE delete_request_id = 'delete-request-lineage'
                    AND component_type = 'minimal_audit')",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("pending lineage closure must be queryable");
    assert_eq!(pending, (2, 2, 0, 1, 2, 3));
    drop(raw);
    drop(database);

    let mut database =
        SqliteDatabase::open(synthetic.path()).expect("pending deletion database must reopen");
    let results = database
        .execute_deletion(
            &id("namespace-1"),
            &id("delete-request-lineage"),
            &deletion_execution(),
        )
        .expect("lineage deletion must execute");
    assert_eq!(results.len(), 10);
    assert!(
        results
            .iter()
            .all(|result| result.params().status == ComponentStatus::Succeeded)
    );
    let evidence = deletion_evidence(&request, results);
    database
        .store_deletion_evidence(&evidence)
        .expect("completed lineage evidence must persist");
    database
        .rebuild_recall_derivations()
        .expect("deleted lineage must not revive during rebuild");
    assert!(search(&database, "lineage delete").is_empty());
    drop(database);

    assert_eq!(
        fs::read(directory.file()).expect("external origin file must remain readable"),
        current_bytes
    );
    assert_eq!(
        fs::read(&user_export).expect("user export must remain readable"),
        historical_bytes
    );

    let raw = Connection::open(synthetic.path()).expect("database must reopen for inspection");
    let purged: (i64, i64, i64, i64, i64, i64, i64) = raw
        .query_row(
            "SELECT
                 (SELECT COUNT(*) FROM radishmemory_source_artifacts
                  WHERE lineage_id = 'lineage-1' AND deletion_state = 'deleted'),
                 (SELECT COUNT(*) FROM radishmemory_source_bodies
                  WHERE source_id IN ('source-1', 'source-2')),
                 (SELECT COUNT(*) FROM radishmemory_source_fragments
                  WHERE source_id IN ('source-1', 'source-2')),
                 (SELECT COUNT(*) FROM radishmemory_source_lineage_tips),
                 (SELECT COUNT(*) FROM radishmemory_source_origin_bindings),
                 (SELECT COUNT(*) FROM radishmemory_source_capture_audit),
                 (SELECT COUNT(*) FROM radishmemory_deletion_evidence
                  WHERE deletion_evidence_id = 'deletion-evidence-lineage-1')",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .expect("purged lineage closure must be queryable");
    assert_eq!(purged, (2, 0, 0, 0, 0, 0, 1));
    drop(raw);

    let mut database =
        SqliteDatabase::open(synthetic.path()).expect("deleted lineage database must reopen");
    database
        .rebuild_recall_derivations()
        .expect("reopened deletion must remain rebuild-safe");
    assert!(search(&database, "lineage delete").is_empty());
    assert_eq!(
        database
            .load_deletion_evidence(&id("namespace-1"), &id("deletion-evidence-lineage-1"))
            .expect("lineage evidence lookup must succeed"),
        Some(evidence)
    );
}

#[test]
fn p1_f11_rejected_paths_and_non_files_leave_store_and_receipt_unchanged() {
    let allowed = SyntheticDirectory::new("reject-path-allowed");
    let outside = SyntheticDirectory::new("reject-path-outside");
    let synthetic = SyntheticDatabase::new("reject-path");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let baseline_receipt = seed_rejection_baseline(&mut database, &allowed);
    let before = source_entry_counts(synthetic.path());

    let outside_file = outside.child("outside.txt");
    fs::write(&outside_file, b"Outside root marker.\n").expect("outside file must be written");
    let outside_request = FileReadRequest::new(&outside_file, vec![allowed.path().to_path_buf()])
        .expect("outside request shape must be valid");
    rejected_capture(
        &mut database,
        &outside_request,
        FileEntryErrorReason::PathNotAllowed,
    );
    assert_eq!(source_entry_counts(synthetic.path()), before);

    let outside_name = outside
        .path()
        .file_name()
        .expect("outside test directory must have a name");
    let escaped_file = allowed
        .path()
        .join("..")
        .join(outside_name)
        .join("outside.txt");
    let escaped_request = FileReadRequest::new(&escaped_file, vec![allowed.path().to_path_buf()])
        .expect("escape request shape must be valid");
    rejected_capture(
        &mut database,
        &escaped_request,
        FileEntryErrorReason::PathNotAllowed,
    );
    assert_eq!(source_entry_counts(synthetic.path()), before);

    let directory_named_file = allowed.child("directory.txt");
    fs::create_dir(&directory_named_file).expect("directory-shaped input must be created");
    let directory_request =
        FileReadRequest::new(&directory_named_file, vec![allowed.path().to_path_buf()])
            .expect("directory request shape must be valid");
    rejected_capture(
        &mut database,
        &directory_request,
        FileEntryErrorReason::NotRegularFile,
    );
    assert_eq!(source_entry_counts(synthetic.path()), before);

    assert_eq!(
        baseline_receipt.source_id(),
        &id("source-rejection-baseline")
    );
    assert_eq!(search(&database, "rejection baseline").len(), 1);
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-must-not-exist"))
            .expect("rejected source lookup must succeed")
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn p1_f12_symlink_parent_and_leaf_are_rejected_without_store_changes() {
    use std::os::unix::fs::symlink;

    let allowed = SyntheticDirectory::new("reject-symlink-allowed");
    let outside = SyntheticDirectory::new("reject-symlink-outside");
    let synthetic = SyntheticDatabase::new("reject-symlink");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_rejection_baseline(&mut database, &allowed);
    let before = source_entry_counts(synthetic.path());

    let outside_file = outside.child("target.txt");
    fs::write(&outside_file, b"Symlink target marker.\n")
        .expect("outside symlink target must be written");
    let linked_parent = allowed.child("linked-parent");
    symlink(outside.path(), &linked_parent).expect("directory symlink must be created");
    let parent_request = FileReadRequest::new(
        linked_parent.join("target.txt"),
        vec![allowed.path().to_path_buf()],
    )
    .expect("symlink parent request shape must be valid");
    let parent_error = rejected_capture(
        &mut database,
        &parent_request,
        FileEntryErrorReason::SymlinkNotAllowed,
    );
    assert_eq!(source_entry_counts(synthetic.path()), before);

    let linked_leaf = allowed.child("linked-leaf.txt");
    symlink(&outside_file, &linked_leaf).expect("file symlink must be created");
    let leaf_request = FileReadRequest::new(&linked_leaf, vec![allowed.path().to_path_buf()])
        .expect("symlink leaf request shape must be valid");
    let leaf_error = rejected_capture(
        &mut database,
        &leaf_request,
        FileEntryErrorReason::SymlinkNotAllowed,
    );
    assert_eq!(source_entry_counts(synthetic.path()), before);

    for rendered in [
        parent_error.to_string(),
        format!("{parent_error:?}"),
        leaf_error.to_string(),
        format!("{leaf_error:?}"),
    ] {
        assert!(!rendered.contains(outside.path().to_string_lossy().as_ref()));
        assert!(!rendered.contains("target.txt"));
    }
    assert_eq!(search(&database, "rejection baseline").len(), 1);
}

#[test]
fn p1_f13_type_and_content_rejections_leave_store_and_receipt_unchanged() {
    let directory = SyntheticDirectory::new("reject-content");
    let synthetic = SyntheticDatabase::new("reject-content");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    seed_rejection_baseline(&mut database, &directory);
    let before = source_entry_counts(synthetic.path());

    let unsupported = directory.child("unsupported.markdown");
    fs::write(&unsupported, b"Unsupported extension marker.\n")
        .expect("unsupported file must be written");
    let empty = directory.child("empty.txt");
    fs::write(&empty, []).expect("empty file must be written");
    let invalid_utf8 = directory.child("invalid-utf8.txt");
    fs::write(&invalid_utf8, [0xff, 0xfe]).expect("invalid UTF-8 file must be written");
    let nul = directory.child("nul.txt");
    fs::write(&nul, b"before\0after").expect("NUL-containing file must be written");

    for (path, reason) in [
        (unsupported, FileEntryErrorReason::UnsupportedFileType),
        (empty, FileEntryErrorReason::EmptyFile),
        (invalid_utf8, FileEntryErrorReason::InvalidUtf8),
        (nul, FileEntryErrorReason::NulByteNotAllowed),
    ] {
        let request = FileReadRequest::new(&path, vec![directory.path().to_path_buf()])
            .expect("rejected content request shape must be valid");
        rejected_capture(&mut database, &request, reason);
        assert_eq!(source_entry_counts(synthetic.path()), before);
    }

    assert_eq!(search(&database, "rejection baseline").len(), 1);
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-must-not-exist"))
            .expect("rejected source lookup must succeed")
            .is_none()
    );
}

#[test]
fn p1_f14_exact_size_commits_and_one_extra_byte_leaves_it_unchanged() {
    let directory = SyntheticDirectory::new("size-capture");
    let synthetic = SyntheticDatabase::new("size-capture");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");

    let exact = directory.child("exact-boundary.txt");
    fs::write(&exact, vec![b'x'; MAX_FILE_BYTES as usize])
        .expect("exact-boundary file must be written");
    let exact_request = FileReadRequest::new(&exact, vec![directory.path().to_path_buf()])
        .expect("exact-boundary request must be valid");
    let receipt = capture_selected_file(
        &mut database,
        &exact_request,
        CaptureSpec {
            source_id: "source-exact-boundary",
            lineage_id: "lineage-exact-boundary",
            fragment_id: "fragment-exact-boundary",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
        "origin-binding-exact-boundary",
    )
    .expect("exact-boundary file must commit and return a receipt");
    assert_eq!(receipt.content_length(), MAX_FILE_BYTES);
    assert_eq!(receipt.outcome(), SourceCaptureOutcome::Created);
    let after_exact = source_entry_counts(synthetic.path());
    assert_eq!(
        after_exact,
        SourceEntryCounts {
            sources: 1,
            bodies: 1,
            fragments: 1,
            tips: 1,
            bindings: 1,
            audits: 1,
            full_text_rows: 1,
        }
    );

    let oversized = directory.child("oversized.txt");
    fs::write(&oversized, vec![b'y'; MAX_FILE_BYTES as usize + 1])
        .expect("oversized file must be written");
    let oversized_request = FileReadRequest::new(&oversized, vec![directory.path().to_path_buf()])
        .expect("oversized request shape must be valid");
    rejected_capture(
        &mut database,
        &oversized_request,
        FileEntryErrorReason::FileTooLarge,
    );
    assert_eq!(source_entry_counts(synthetic.path()), after_exact);
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-exact-boundary"))
            .expect("exact-boundary source lookup must succeed")
            .is_some()
    );
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-must-not-exist"))
            .expect("oversized source lookup must succeed")
            .is_none()
    );
}

#[test]
fn fragment_conflict_rolls_back_source_tip_index_binding_and_audit() {
    let directory = SyntheticDirectory::new("rollback");
    fs::write(directory.file(), b"Stable previous marker.\n")
        .expect("version one file must be written");
    let synthetic = SyntheticDatabase::new("atomic-rollback");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let first = capture_from_file(
        &directory,
        CaptureSpec {
            source_id: "source-1",
            lineage_id: "lineage-1",
            fragment_id: "fragment-shared",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
    );
    database
        .capture_source(&first)
        .expect("first capture must commit");

    fs::write(directory.file(), b"Uncommitted replacement marker.\n")
        .expect("version two file must be written");
    let conflicting = capture_from_file(
        &directory,
        CaptureSpec {
            source_id: "source-2",
            lineage_id: "lineage-1",
            fragment_id: "fragment-shared",
            version: 2,
            supersedes: vec![id("source-1")],
            captured_at: "2026-08-29T09:00:00Z",
        },
    );
    let error = database
        .capture_source(&conflicting)
        .expect_err("fragment conflict must roll back the complete capture");
    assert_eq!(error.code(), SqliteErrorCode::Conflict);
    assert!(
        database
            .load_source_artifact(&id("namespace-1"), &id("source-2"))
            .expect("failed source lookup must succeed")
            .is_none()
    );
    assert_eq!(search(&database, "Stable previous").len(), 1);
    assert!(search(&database, "Uncommitted replacement").is_empty());

    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    assert_eq!(table_count(&raw, "radishmemory_source_artifacts"), 1);
    assert_eq!(table_count(&raw, "radishmemory_source_fragments"), 1);
    assert_eq!(table_count(&raw, "radishmemory_source_origin_bindings"), 1);
    assert_eq!(table_count(&raw, "radishmemory_source_capture_audit"), 1);
    assert_eq!(table_count(&raw, "radishmemory_recall_fts"), 1);
}

#[test]
fn idempotent_capture_fails_closed_when_derived_recall_has_drifted() {
    let directory = SyntheticDirectory::new("drift");
    fs::write(directory.file(), b"Drift-sensitive marker.\n")
        .expect("synthetic file must be written");
    let synthetic = SyntheticDatabase::new("atomic-drift");
    let mut database = SqliteDatabase::open(synthetic.path()).expect("database must initialize");
    let first = capture_from_file(
        &directory,
        CaptureSpec {
            source_id: "source-1",
            lineage_id: "lineage-1",
            fragment_id: "fragment-1",
            version: 1,
            supersedes: vec![],
            captured_at: "2026-08-29T08:00:00Z",
        },
    );
    database
        .capture_source(&first)
        .expect("first capture must commit");
    let raw = Connection::open(synthetic.path()).expect("synthetic database must reopen");
    raw.execute("DELETE FROM radishmemory_recall_fts", [])
        .expect("synthetic derived drift must be introduced");
    drop(raw);

    let repeated = capture_from_file(
        &directory,
        CaptureSpec {
            source_id: "source-unused",
            lineage_id: "lineage-1",
            fragment_id: "fragment-unused",
            version: 2,
            supersedes: vec![id("source-1")],
            captured_at: "2026-08-29T08:05:00Z",
        },
    );
    let error = database
        .capture_source(&repeated)
        .expect_err("idempotency must not hide derived drift");
    assert_eq!(error.code(), SqliteErrorCode::InvalidStoredData);
    assert_eq!(
        error.storage_reason(),
        Some(SqliteStorageReason::DerivedDataMismatch)
    );
}
