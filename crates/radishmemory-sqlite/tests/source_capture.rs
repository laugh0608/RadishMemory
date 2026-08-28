use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use radishmemory_core::{
    DeletionState, EgressPolicy, Governance, Identifier, LocalSearch, LocalSearchHit,
    LocalSearchRequest, NonEmptyText, ProducerRef, ProducerType, RetentionMode, RetentionRule,
    Sensitivity, SourceCapture, SourceCaptureOutcome, SourceCaptureStore, SourceVault, Timestamp,
    Version,
};
use radishmemory_file_entry::{
    FileCapturePlan, FileCaptureReceipt, FileEntryErrorReason, FileExportRequest, FileReadRequest,
    build_source_capture, export_managed_source, read_file_snapshot,
};
use radishmemory_sqlite::{SqliteDatabase, SqliteErrorCode, SqliteStorageReason};
use rusqlite::{Connection, params};

mod support;

use support::SyntheticDatabase;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

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
    let snapshot = read_file_snapshot(
        &FileReadRequest::new(&file, vec![directory.path().to_path_buf()])
            .expect("read request must be valid"),
    )
    .expect("synthetic file must produce a snapshot");
    build_source_capture(
        snapshot,
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
        },
    )
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
