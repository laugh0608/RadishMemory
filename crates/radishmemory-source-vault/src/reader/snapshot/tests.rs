use super::*;
use crate::capture::tests::{DEVICE, NS, files, interrupt, request, run, setup};
use crate::test_support::{assert_database_writer_locked, key};
use std::{cell::Cell, fs};

fn catalog() -> SourceCatalogRequest {
    SourceCatalogRequest::new(Identifier::new(NS).unwrap(), 0, 200).unwrap()
}

#[test]
fn snapshot_selection_and_pending_inspection_share_one_session_without_mutation() {
    let (root, directory) = setup(true);
    run(
        &directory,
        &request(
            "second-source",
            "second-lineage",
            1,
            "Second synthetic body",
            None,
        ),
    )
    .unwrap();
    run(
        &directory,
        &request(
            "next-source",
            "legacy-lineage",
            2,
            "Next synthetic version",
            Some("legacy-source"),
        ),
    )
    .unwrap();
    interrupt(
        &directory,
        &request(
            "pending-source",
            "pending-lineage",
            1,
            "Uncommitted body",
            None,
        ),
        crate::capture::Step::Published,
    );
    let expected_target =
        crate::abandonment::inspect(&directory, NS, DEVICE, || Ok(key())).unwrap();
    let before = files(&root);
    let database = fs::read(root.0.join("library.sqlite3")).unwrap();
    let preferred = Identifier::new("legacy-lineage").unwrap();
    let loads = Cell::new(0);
    let result = snapshot(&directory, NS, DEVICE, &catalog(), Some(&preferred), || {
        loads.set(loads.get() + 1);
        assert_database_writer_locked(&root.0);
        Ok(key())
    })
    .unwrap();
    assert_eq!(loads.get(), 2);
    assert_eq!(result.sources.len(), 2);
    assert_eq!(result.selected_lineage_id, Some(preferred));
    assert_eq!(
        result.selected_source_id.as_ref().unwrap().as_str(),
        "next-source"
    );
    assert_eq!(result.versions.len(), 2);
    assert_eq!(result.abandonment_target, expected_target);
    assert!(result.deletion_requests.is_empty());
    assert_eq!(files(&root), before);
    assert_eq!(fs::read(root.0.join("library.sqlite3")).unwrap(), database);
    // A new reader can acquire the lock after the completed action.
    drop(open(&directory, NS, DEVICE, || Ok(key())).unwrap());
    let missing = Identifier::new("missing-lineage").unwrap();
    let fallback = snapshot(&directory, NS, DEVICE, &catalog(), Some(&missing), || {
        Ok(key())
    })
    .unwrap();
    assert_eq!(
        fallback.selected_lineage_id.as_ref(),
        fallback.sources.first().map(|s| s.lineage_id())
    );
}

#[test]
fn snapshot_reauthenticates_objects_after_final_key_access() {
    for remove in [false, true] {
        let (root, directory) = setup(true);
        let name = files(&root)[0].0.clone();
        let path = root.0.join("source-objects-v1").join(name);
        let bytes = fs::read(&path).unwrap();
        let held = root.0.join("held-object");
        let mut damaged = bytes.clone();
        *damaged.last_mut().unwrap() ^= 1;
        let loads = Cell::new(0);
        let result = snapshot(&directory, NS, DEVICE, &catalog(), None, || {
            loads.set(loads.get() + 1);
            if loads.get() == 2 {
                if remove {
                    fs::rename(&path, &held).unwrap();
                } else {
                    fs::write(&path, &damaged).unwrap();
                }
            }
            Ok(key())
        });
        assert!(result.is_err());
        assert_eq!(loads.get(), 2);
        if remove {
            assert!(!path.exists());
            assert_eq!(fs::read(&held).unwrap(), bytes);
            fs::rename(&held, &path).unwrap();
        } else {
            assert_eq!(fs::read(&path).unwrap(), damaged);
            fs::write(&path, &bytes).unwrap();
        }
        snapshot(&directory, NS, DEVICE, &catalog(), None, || Ok(key())).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn snapshot_rejects_database_replacement_during_final_key_access() {
    let (root, directory) = setup(true);
    let path = root.0.join("library.sqlite3");
    let held = root.0.join("held-database");
    let replacement = root.0.join("replacement-database");
    fs::copy(&path, &replacement).unwrap();
    let mut loads = 0;
    let result = snapshot(&directory, NS, DEVICE, &catalog(), None, || {
        loads += 1;
        if loads == 2 {
            // Do not open/close this inode while its POSIX lock is held.
            fs::rename(&path, &held).unwrap();
            fs::rename(&replacement, &path).unwrap();
        }
        Ok(key())
    });
    assert!(result.is_err());
    fs::rename(&path, &replacement).unwrap();
    fs::rename(&held, &path).unwrap();
    drop(open(&directory, NS, DEVICE, || Ok(key())).unwrap());
}

#[test]
fn snapshot_releases_session_on_unwind_and_rejects_profile_before_loading() {
    let (_root, directory) = setup(true);
    let wrong_namespace =
        SourceCatalogRequest::new(Identifier::new("namespace-other").unwrap(), 0, 200).unwrap();
    for (namespace, device, request) in [
        (NS, DEVICE, &wrong_namespace),
        (NS, "device-00000000000000000000000000000000", &catalog()),
    ] {
        assert!(
            snapshot(&directory, namespace, device, request, None, || panic!(
                "preflight must not load key"
            ))
            .is_err()
        );
    }
    for panic_at in [1, 2] {
        let mut loads = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            snapshot(&directory, NS, DEVICE, &catalog(), None, || {
                loads += 1;
                assert_ne!(loads, panic_at, "synthetic key access unwind");
                Ok(key())
            })
        }));
        assert!(result.is_err());
        assert_eq!(loads, panic_at);
        drop(open(&directory, NS, DEVICE, || Ok(key())).unwrap());
    }
}
