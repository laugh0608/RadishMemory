use super::*;
use crate::capture::tests::setup;
use radishmemory_core::{Identifier, SourceCatalogRequest};

fn read_snapshot(
    directory: &crate::ObjectDirectory,
    store: &Store,
) -> std::result::Result<crate::LibraryReadSnapshot, crate::VaultMaintenanceError> {
    let request = SourceCatalogRequest::new(Identifier::new(NAMESPACE).unwrap(), 0, 200).unwrap();
    crate::reader::snapshot(directory, NAMESPACE, DEVICE, &request, None, || {
        load_existing(store)
    })
}

fn snapshot_store() -> Store {
    let store = Store::valid();
    *store.value.borrow_mut() = Some(encode(&[0xa5; 32]).to_vec());
    store
}

fn code(error: crate::VaultMaintenanceError) -> SourceVaultErrorCode {
    match error {
        crate::VaultMaintenanceError::Vault(error) => error.code(),
        other => panic!("unexpected error class: {other:?}"),
    }
}

#[test]
fn snapshot_double_reads_at_entry_and_release_without_writes_or_cross_action_cache() {
    for populated in [false, true] {
        let (_root, directory) = setup(populated);
        let store = snapshot_store();
        for action in 1..=2 {
            let result = read_snapshot(&directory, &store).unwrap();
            assert_eq!(result.sources.len(), usize::from(populated));
            assert_eq!(result.versions.len(), usize::from(populated));
            assert_eq!(store.read_count.get(), action * 4);
            assert_eq!(store.label_checks.get(), action * 4);
            store.assert_no_writes();
        }
        *store.value.borrow_mut() = None;
        assert_eq!(
            code(read_snapshot(&directory, &store).unwrap_err()),
            SourceVaultErrorCode::KeyMissing
        );
        assert_eq!(store.read_count.get(), 9);
    }
}

#[test]
fn snapshot_stops_on_every_secret_read_failure_and_preserves_the_error() {
    let (root, directory) = setup(true);
    let before = std::fs::read(root.0.join("library.sqlite3")).unwrap();
    for failure in [
        SourceVaultErrorCode::KeyStoreCancelled,
        SourceVaultErrorCode::KeyStoreDenied,
        SourceVaultErrorCode::KeyStoreLocked,
        SourceVaultErrorCode::KeyMissing,
        SourceVaultErrorCode::KeyAmbiguous,
        SourceVaultErrorCode::KeyMetadataMismatch,
        SourceVaultErrorCode::KeyStoreUnavailable,
        SourceVaultErrorCode::KeyStoreFailure,
        SourceVaultErrorCode::KeyCorrupt,
    ] {
        for position in 1..=4 {
            let store = snapshot_store();
            store.script(
                (1..position)
                    .map(|_| Read::Value(encode(&[0xa5; 32]).to_vec()))
                    .chain(std::iter::once(Read::Fail(failure))),
            );
            assert_eq!(
                code(read_snapshot(&directory, &store).unwrap_err()),
                failure
            );
            assert_eq!(store.read_count.get(), position);
            store.assert_no_writes();
            // Every failure releases its exclusive session, without retrying.
            read_snapshot(&directory, &snapshot_store()).unwrap();
        }
    }
    assert_eq!(
        std::fs::read(root.0.join("library.sqlite3")).unwrap(),
        before
    );
}

#[test]
fn snapshot_rejects_intra_load_and_between_load_key_changes_even_in_empty_library() {
    for populated in [false, true] {
        let (_root, directory) = setup(populated);
        for values in [
            [0xa5, 0x44, 0x44, 0x44],
            [0xa5, 0xa5, 0x44, 0xa5],
            [0xa5, 0xa5, 0x44, 0x44],
        ] {
            let store = snapshot_store();
            store.script(values.map(|value| Read::Value(encode(&[value; 32]).to_vec())));
            assert_eq!(
                code(read_snapshot(&directory, &store).unwrap_err()),
                SourceVaultErrorCode::KeyReadbackMismatch
            );
            assert_eq!(
                store.read_count.get(),
                if values[1] == 0x44 { 2 } else { 4 }
            );
            store.assert_no_writes();
            read_snapshot(&directory, &snapshot_store()).unwrap();
        }
    }
}

#[test]
fn observed_snapshot_preserves_reads_labels_final_key_check_and_original_errors() {
    use crate::key_read_observation::{TraceFailure, tests::recording};
    let (_root, directory) = setup(true);
    let store = snapshot_store();
    let (result, log) = recording(None, None, || read_snapshot(&directory, &store));
    assert_eq!(result.unwrap().sources.len(), 1);
    assert_eq!((store.read_count.get(), store.label_checks.get()), (4, 4));
    store.assert_no_writes();
    let text = String::from_utf8(log.borrow().bytes.clone()).unwrap();
    assert_eq!(
        text.matches("\"operation\":\"Entry\",\"edge\":\"begin\"")
            .count(),
        1
    );
    assert_eq!(
        text.matches("\"operation\":\"Release\",\"edge\":\"begin\"")
            .count(),
        1
    );
    assert_eq!(
        text.matches("\"operation\":\"Label\",\"edge\":\"begin\"")
            .count(),
        4
    );
    for position in 1..=4 {
        let store = snapshot_store();
        store.script(
            (1..position)
                .map(|_| Read::Value(encode(&[0xa5; 32]).to_vec()))
                .chain(std::iter::once(Read::Fail(
                    SourceVaultErrorCode::KeyStoreCancelled,
                ))),
        );
        let (result, _) = recording(None, None, || read_snapshot(&directory, &store));
        let Err(TraceFailure::Business(error)) = result else {
            panic!("business error lost")
        };
        assert_eq!(code(error), SourceVaultErrorCode::KeyStoreCancelled);
        assert_eq!(store.read_count.get(), position);
        store.assert_no_writes();
    }
    for (values, reads) in [([0xa5, 0x44, 0x44, 0x44], 2), ([0xa5, 0xa5, 0x44, 0x44], 4)] {
        let store = snapshot_store();
        store.script(values.map(|value| Read::Value(encode(&[value; 32]).to_vec())));
        let (result, _) = recording(None, None, || read_snapshot(&directory, &store));
        let Err(TraceFailure::Business(error)) = result else {
            panic!("changed key accepted")
        };
        assert_eq!(code(error), SourceVaultErrorCode::KeyReadbackMismatch);
        assert_eq!(store.read_count.get(), reads);
    }
    let store = snapshot_store();
    *store.label.borrow_mut() = "synthetic-private-label".into();
    let (result, log) = recording(None, None, || read_snapshot(&directory, &store));
    let Err(TraceFailure::Business(error)) = result else {
        panic!("changed label accepted")
    };
    assert_eq!(code(error), SourceVaultErrorCode::KeyMetadataMismatch);
    assert_eq!(store.read_count.get(), 1);
    assert!(
        !String::from_utf8(log.borrow().bytes.clone())
            .unwrap()
            .contains("synthetic")
    );
}
