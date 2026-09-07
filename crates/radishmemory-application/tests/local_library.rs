use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use radishmemory_application::{
    ApplicationErrorReason, ApplicationIdentifierKind, ApplicationRuntime, LocalDeletionConfig,
    LocalLibrary, LocalLibraryConfig,
};
use radishmemory_core::{
    ActorRef, ActorType, DeletionOverallStatus, DeletionState, EgressPolicy, Governance,
    Identifier, NonEmptyText, ProducerRef, ProducerType, RetentionMode, RetentionRule, Sensitivity,
    Timestamp,
};
use radishmemory_file_entry::{FileCaptureOutcome, FileExportRequest, FileReadRequest};
use radishmemory_sqlite as _;

static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let sequence = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "radishmemory-application-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("synthetic application directory must be created");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Debug)]
struct TestRuntimeError;

impl fmt::Display for TestRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("synthetic runtime failure")
    }
}

impl Error for TestRuntimeError {}

#[derive(Default)]
struct TestRuntime {
    next_id: u64,
    next_time: u8,
}

impl ApplicationRuntime for TestRuntime {
    type Error = TestRuntimeError;

    fn next_identifier(
        &mut self,
        kind: ApplicationIdentifierKind,
    ) -> Result<Identifier, Self::Error> {
        self.next_id += 1;
        let prefix = match kind {
            ApplicationIdentifierKind::Namespace => "namespace-host",
            ApplicationIdentifierKind::Device => "device-host",
            ApplicationIdentifierKind::OriginBinding => "origin-binding-host",
            ApplicationIdentifierKind::Source => "source-host",
            ApplicationIdentifierKind::Lineage => "lineage-host",
            ApplicationIdentifierKind::Fragment => "fragment-host",
            ApplicationIdentifierKind::DeleteRequest => "delete-request-host",
            ApplicationIdentifierKind::DeletionEvidence => "deletion-evidence-host",
        };
        Ok(id(&format!("{prefix}-{}", self.next_id)))
    }

    fn now(&mut self) -> Result<Timestamp, Self::Error> {
        self.next_time += 1;
        Ok(timestamp(&format!(
            "2026-08-30T10:00:{:02}Z",
            self.next_time
        )))
    }
}

struct InvalidBindingRuntime(TestRuntime);

impl ApplicationRuntime for InvalidBindingRuntime {
    type Error = TestRuntimeError;

    fn next_identifier(
        &mut self,
        kind: ApplicationIdentifierKind,
    ) -> Result<Identifier, Self::Error> {
        if kind == ApplicationIdentifierKind::OriginBinding {
            return Ok(id("invalid-binding"));
        }
        self.0.next_identifier(kind)
    }

    fn now(&mut self) -> Result<Timestamp, Self::Error> {
        self.0.now()
    }
}

fn id(value: &str) -> Identifier {
    Identifier::new(value.to_owned()).expect("synthetic identifier must be valid")
}

fn text(value: &str) -> NonEmptyText {
    NonEmptyText::new(value.to_owned()).expect("synthetic text must be valid")
}

fn timestamp(value: &str) -> Timestamp {
    Timestamp::parse(value).expect("synthetic timestamp must be valid")
}

fn config() -> LocalLibraryConfig {
    let governance = Governance::new(
        Sensitivity::Personal,
        EgressPolicy::LocalOnly,
        RetentionRule::new(RetentionMode::UntilDeleted, None, None).unwrap(),
        DeletionState::Active,
        id("policy-host-local"),
    )
    .unwrap();
    LocalLibraryConfig::new(
        id("namespace-host"),
        governance,
        ProducerRef::new(ProducerType::System, id("producer-host"), text("1.0.0")),
        ProducerRef::new(
            ProducerType::Rule,
            id("segmenter-whole-file"),
            text("1.0.0"),
        ),
        LocalDeletionConfig::new(
            ActorRef::new(ActorType::User, id("user-local"), None),
            text("explicit-user-lineage-deletion"),
            id("device-local"),
            text("user-requested-local-purge"),
            id("policy-local-deletion"),
            ProducerRef::new(ProducerType::System, id("deletion-verifier"), text("1.0.0")),
        ),
    )
    .unwrap()
}

fn read_request(path: &Path, allowed_root: &Path) -> FileReadRequest {
    FileReadRequest::new(path, vec![allowed_root.to_path_buf()]).unwrap()
}

#[test]
fn p1_hf01_hf02_hf03_import_search_export_and_reopen_file_database() {
    let directory = TestDirectory::new("import-reopen");
    let input_root = directory.path().join("input");
    let export_root = directory.path().join("export");
    fs::create_dir(&input_root).unwrap();
    fs::create_dir(&export_root).unwrap();
    let input = input_root.join("synthetic-note.md");
    let body = b"# Synthetic\r\nlocal library citation\n";
    fs::write(&input, body).unwrap();
    let database = directory.path().join("library.sqlite");

    let mut library = LocalLibrary::open(&database, TestRuntime::default(), config()).unwrap();
    assert!(library.list_sources(0, 20).unwrap().is_empty());
    let receipt = library
        .import_new_source(&read_request(&input, &input_root))
        .unwrap();
    assert_eq!(receipt.outcome(), FileCaptureOutcome::Created);
    assert_eq!(receipt.version().get(), 1);

    let catalog = library.list_sources(0, 20).unwrap();
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].current_source_id(), receipt.source_id());
    assert_eq!(catalog[0].version_count(), 1);
    let hits = library
        .search_sources(text("citation"), 5, [Sensitivity::Personal])
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].source_id(), receipt.source_id());
    assert_eq!(hits[0].byte_start(), 0);
    assert_eq!(usize::try_from(hits[0].byte_end()).unwrap(), body.len());
    assert_eq!(hits[0].content().as_str().as_bytes(), body);

    let first_export = export_root.join("first.md");
    library
        .export_source(
            receipt.source_id(),
            &FileExportRequest::new(&first_export, vec![export_root.clone()]).unwrap(),
        )
        .unwrap();
    assert_eq!(fs::read(&first_export).unwrap(), body);
    let source_id = receipt.source_id().clone();
    drop(library);
    fs::remove_file(&input).unwrap();

    let mut reopened = LocalLibrary::open(
        &database,
        TestRuntime {
            next_time: 20,
            ..TestRuntime::default()
        },
        config(),
    )
    .unwrap();
    assert_eq!(reopened.list_sources(0, 20).unwrap().len(), 1);
    assert!(reopened.get_source(&source_id).unwrap().is_some());
    assert_eq!(
        reopened
            .search_sources(text("library"), 5, [Sensitivity::Personal])
            .unwrap()
            .len(),
        1
    );
    let reopened_export = export_root.join("reopened.md");
    reopened
        .export_source(
            &source_id,
            &FileExportRequest::new(&reopened_export, vec![export_root]).unwrap(),
        )
        .unwrap();
    assert_eq!(fs::read(reopened_export).unwrap(), body);
}

#[test]
fn p1_hf04_hf05_explicit_update_is_idempotent_then_versions_changed_bytes() {
    let directory = TestDirectory::new("update");
    let input_root = directory.path().join("input");
    fs::create_dir(&input_root).unwrap();
    let input = input_root.join("versioned.txt");
    fs::write(&input, b"synthetic version one").unwrap();
    let mut library = LocalLibrary::open(
        directory.path().join("library.sqlite"),
        TestRuntime::default(),
        config(),
    )
    .unwrap();

    let created = library
        .import_new_source(&read_request(&input, &input_root))
        .unwrap();
    let idempotent = library
        .update_source(created.lineage_id(), &read_request(&input, &input_root))
        .unwrap();
    assert_eq!(idempotent.outcome(), FileCaptureOutcome::Idempotent);
    assert_eq!(idempotent.source_id(), created.source_id());

    fs::write(&input, b"synthetic version two searchable").unwrap();
    let versioned = library
        .update_source(created.lineage_id(), &read_request(&input, &input_root))
        .unwrap();
    assert_eq!(versioned.outcome(), FileCaptureOutcome::Versioned);
    assert_eq!(versioned.version().get(), 2);
    assert_ne!(versioned.source_id(), created.source_id());

    let catalog = library.list_sources(0, 1).unwrap();
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].current_source_id(), versioned.source_id());
    assert_eq!(catalog[0].version_count(), 2);
    assert!(library.list_sources(1, 1).unwrap().is_empty());
    let versions = library.list_source_versions(created.lineage_id()).unwrap();
    assert_eq!(versions.len(), 2);
    assert!(!versions[0].current());
    assert!(versions[1].current());
    assert_eq!(versions[0].source_id(), created.source_id());
    assert_eq!(versions[1].source_id(), versioned.source_id());

    let hits = library
        .search_sources(text("searchable"), 5, [Sensitivity::Personal])
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].source_id(), versioned.source_id());
}

#[test]
fn p1_hf08_lineage_delete_closes_all_versions_and_persists_completed_evidence() {
    let directory = TestDirectory::new("delete");
    let input_root = directory.path().join("input");
    let export_root = directory.path().join("export");
    fs::create_dir(&input_root).unwrap();
    fs::create_dir(&export_root).unwrap();
    let input = input_root.join("delete-me.md");
    fs::write(&input, b"synthetic deletion version one").unwrap();
    let database = directory.path().join("library.sqlite");
    let mut library = LocalLibrary::open(&database, TestRuntime::default(), config()).unwrap();

    let first = library
        .import_new_source(&read_request(&input, &input_root))
        .unwrap();
    fs::write(&input, b"synthetic deletion version two").unwrap();
    let second = library
        .update_source(first.lineage_id(), &read_request(&input, &input_root))
        .unwrap();
    let evidence = library.delete_source_lineage(first.lineage_id()).unwrap();
    assert_eq!(
        evidence.params().overall_status,
        DeletionOverallStatus::Completed
    );
    assert_eq!(evidence.params().component_results.len(), 10);
    assert!(library.list_sources(0, 20).unwrap().is_empty());
    assert!(
        library
            .list_source_versions(first.lineage_id())
            .unwrap()
            .is_empty()
    );
    assert!(library.get_source(first.source_id()).unwrap().is_none());
    assert!(library.get_source(second.source_id()).unwrap().is_none());
    assert!(
        library
            .search_sources(text("deletion"), 5, [Sensitivity::Personal])
            .unwrap()
            .is_empty()
    );
    let export_error = library
        .export_source(
            second.source_id(),
            &FileExportRequest::new(export_root.join("must-not-exist.md"), vec![export_root])
                .unwrap(),
        )
        .unwrap_err();
    assert_eq!(
        export_error.reason(),
        ApplicationErrorReason::SourceNotFound
    );
    assert_eq!(fs::read(&input).unwrap(), b"synthetic deletion version two");
    library.verify_library().unwrap();

    let evidence_id = evidence.params().deletion_evidence_id.clone();
    drop(library);
    let reopened = LocalLibrary::open(
        &database,
        TestRuntime {
            next_time: 30,
            ..TestRuntime::default()
        },
        config(),
    )
    .unwrap();
    assert!(reopened.list_sources(0, 20).unwrap().is_empty());
    assert_eq!(
        reopened
            .get_deletion_evidence(&evidence_id)
            .unwrap()
            .unwrap(),
        evidence
    );
}

#[test]
fn p1_hf09_hf12_invalid_runtime_and_rejected_file_leave_no_catalog_or_sensitive_debug() {
    let directory = TestDirectory::new("failure-redaction");
    let input_root = directory.path().join("input");
    fs::create_dir(&input_root).unwrap();
    let input = input_root.join("private-looking.txt");
    let marker = "synthetic-secret-marker";
    fs::write(&input, marker).unwrap();
    let database = directory.path().join("library.sqlite");
    let mut library = LocalLibrary::open(
        &database,
        InvalidBindingRuntime(TestRuntime::default()),
        config(),
    )
    .unwrap();

    let error = library
        .import_new_source(&read_request(&input, &input_root))
        .unwrap_err();
    assert_eq!(
        error.reason(),
        ApplicationErrorReason::InvalidRuntimeIdentifier
    );
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains(marker));
    assert!(!rendered.contains(input.to_string_lossy().as_ref()));
    assert!(!rendered.contains(input_root.to_string_lossy().as_ref()));
    assert!(!rendered.contains(database.to_string_lossy().as_ref()));
    assert!(library.list_sources(0, 20).unwrap().is_empty());

    let missing = id("lineage-does-not-exist");
    let error = library
        .update_source(&missing, &read_request(&input, &input_root))
        .unwrap_err();
    assert_eq!(error.reason(), ApplicationErrorReason::LineageNotFound);
    assert!(library.list_sources(0, 20).unwrap().is_empty());
}
