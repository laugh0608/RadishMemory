use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;

use rusqlite::Connection;
use zeroize::Zeroizing;

use super::*;
use crate::provider::{KeyStore, load_or_bootstrap};
use crate::test_support::{FixedRandom, TestDirectory, metadata};
use crate::{ObjectWrite, open_object, seal_object};

const NAMESPACE: &str = "namespace-0123456789abcdef0123456789abcdef";
const DEVICE: &str = "device-fedcba9876543210fedcba9876543210";

#[derive(Default)]
struct Store {
    value: Mutex<Option<Zeroizing<Vec<u8>>>>,
    writes: AtomicUsize,
    failure: Mutex<Option<SourceVaultErrorCode>>,
}
impl KeyStore for Store {
    fn read_secret(&self) -> Result<Zeroizing<Vec<u8>>, SourceVaultError> {
        if let Some(code) = *self.failure.lock().unwrap() {
            return Err(SourceVaultError::new(code, "synthetic store failure"));
        }
        self.value.lock().unwrap().clone().ok_or_else(|| {
            SourceVaultError::new(SourceVaultErrorCode::KeyMissing, "synthetic missing key")
        })
    }
    fn validate_label(&self) -> Result<(), SourceVaultError> {
        Ok(())
    }
    fn write_secret(&self, value: &[u8]) -> Result<(), SourceVaultError> {
        *self.value.lock().unwrap() = Some(Zeroizing::new(value.to_vec()));
        self.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn set_label(&self) -> Result<(), SourceVaultError> {
        Ok(())
    }
}
fn run(
    directory: &ObjectDirectory,
    store: &Store,
) -> Result<KeyEncryptionKey, KeyInitializationError> {
    initialize(directory, NAMESPACE, DEVICE, |existing| {
        load_or_bootstrap(store, existing, &mut FixedRandom(13))
    })
}
fn version(root: &TestDirectory) -> i64 {
    Connection::open(root.0.join("library.sqlite3"))
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}
fn vault_code(error: KeyInitializationError) -> SourceVaultErrorCode {
    match error {
        KeyInitializationError::Vault(error) => error.code(),
        _ => panic!("expected vault failure"),
    }
}

#[test]
fn fresh_library_commits_once_and_reopens_with_the_same_key() {
    let root = TestDirectory::new();
    let directory = ObjectDirectory::open_application_directory(&root.0).unwrap();
    let store = Store::default();
    let key = run(&directory, &store).unwrap();
    assert_eq!(version(&root), 7);
    let body = b"synthetic checkpoint";
    let meta = metadata(body);
    let sealed = seal_object(&key, &meta, body).unwrap();
    let reopened = run(&directory, &store).unwrap();
    assert_eq!(open_object(&reopened, &meta, &sealed).unwrap(), body);
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(
        radishmemory_sqlite::SqliteDatabase::open(root.0.join("library.sqlite3"))
            .unwrap_err()
            .code(),
        SqliteErrorCode::UnsupportedSchemaVersion
    );
}

#[test]
fn missing_key_after_checkpoint_never_creates_a_replacement() {
    let root = TestDirectory::new();
    let directory = ObjectDirectory::open_application_directory(&root.0).unwrap();
    let store = Store::default();
    run(&directory, &store).unwrap();
    *store.value.lock().unwrap() = None;
    assert_eq!(
        vault_code(run(&directory, &store).unwrap_err()),
        SourceVaultErrorCode::KeyMissing
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(version(&root), 7);
}

#[test]
fn provider_failures_and_corrupt_existing_keys_roll_back_without_writes() {
    for code in [
        SourceVaultErrorCode::KeyAmbiguous,
        SourceVaultErrorCode::KeyCorrupt,
        SourceVaultErrorCode::KeyStoreDenied,
        SourceVaultErrorCode::KeyStoreCancelled,
        SourceVaultErrorCode::KeyStoreLocked,
        SourceVaultErrorCode::KeyStoreUnavailable,
    ] {
        let root = TestDirectory::new();
        let directory = ObjectDirectory::open_application_directory(&root.0).unwrap();
        let store = Store::default();
        *store.failure.lock().unwrap() = Some(code);
        assert_eq!(vault_code(run(&directory, &store).unwrap_err()), code);
        assert_eq!(version(&root), 0);
        assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    }
    let root = TestDirectory::new();
    let directory = ObjectDirectory::open_application_directory(&root.0).unwrap();
    let store = Store::default();
    *store.value.lock().unwrap() = Some(Zeroizing::new(b"synthetic malformed value".to_vec()));
    assert_eq!(
        vault_code(run(&directory, &store).unwrap_err()),
        SourceVaultErrorCode::KeyCorrupt
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn published_staging_and_unknown_entries_block_missing_key_creation() {
    for child in ["source-objects-v1", "source-staging-v1"] {
        let root = TestDirectory::new();
        let directory = ObjectDirectory::open_application_directory(&root.0).unwrap();
        std::fs::write(root.0.join(child).join("synthetic-unknown"), b"synthetic").unwrap();
        let store = Store::default();
        assert_eq!(
            vault_code(run(&directory, &store).unwrap_err()),
            SourceVaultErrorCode::KeyMissing
        );
        assert_eq!(store.writes.load(Ordering::SeqCst), 0);
        assert_eq!(version(&root), 0);
    }
    let root = TestDirectory::new();
    let directory = ObjectDirectory::open_application_directory(&root.0).unwrap();
    let key = crate::test_support::key();
    let write =
        ObjectWrite::seal(&key, &metadata(b"synthetic object"), b"synthetic object").unwrap();
    directory.publish(&write, &key).unwrap();
    assert_eq!(
        vault_code(run(&directory, &Store::default()).unwrap_err()),
        SourceVaultErrorCode::KeyMissing
    );
}

#[test]
fn directory_change_after_key_write_does_not_commit_or_replace_the_key() {
    let root = TestDirectory::new();
    let directory = ObjectDirectory::open_application_directory(&root.0).unwrap();
    let store = Store::default();
    let error = initialize(&directory, NAMESPACE, DEVICE, |existing| {
        let key = load_or_bootstrap(&store, existing, &mut FixedRandom(11))?;
        std::fs::write(
            root.0.join("source-staging-v1").join("synthetic-race"),
            b"synthetic",
        )
        .unwrap();
        Ok(key)
    })
    .unwrap_err();
    assert_eq!(vault_code(error), SourceVaultErrorCode::AttemptMismatch);
    assert_eq!(version(&root), 0);
    assert_eq!(
        vault_code(run(&directory, &store).unwrap_err()),
        SourceVaultErrorCode::AttemptMismatch
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
}

#[test]
fn actual_sqlite_commit_failure_leaves_a_reusable_key() {
    let root = TestDirectory::new();
    let path = root.0.join("library.sqlite3");
    drop(radishmemory_sqlite::SqliteDatabase::open(&path).unwrap());
    let directory = ObjectDirectory::open_application_directory(&root.0).unwrap();
    let store = Store::default();
    let reader = Connection::open(&path).unwrap();
    reader
        .execute_batch("BEGIN; SELECT * FROM radishmemory_schema_migrations;")
        .unwrap();
    let error = run(&directory, &store).unwrap_err();
    assert!(matches!(
        error,
        KeyInitializationError::Database {
            code: SqliteErrorCode::Storage,
            sqlite_extended_code: Some(rusqlite::ffi::SQLITE_BUSY),
            ..
        }
    ));
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    reader.execute_batch("ROLLBACK").unwrap();
    assert_eq!(version(&root), 6);
    let value_before = store.value.lock().unwrap().clone();
    run(&directory, &store).unwrap();
    assert_eq!(
        store.value.lock().unwrap().as_ref().unwrap().as_slice(),
        value_before.as_ref().unwrap().as_slice()
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(version(&root), 7);
}

#[test]
fn two_connections_recheck_committed_facts_after_waiting_for_the_writer() {
    let root = TestDirectory::new();
    let path = root.0.join("library.sqlite3");
    let first = SourceVaultKeyDatabase::open(&path).unwrap();
    let second = SourceVaultKeyDatabase::open(&path).unwrap();
    let store = Arc::new(Store::default());
    let (held_tx, held_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (second_entered_tx, second_entered_rx) = mpsc::channel();
    let first_root = root.0.clone();
    let first_store = Arc::clone(&store);
    let thread_one = std::thread::spawn(move || {
        let directory = ObjectDirectory::open_application_directory(first_root).unwrap();
        let mut first = first;
        initialize_in_database(&mut first, &directory, NAMESPACE, DEVICE, |existing| {
            assert!(!existing);
            held_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            load_or_bootstrap(&*first_store, existing, &mut FixedRandom(31))
        })
    });
    held_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let second_root = root.0.clone();
    let second_store = Arc::clone(&store);
    let thread_two = std::thread::spawn(move || {
        let directory = ObjectDirectory::open_application_directory(second_root).unwrap();
        let mut second = second;
        initialize_in_database(&mut second, &directory, NAMESPACE, DEVICE, |existing| {
            second_entered_tx.send(existing).unwrap();
            load_or_bootstrap(&*second_store, existing, &mut FixedRandom(71))
        })
    });
    assert!(matches!(
        second_entered_rx.recv_timeout(Duration::from_millis(150)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    release_tx.send(()).unwrap();
    let key_one = thread_one.join().unwrap().unwrap();
    let key_two = thread_two.join().unwrap().unwrap();
    assert!(
        second_entered_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
    );
    let meta = metadata(b"synthetic concurrent key");
    let sealed = seal_object(&key_one, &meta, b"synthetic concurrent key").unwrap();
    assert_eq!(
        open_object(&key_two, &meta, &sealed).unwrap(),
        b"synthetic concurrent key"
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
}

#[test]
fn slot_mismatch_schema_drift_and_future_schema_fail_before_provider_access() {
    let root = TestDirectory::new();
    let directory = ObjectDirectory::open_application_directory(&root.0).unwrap();
    run(&directory, &Store::default()).unwrap();
    initialize(&directory, NAMESPACE, "device-other", |_| {
        panic!("must not access provider")
    })
    .unwrap_err();
    let raw = Connection::open(root.0.join("library.sqlite3")).unwrap();
    raw.execute("DELETE FROM radishmemory_source_vault_key_profile", [])
        .unwrap();
    initialize(&directory, NAMESPACE, DEVICE, |_| {
        panic!("must not access provider")
    })
    .unwrap_err();
    raw.pragma_update(None, "user_version", 8).unwrap();
    initialize(&directory, NAMESPACE, DEVICE, |_| {
        panic!("must not access provider")
    })
    .unwrap_err();
}

#[test]
fn diagnostic_chain_does_not_expose_sql_or_slot_identity() {
    let root = TestDirectory::new();
    let directory = ObjectDirectory::open_application_directory(&root.0).unwrap();
    let raw = Connection::open(root.0.join("library.sqlite3")).unwrap();
    raw.execute_batch("CREATE TABLE synthetic_private_table (value TEXT);")
        .unwrap();
    let error = run(&directory, &Store::default()).unwrap_err();
    let text = format!("{error} {error:?}");
    for forbidden in [
        "synthetic_private_table",
        NAMESPACE,
        DEVICE,
        root.0.to_str().unwrap(),
        "CREATE TABLE",
    ] {
        assert!(!text.contains(forbidden));
    }
    assert!(error.source().is_none());
}

#[cfg(unix)]
#[test]
fn database_symlink_and_replacement_cannot_return_initialization_success() {
    use std::os::unix::fs::symlink;
    let root = TestDirectory::new();
    let directory = ObjectDirectory::open_application_directory(&root.0).unwrap();
    let other = root.0.join("synthetic-other.sqlite3");
    std::fs::write(&other, b"").unwrap();
    let database = root.0.join("library.sqlite3");
    symlink(&other, &database).unwrap();
    initialize(&directory, NAMESPACE, DEVICE, |_| {
        panic!("symlink must fail before provider access")
    })
    .unwrap_err();
    std::fs::remove_file(&database).unwrap();
    let store = Store::default();
    let error = initialize(&directory, NAMESPACE, DEVICE, |existing| {
        let key = load_or_bootstrap(&store, existing, &mut FixedRandom(9))?;
        std::fs::rename(&database, root.0.join("synthetic-original.sqlite3")).unwrap();
        std::fs::rename(&other, &database).unwrap();
        Ok(key)
    })
    .unwrap_err();
    assert_eq!(vault_code(error), SourceVaultErrorCode::FilesystemChanged);
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(version(&root), 0);
}
