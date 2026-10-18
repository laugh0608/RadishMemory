mod memory_deletion;
mod write;

use super::*;
use crate::{ApplicationErrorCode, ApplicationIdentifierKind, LocalLibrary};
use radishmemory_core::{
    ActorRef, ActorType, CanonicalObjectType, ComponentStatus, DeleteRequest, DeleteRequestParams,
    EvidenceRef, EvidenceType, LocalDeletionExecution, ObjectRef, RequestedGuarantee,
    SourceCapture, Timestamp, Version, build_local_purge_targets,
};
use radishmemory_file_entry::{
    FileCapturePlan, FileCaptureReceipt, FileReadRequest, build_source_capture, read_file_snapshot,
};
use radishmemory_source_vault::{
    SourceVaultErrorCode, VaultMaintenanceError, acceptance::SyntheticLibraryProvider,
};
use rusqlite::Connection;
use std::{
    error::Error,
    fs, io,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

const NS: &str = "namespace-0123456789abcdef0123456789abcdef";
const DEVICE: &str = "device-fedcba9876543210fedcba9876543210";
const BODY: &str = "\u{feff}Synthetic original needle\r\n萝卜 e\u{301} 🥕\r\n";
const UPDATED: &str = "\u{feff}Synthetic updated needle\r\n第二版 e\u{301}\r\n";

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "radishmemory-application-vault-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[derive(Default)]
struct Runtime {
    next: usize,
    fail_clock: bool,
}
impl ApplicationRuntime for Runtime {
    type Error = io::Error;
    fn next_identifier(
        &mut self,
        kind: ApplicationIdentifierKind,
    ) -> Result<Identifier, io::Error> {
        self.next += 1;
        let prefix = match kind {
            ApplicationIdentifierKind::OriginBinding => "origin-binding",
            _ => "synthetic",
        };
        Ok(id(&format!("{prefix}-{}", self.next)))
    }
    fn now(&mut self) -> Result<Timestamp, io::Error> {
        if self.fail_clock {
            Err(io::Error::other("synthetic clock unavailable"))
        } else {
            Ok(at())
        }
    }
}
fn id(value: &str) -> Identifier {
    Identifier::new(value).unwrap()
}
fn text(value: &str) -> NonEmptyText {
    NonEmptyText::new(value).unwrap()
}
fn at() -> Timestamp {
    Timestamp::parse("2026-10-01T00:00:00Z").unwrap()
}
fn config() -> LocalLibraryConfig {
    LocalLibraryConfig::phase1_local(id(NS), id(DEVICE)).unwrap()
}
type Location = EncryptedLibraryLocation<SyntheticLibraryProvider>;
type Library = EncryptedLibrary<Runtime, SyntheticLibraryProvider>;

struct Fixture {
    root: Directory,
    provider: SyntheticLibraryProvider,
    runtime: Runtime,
    config: LocalLibraryConfig,
    first: FileCaptureReceipt,
}
impl Fixture {
    fn legacy() -> Self {
        let root = Directory::new();
        let input = root.0.join("synthetic.md");
        fs::write(&input, BODY).unwrap();
        let mut library =
            LocalLibrary::open(root.0.join("library.sqlite3"), Runtime::default(), config())
                .unwrap();
        let first = library
            .import_new_source(&FileReadRequest::new(&input, vec![root.0.clone()]).unwrap())
            .unwrap();
        let (runtime, config) = library.close();
        Self {
            root,
            provider: SyntheticLibraryProvider::default(),
            runtime,
            config,
            first,
        }
    }
    fn location(&self) -> Location {
        EncryptedLibraryLocation::with_provider(
            &self.root.0,
            self.config.clone(),
            self.provider.clone(),
        )
        .unwrap()
    }
    fn migrated() -> Self {
        let fixture = Self::legacy();
        let location = fixture.location();
        location.initialize_key().unwrap();
        assert_eq!(location.migrate_bodies().unwrap().objects_verified, 1);
        fixture
    }
    fn open(&self) -> Library {
        self.location().open(Runtime::default()).unwrap()
    }
    fn sql(&self, statement: &str) {
        Connection::open(self.root.0.join("library.sqlite3"))
            .unwrap()
            .execute_batch(statement)
            .unwrap();
    }
    fn scalar(&self, query: &str) -> i64 {
        Connection::open(self.root.0.join("library.sqlite3"))
            .unwrap()
            .query_row(query, [], |row| row.get(0))
            .unwrap()
    }
    fn objects(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut objects: Vec<_> = fs::read_dir(self.root.0.join("source-objects-v1"))
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let bytes = fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect();
        objects.sort();
        objects
    }
    fn capture(&self) -> SourceCapture {
        let input = self.root.0.join("pending.md");
        fs::write(&input, "Synthetic pending capture").unwrap();
        build_source_capture(
            read_file_snapshot(&FileReadRequest::new(&input, vec![self.root.0.clone()]).unwrap())
                .unwrap(),
            FileCapturePlan {
                namespace_id: id(NS),
                origin_binding_id: id("origin-binding-pending"),
                source_id: id("source-pending"),
                lineage_id: id("lineage-pending"),
                version: Version::new(1).unwrap(),
                supersedes_source_ids: vec![],
                fragment_id: id("fragment-pending"),
                observed_at: at(),
                captured_at: at(),
                governance: self.config.governance.clone(),
                source_producer: self.config.source_producer.clone(),
                segmenter: self.config.segmenter.clone(),
            },
        )
        .unwrap()
    }
}

fn vault_code(error: &ApplicationError) -> Option<SourceVaultErrorCode> {
    match error.vault_failure()? {
        VaultMaintenanceError::Vault(error) => Some(error.code()),
        _ => None,
    }
}
fn assert_redacted(error: &ApplicationError, root: &Directory) {
    let mut diagnostic = format!("{error:?} {error}");
    let mut cause = error.source();
    while let Some(error) = cause {
        diagnostic.push_str(&format!(" {error:?} {error}"));
        cause = error.source();
    }
    for secret in [NS, DEVICE, BODY, root.0.to_str().unwrap()] {
        assert!(!diagnostic.contains(secret));
    }
}

#[test]
fn legacy_close_migration_application_reads_and_exact_exports_preserve_versions() {
    let mut fixture = Fixture::legacy();
    let mut legacy = LocalLibrary::open(
        fixture.root.0.join("library.sqlite3"),
        fixture.runtime,
        fixture.config.clone(),
    )
    .unwrap();
    let input = fixture.root.0.join("synthetic.md");
    fs::write(&input, UPDATED).unwrap();
    let second = legacy
        .update_source(
            fixture.first.lineage_id(),
            &FileReadRequest::new(&input, vec![fixture.root.0.clone()]).unwrap(),
        )
        .unwrap();
    let expected_catalog = legacy.list_sources(0, 20).unwrap();
    let expected_versions = legacy
        .list_source_versions(fixture.first.lineage_id())
        .unwrap();
    let expected_hits = legacy
        .search_sources(text("needle"), 20, [Sensitivity::Personal])
        .unwrap();
    let expected_source = legacy.get_source(fixture.first.source_id()).unwrap();
    (fixture.runtime, fixture.config) = legacy.close();
    assert_eq!(fixture.scalar("PRAGMA user_version"), 6);
    assert!(fixture.location().open(Runtime::default()).is_err());
    assert_eq!(fixture.provider.key_loads(), 0);
    let location = fixture.location();
    location.initialize_key().unwrap();
    assert_eq!(location.migrate_bodies().unwrap().objects_verified, 2);
    assert_eq!(
        fixture.scalar("SELECT count(*) FROM radishmemory_source_bodies"),
        0
    );
    fs::remove_file(input).unwrap();
    let mut library = location.open(fixture.runtime).unwrap();
    assert_eq!(library.list_sources(0, 20).unwrap(), expected_catalog);
    assert_eq!(library.list_sources(1, 20).unwrap(), vec![]);
    assert_eq!(
        library
            .list_source_versions(fixture.first.lineage_id())
            .unwrap(),
        expected_versions
    );
    assert_eq!(
        library.get_source(fixture.first.source_id()).unwrap(),
        expected_source
    );
    assert_eq!(
        library
            .search_sources(text("needle"), 20, [Sensitivity::Personal])
            .unwrap(),
        expected_hits
    );
    assert!(
        library
            .search_sources(text("needle"), 20, [Sensitivity::Restricted])
            .unwrap()
            .is_empty()
    );
    let exports = Directory::new();
    for (name, source, body) in [
        ("old.md", fixture.first.source_id(), BODY),
        ("new.md", second.source_id(), UPDATED),
    ] {
        let target = exports.0.join(name);
        let request = FileExportRequest::new(&target, vec![exports.0.clone()]).unwrap();
        let receipt = library.export_source(source, &request).unwrap();
        assert_eq!(receipt.source_id(), source);
        assert_eq!(fs::read(&target).unwrap(), body.as_bytes());
        assert_eq!(
            library.export_source(source, &request).unwrap_err().code(),
            ApplicationErrorCode::FileEntry
        );
        assert_eq!(fs::read(&target).unwrap(), body.as_bytes());
    }
    assert_eq!(fs::read_dir(&exports.0).unwrap().count(), 2);
    let (runtime, location) = library.close();
    let library = location.open(runtime).unwrap();
    assert_eq!(
        library.get_source(fixture.first.source_id()).unwrap(),
        expected_source
    );
    assert_eq!(fixture.provider.key_creations(), 1);
    assert!(
        LocalLibrary::open(
            fixture.root.0.join("library.sqlite3"),
            Runtime::default(),
            config()
        )
        .is_err()
    );
}

#[test]
fn missing_and_unready_libraries_do_not_implicitly_initialize_or_migrate() {
    let root = Directory::new();
    let provider = SyntheticLibraryProvider::default();
    let location = || Location::with_provider(&root.0, config(), provider.clone()).unwrap();
    assert!(location().open(Runtime::default()).is_err());
    assert!(!root.0.join("library.sqlite3").exists());
    assert_eq!((provider.key_loads(), provider.key_creations()), (0, 0));
    location().initialize_key().unwrap();
    let loads = provider.key_loads();
    assert!(location().open(Runtime::default()).is_err());
    assert_eq!(provider.key_loads(), loads);
    assert_eq!(location().migrate_bodies().unwrap().objects_verified, 0);
    assert!(
        location()
            .open(Runtime::default())
            .unwrap()
            .list_sources(0, 20)
            .unwrap()
            .is_empty()
    );
    assert_eq!(provider.key_creations(), 1);
}

#[test]
fn checkpoint_recovery_never_recreates_a_missing_key() {
    let fixture = Fixture::legacy();
    fixture.location().initialize_key().unwrap();
    fixture.provider.forget_key();
    for error in [
        fixture.location().initialize_key().unwrap_err(),
        fixture.location().migrate_bodies().unwrap_err(),
    ] {
        assert_eq!(vault_code(&error), Some(SourceVaultErrorCode::KeyMissing));
        assert_redacted(&error, &fixture.root);
    }
    assert_eq!(fixture.provider.key_creations(), 1);
    assert_eq!(fixture.scalar("PRAGMA user_version"), 7);
    assert_eq!(
        fixture.scalar("SELECT count(*) FROM radishmemory_source_bodies"),
        1
    );
    assert!(fixture.objects().is_empty());
}

#[test]
fn legacy_writer_competition_blocks_preparation_and_migration_before_key_access() {
    let fixture = Fixture::legacy();
    let competing = Connection::open(fixture.root.0.join("library.sqlite3")).unwrap();
    competing.execute_batch("BEGIN IMMEDIATE").unwrap();
    let error = fixture.location().initialize_key().unwrap_err();
    assert_eq!(
        error.operation(),
        ApplicationOperation::InitializeLibraryKey
    );
    assert!(matches!(
        error.vault_failure(),
        Some(VaultMaintenanceError::Database { .. })
    ));
    assert_eq!(
        (
            fixture.provider.key_loads(),
            fixture.provider.key_creations()
        ),
        (0, 0)
    );
    competing.execute_batch("ROLLBACK").unwrap();
    assert_eq!(fixture.scalar("PRAGMA user_version"), 6);
    fixture.location().initialize_key().unwrap();
    let loads = fixture.provider.key_loads();
    competing.execute_batch("BEGIN IMMEDIATE").unwrap();
    let error = fixture.location().migrate_bodies().unwrap_err();
    assert_eq!(
        error.operation(),
        ApplicationOperation::MigrateLibraryBodies
    );
    assert_eq!(fixture.provider.key_loads(), loads);
    competing.execute_batch("ROLLBACK").unwrap();
    // Close every old connection before handing over to the exclusive session.
    drop(competing);
    assert_eq!(
        fixture
            .location()
            .migrate_bodies()
            .unwrap()
            .objects_verified,
        1
    );
    assert_eq!(fixture.open().list_sources(0, 20).unwrap().len(), 1);
}

#[test]
fn interrupted_migration_requires_explicit_resume_and_reuses_published_objects() {
    let fixture = Fixture::legacy();
    fixture.location().initialize_key().unwrap();
    fixture.provider.interrupt_next_migration_after_publish();
    assert!(fixture.location().migrate_bodies().is_err());
    let before = fixture.objects();
    assert_eq!(before.len(), 1);
    assert!(fixture.location().open(Runtime::default()).is_err());
    assert_eq!(
        fixture.scalar("SELECT count(*) FROM radishmemory_source_bodies"),
        1
    );
    assert_eq!(fixture.objects(), before);
    assert_eq!(
        fixture
            .location()
            .migrate_bodies()
            .unwrap()
            .objects_verified,
        1
    );
    assert_eq!(fixture.objects(), before);
    assert_eq!(
        fixture
            .open()
            .get_source(fixture.first.source_id())
            .unwrap()
            .unwrap()
            .params()
            .content
            .as_str(),
        BODY
    );
    assert_eq!(fixture.provider.key_creations(), 1);
}

#[test]
fn every_operation_reloads_the_existing_key_and_preserves_bounded_failure_causes() {
    let fixture = Fixture::migrated();
    let mut library = fixture.open();
    let exports = Directory::new();
    let request =
        FileExportRequest::new(exports.0.join("missing.md"), vec![exports.0.clone()]).unwrap();
    for code in [
        SourceVaultErrorCode::KeyMissing,
        SourceVaultErrorCode::KeyStoreLocked,
        SourceVaultErrorCode::KeyStoreDenied,
        SourceVaultErrorCode::KeyStoreCancelled,
    ] {
        fixture.provider.fail_key_access(Some(code));
        for error in [
            fixture.location().open(Runtime::default()).unwrap_err(),
            library.list_sources(0, 20).unwrap_err(),
            library.get_source(fixture.first.source_id()).unwrap_err(),
            library
                .search_sources(text("needle"), 10, [Sensitivity::Personal])
                .unwrap_err(),
            library
                .export_source(fixture.first.source_id(), &request)
                .unwrap_err(),
        ] {
            assert_eq!(vault_code(&error), Some(code));
            assert_eq!(error.code(), ApplicationErrorCode::SourceVault);
            assert_redacted(&error, &fixture.root);
        }
    }
    fixture.provider.fail_key_access(None);
    fixture.provider.use_wrong_key(true);
    assert_eq!(
        vault_code(&library.list_sources(0, 20).unwrap_err()),
        Some(SourceVaultErrorCode::AuthenticationFailed)
    );
    fixture.provider.use_wrong_key(false);
    let loads = fixture.provider.key_loads();
    assert_eq!(library.list_sources(0, 20).unwrap().len(), 1);
    assert_eq!(fixture.provider.key_loads(), loads + 1);
    assert_eq!(fixture.provider.key_creations(), 1);
    assert_eq!(fs::read_dir(&exports.0).unwrap().count(), 0);
    probe(&fixture.root, "writable");
}

#[test]
fn profile_mismatch_is_rejected_before_loading_a_key() {
    let fixture = Fixture::migrated();
    let loads = fixture.provider.key_loads();
    for (namespace, device) in [
        ("namespace-11111111111111111111111111111111", DEVICE),
        (NS, "device-11111111111111111111111111111111"),
    ] {
        let config = LocalLibraryConfig::phase1_local(id(namespace), id(device)).unwrap();
        let error = Location::with_provider(&fixture.root.0, config, fixture.provider.clone())
            .unwrap()
            .open(Runtime::default())
            .unwrap_err();
        assert!(matches!(
            error.vault_failure(),
            Some(VaultMaintenanceError::Database { .. })
        ));
        assert_eq!(fixture.provider.key_loads(), loads);
    }
}

#[test]
fn object_damage_between_operations_blocks_all_reads_and_export_without_origin_fallback() {
    for damage in ["missing", "corrupt", "unknown"] {
        let fixture = Fixture::migrated();
        let mut library = fixture.open();
        let object = fixture.objects().remove(0).0;
        match damage {
            "missing" => fs::rename(object, fixture.root.0.join("held-object")).unwrap(),
            "corrupt" => fs::write(object, b"synthetic damaged ciphertext").unwrap(),
            "unknown" => fs::write(
                fixture.root.0.join("source-objects-v1/unknown.rmo"),
                b"synthetic unknown object",
            )
            .unwrap(),
            _ => unreachable!(),
        }
        let exports = Directory::new();
        let request =
            FileExportRequest::new(exports.0.join("blocked.md"), vec![exports.0.clone()]).unwrap();
        for error in [
            fixture.location().open(Runtime::default()).unwrap_err(),
            library.get_source(fixture.first.source_id()).unwrap_err(),
            library.list_sources(0, 20).unwrap_err(),
            library
                .search_sources(text("needle"), 20, [Sensitivity::Personal])
                .unwrap_err(),
            library
                .export_source(fixture.first.source_id(), &request)
                .unwrap_err(),
        ] {
            assert_redacted(&error, &fixture.root);
        }
        assert_eq!(fs::read_dir(&exports.0).unwrap().count(), 0);
        assert_eq!(
            fs::read(fixture.root.0.join("synthetic.md")).unwrap(),
            BODY.as_bytes()
        );
        probe(&fixture.root, "writable");
    }
}

#[test]
fn derived_drift_requires_explicit_maintenance_and_is_not_repaired_by_open_or_search() {
    let fixture = Fixture::migrated();
    let mut library = fixture.open();
    fixture.sql("DELETE FROM radishmemory_recall_fts");
    assert!(fixture.location().open(Runtime::default()).is_err());
    assert!(
        library
            .search_sources(text("needle"), 20, [Sensitivity::Personal])
            .is_err()
    );
    assert_eq!(
        fixture.scalar("SELECT count(*) FROM radishmemory_recall_fts"),
        0
    );
    fixture
        .provider
        .rebuild(&library.location.directory, NS, DEVICE)
        .unwrap();
    assert_eq!(
        library
            .search_sources(text("needle"), 20, [Sensitivity::Personal])
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn idle_application_hands_off_to_capture_and_preserves_pending_exact_request() {
    let fixture = Fixture::migrated();
    let mut library = fixture.open();
    let request = fixture.capture();
    fixture.provider.interrupt_next_capture_after_publish();
    assert!(
        fixture
            .provider
            .capture(&library.location.directory, NS, DEVICE, &request)
            .is_err()
    );
    let before = fixture.objects();
    assert_eq!(library.list_sources(0, 20).unwrap().len(), 1);
    assert!(library.get_source(&id("source-pending")).unwrap().is_none());
    assert!(
        library
            .search_sources(text("pending"), 20, [Sensitivity::Personal])
            .unwrap()
            .is_empty()
    );
    assert_eq!(fixture.open().list_sources(0, 20).unwrap().len(), 1);
    assert_eq!(fixture.objects(), before);
    assert_eq!(
        fixture.scalar(
            "SELECT count(*) FROM radishmemory_source_vault_attempts WHERE state = 'prepared'"
        ),
        1
    );
    fixture
        .provider
        .capture(&library.location.directory, NS, DEVICE, &request)
        .unwrap();
    assert_eq!(library.list_sources(0, 20).unwrap().len(), 2);
    assert_eq!(
        library
            .search_sources(text("pending"), 20, [Sensitivity::Personal])
            .unwrap()
            .len(),
        1
    );
    assert_eq!(fixture.provider.key_creations(), 1);
}

#[test]
fn external_deletion_is_visible_to_next_operation_and_rebuild_does_not_resurrect_it() {
    let fixture = Fixture::migrated();
    let mut library = fixture.open();
    let exports = Directory::new();
    let export = exports.0.join("retained.md");
    library
        .export_source(
            fixture.first.source_id(),
            &FileExportRequest::new(&export, vec![exports.0.clone()]).unwrap(),
        )
        .unwrap();
    let targets = vec![ObjectRef::new(
        CanonicalObjectType::SourceArtifact,
        fixture.first.source_id().clone(),
    )];
    let request = DeleteRequest::new(DeleteRequestParams {
        delete_request_id: id("delete-synthetic"),
        namespace_id: id(NS),
        requested_by: ActorRef::new(ActorType::TestFixture, id("actor-synthetic"), None),
        authorization_basis: text("explicit-synthetic-deletion"),
        requested_guarantee: RequestedGuarantee::LocalPurge,
        device_id: id(DEVICE),
        planned_components: build_local_purge_targets(&targets).unwrap(),
        target_refs: targets,
        reason_code: text("synthetic-local-purge"),
        requested_at: at(),
    })
    .unwrap();
    let execution = LocalDeletionExecution::new(
        at(),
        EvidenceRef::new(EvidenceType::PolicyBasis, id("policy-synthetic")),
    )
    .unwrap();
    let results = fixture
        .provider
        .execute_deletion(&library.location.directory, &request, &execution)
        .unwrap();
    assert_eq!(results.len(), 10);
    assert!(
        results
            .iter()
            .all(|result| result.params().status == ComponentStatus::Succeeded)
    );
    for rebuild in [false, true] {
        if rebuild {
            fixture
                .provider
                .rebuild(&library.location.directory, NS, DEVICE)
                .unwrap();
        }
        assert!(library.list_sources(0, 20).unwrap().is_empty());
        assert!(
            library
                .list_source_versions(fixture.first.lineage_id())
                .unwrap()
                .is_empty()
        );
        assert!(
            library
                .get_source(fixture.first.source_id())
                .unwrap()
                .is_none()
        );
        assert!(
            library
                .search_sources(text("needle"), 20, [Sensitivity::Personal])
                .unwrap()
                .is_empty()
        );
        let request =
            FileExportRequest::new(exports.0.join("blocked.md"), vec![exports.0.clone()]).unwrap();
        assert_eq!(
            library
                .export_source(fixture.first.source_id(), &request)
                .unwrap_err()
                .code(),
            ApplicationErrorCode::NotFound
        );
    }
    assert!(fixture.objects().is_empty());
    assert_eq!(fs::read(export).unwrap(), BODY.as_bytes());
}

fn probe(root: &Directory, mode: &str) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "encrypted::tests::independent_process_probe",
            "--ignored",
        ])
        .env("RADISHMEMORY_APPLICATION_PROBE_ROOT", &root.0)
        .env("RADISHMEMORY_APPLICATION_PROBE_MODE", mode)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

#[test]
#[ignore = "invoked only by parent against its isolated synthetic library"]
fn independent_process_probe() {
    let Some(root) = std::env::var_os("RADISHMEMORY_APPLICATION_PROBE_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let mode = std::env::var("RADISHMEMORY_APPLICATION_PROBE_MODE").unwrap();
    if matches!(mode.as_str(), "locked" | "writable") {
        let connection = Connection::open(root.join("library.sqlite3")).unwrap();
        connection.busy_timeout(std::time::Duration::ZERO).unwrap();
        let result = connection.execute_batch("BEGIN IMMEDIATE");
        if mode == "locked" {
            assert!(matches!(
                result.unwrap_err().sqlite_error_code(),
                Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
            ));
        } else {
            result.unwrap();
            connection.execute_batch("ROLLBACK").unwrap();
        }
    } else {
        let provider = SyntheticLibraryProvider::existing(NS, DEVICE).unwrap();
        let result = Location::with_provider(&root, config(), provider.clone())
            .unwrap()
            .open(Runtime::default());
        if mode == "reader-blocked" {
            assert!(result.is_err());
            assert_eq!(provider.key_loads(), 0);
        } else {
            assert_eq!(mode, "reader-ready");
            assert_eq!(result.unwrap().list_sources(0, 20).unwrap().len(), 1);
        }
    }
}

#[test]
fn scoped_sessions_exclude_other_processes_through_export_and_release_on_errors_and_unwind() {
    let fixture = Fixture::migrated();
    let mut library = fixture.open();
    probe(&fixture.root, "writable");
    let exports = Directory::new();
    let target = exports.0.join("locked-export.md");
    let request = FileExportRequest::new(&target, vec![exports.0.clone()]).unwrap();
    library
        .with_reader(ApplicationOperation::ExportSource, |reader| {
            probe(&fixture.root, "locked");
            probe(&fixture.root, "reader-blocked");
            let receipt = read::export_source(
                reader,
                &library.location.config,
                fixture.first.source_id(),
                &request,
            )?;
            assert_eq!(fs::read(&target).unwrap(), BODY.as_bytes());
            probe(&fixture.root, "locked");
            Ok(receipt)
        })
        .unwrap();
    probe(&fixture.root, "reader-ready");
    assert!(
        library
            .export_source(fixture.first.source_id(), &request)
            .is_err()
    );
    probe(&fixture.root, "writable");
    assert!(library.list_sources(0, 0).is_err());
    probe(&fixture.root, "writable");
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), ApplicationError> = library
            .with_reader(ApplicationOperation::GetSource, |_| {
                panic!("synthetic interrupted use case")
            });
    }));
    assert!(panic.is_err());
    probe(&fixture.root, "writable");
    library.runtime.fail_clock = true;
    let loads = fixture.provider.key_loads();
    assert_eq!(
        library
            .search_sources(text("needle"), 20, [Sensitivity::Personal])
            .unwrap_err()
            .code(),
        ApplicationErrorCode::Runtime
    );
    assert_eq!(fixture.provider.key_loads(), loads);
    probe(&fixture.root, "writable");
}
