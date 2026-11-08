use super::*;
use radishmemory_application::{ApplicationError, ApplicationOperation, RecallProfile};
use radishmemory_sqlite::{SqliteError, SqliteStorageReason};
use rusqlite::{Connection, ErrorCode};

fn adapter_error(error: &ApplicationError) -> &SqliteError {
    error.source().unwrap().downcast_ref().unwrap()
}

fn create_library(root: &Path) -> (PathBuf, LocalLibrary<TestRuntime>, FileReadRequest) {
    let input = root.join("synthetic.md");
    fs::write(&input, b"\xef\xbb\xbfsynthetic original\r\n").unwrap();
    let path = root.join("library.sqlite3");
    let library = LocalLibrary::open(&path, TestRuntime::default(), config()).unwrap();
    (path, library, read_request(&input, root))
}

fn remove_derivations(path: &Path) {
    Connection::open(path)
        .unwrap()
        .execute_batch(
            "DELETE FROM radishmemory_recall_fts;
             DELETE FROM radishmemory_source_lineage_tips;",
        )
        .unwrap();
}

#[test]
fn repairs_both_profiles_and_preserves_history_and_completed_deletion() {
    for profile in [RecallProfile::HanPhraseV1, RecallProfile::LegacyUnicode61] {
        let directory = TestDirectory::new("recall-repair");
        let (path, mut library, request) = create_library(directory.path());
        let first = library.import_new_source(&request).unwrap();
        let revised = directory.path().join("revised.md");
        let revised_bytes = b"\xef\xbb\xbfcurrent recallfixture\r\n";
        fs::write(&revised, revised_bytes).unwrap();
        let second = library
            .update_source(
                first.lineage_id(),
                &read_request(&revised, directory.path()),
            )
            .unwrap();
        let discarded = library.import_new_source(&request).unwrap();
        let deleted = library
            .delete_source_lineage(discarded.lineage_id())
            .unwrap();
        assert_eq!(
            deleted.params().overall_status,
            DeletionOverallStatus::Completed
        );
        library.change_recall_profile(profile).unwrap();
        let original = library.get_source(first.source_id()).unwrap().unwrap();
        let latest = library.get_source(second.source_id()).unwrap().unwrap();
        drop(library);
        remove_derivations(&path);

        let error = LocalLibrary::open(&path, TestRuntime::default(), config()).unwrap_err();
        assert!(error.is_recall_open_mismatch());
        let damaged = fs::read(&path).unwrap();
        LocalLibrary::<TestRuntime>::check_recall_repair(&path, &config()).unwrap_or_else(
            |error| {
                let adapter = adapter_error(&error);
                panic!(
                    "{adapter:?} / {:?}",
                    adapter
                        .source()
                        .and_then(|source| source.downcast_ref::<rusqlite::Error>())
                        .and_then(rusqlite::Error::sqlite_error_code)
                )
            },
        );
        assert_eq!(fs::read(&path).unwrap(), damaged);

        let library = LocalLibrary::repair_recall(&path, TestRuntime::default(), config()).unwrap();
        assert_eq!(library.recall_profile().unwrap(), profile);
        assert_eq!(
            library.get_source(first.source_id()).unwrap(),
            Some(original)
        );
        assert_eq!(
            library.get_source(second.source_id()).unwrap(),
            Some(latest)
        );
        assert!(library.get_source(discarded.source_id()).unwrap().is_none());
        assert_eq!(
            library
                .get_deletion_evidence(&deleted.params().deletion_evidence_id)
                .unwrap(),
            Some(deleted)
        );
        drop(library);

        let mut reopened = LocalLibrary::open(
            &path,
            TestRuntime {
                next_time: 20,
                ..TestRuntime::default()
            },
            config(),
        )
        .unwrap();
        reopened.verify_library().unwrap();
        assert!(
            reopened
                .search_sources(text("original"), 10, [Sensitivity::Personal])
                .unwrap()
                .is_empty()
        );
        let results = reopened
            .search_sources(text("recallfixture"), 10, [Sensitivity::Personal])
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_id(), second.source_id());
        assert_eq!(results[0].content().as_str().as_bytes(), revised_bytes);
    }
}

#[test]
fn rechecks_managed_body_and_binding_changes_before_rebuilding() {
    for (sql, expected) in [
        (
            "UPDATE radishmemory_source_bodies SET content = X'616c7465726564';",
            SqliteStorageReason::StoredIntegrityMismatch,
        ),
        (
            "UPDATE radishmemory_source_origin_bindings SET lineage_id = 'different-lineage';",
            SqliteStorageReason::OriginBindingMismatch,
        ),
    ] {
        let directory = TestDirectory::new("recall-repair-refused");
        let (path, mut library, request) = create_library(directory.path());
        library.import_new_source(&request).unwrap();
        drop(library);
        remove_derivations(&path);
        LocalLibrary::<TestRuntime>::check_recall_repair(&path, &config()).unwrap();
        Connection::open(&path).unwrap().execute_batch(sql).unwrap();
        let damaged = fs::read(&path).unwrap();

        let error =
            LocalLibrary::repair_recall(&path, TestRuntime::default(), config()).unwrap_err();
        assert_eq!(error.operation(), ApplicationOperation::RebuildRecall);
        assert_eq!(adapter_error(&error).storage_reason(), Some(expected));
        let error = LocalLibrary::<TestRuntime>::check_recall_repair(&path, &config()).unwrap_err();
        assert_eq!(error.operation(), ApplicationOperation::CheckRecallRepair);
        assert_eq!(adapter_error(&error).storage_reason(), Some(expected));
        assert_eq!(fs::read(&path).unwrap(), damaged);
        assert!(LocalLibrary::open(&path, TestRuntime::default(), config()).is_err());
    }
}

#[test]
fn reader_blocking_commit_keeps_damaged_library_closed_until_retry() {
    let directory = TestDirectory::new("recall-repair-rollback");
    let (path, mut library, request) = create_library(directory.path());
    library.import_new_source(&request).unwrap();
    drop(library);
    remove_derivations(&path);
    LocalLibrary::<TestRuntime>::check_recall_repair(&path, &config()).unwrap();
    let damaged = fs::read(&path).unwrap();

    let reader = Connection::open(&path).unwrap();
    reader.execute_batch("BEGIN").unwrap();
    let rows: i64 = reader
        .query_row("SELECT count(*) FROM radishmemory_recall_fts", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(rows, 0);
    let error = LocalLibrary::repair_recall(&path, TestRuntime::default(), config()).unwrap_err();
    let error = adapter_error(&error).source().unwrap();
    assert!(matches!(
        error.downcast_ref::<rusqlite::Error>(),
        Some(rusqlite::Error::SqliteFailure(error, _)) if error.code == ErrorCode::DatabaseBusy
    ));
    drop(reader);
    assert_eq!(fs::read(&path).unwrap(), damaged);
    assert!(LocalLibrary::open(&path, TestRuntime::default(), config()).is_err());
    LocalLibrary::repair_recall(&path, TestRuntime::default(), config()).unwrap();
}

#[test]
fn repair_never_creates_a_database_or_accepts_v7() {
    let directory = TestDirectory::new("recall-repair-version");
    let missing = directory.path().join("absent.sqlite3");
    assert!(LocalLibrary::<TestRuntime>::check_recall_repair(&missing, &config()).is_err());
    assert!(LocalLibrary::repair_recall(&missing, TestRuntime::default(), config()).is_err());
    assert!(!missing.exists());

    let (path, library, _) = create_library(directory.path());
    drop(library);
    let mut database =
        radishmemory_sqlite::SourceVaultKeyDatabase::open(path.canonicalize().unwrap()).unwrap();
    database
        .begin_key_initialization("namespace-host", "device-local", "synthetic-provider")
        .unwrap()
        .commit()
        .unwrap();
    drop(database);
    let checkpoint = fs::read(&path).unwrap();
    for error in [
        LocalLibrary::<TestRuntime>::check_recall_repair(&path, &config()).unwrap_err(),
        LocalLibrary::repair_recall(&path, TestRuntime::default(), config()).unwrap_err(),
    ] {
        assert_eq!(
            adapter_error(&error).code(),
            radishmemory_sqlite::SqliteErrorCode::UnsupportedSchemaVersion
        );
    }
    assert_eq!(fs::read(&path).unwrap(), checkpoint);
}
