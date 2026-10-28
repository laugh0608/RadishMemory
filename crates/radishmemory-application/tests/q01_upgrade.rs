//! Real application transitions using synthetic sources and the existing test key.
#[path = "q01/support.rs"]
mod support;

use radishmemory_application::{EncryptedLibraryLocation, LocalLibrary, RecallProfile};
use radishmemory_core::{DeletionOverallStatus, NonEmptyText, Sensitivity};
use radishmemory_file_entry::{FileExportRequest, FileReadRequest};
use radishmemory_source_vault::acceptance::SyntheticLibraryProvider;
use radishmemory_sqlite as _;
use rusqlite::Connection;
use std::fs;
use support::{Directory, Runtime, config};

fn input(root: &Directory, name: &str, body: &str) -> FileReadRequest {
    let path = root.0.join(name);
    fs::write(&path, body).unwrap();
    FileReadRequest::new(path, vec![root.0.clone()]).unwrap()
}
fn text(query: &str) -> NonEmptyText {
    NonEmptyText::new(query).unwrap()
}
fn database_version(root: &Directory) -> i64 {
    Connection::open(root.0.join("library.sqlite3"))
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

const ORIGINAL: &str = "\u{feff}项目默认使用蓝色主题。API接口使用UTF-8编码。\r\n";

#[test]
fn plain_explicit_upgrade_reopen_concurrent_connection_and_downgrade_preserve_sources() {
    let root = Directory::new();
    let path = root.0.join("library.sqlite3");
    let request = input(&root, "synthetic.md", ORIGINAL);
    let mut library = LocalLibrary::open(&path, Runtime::default(), config()).unwrap();
    let receipt = library.import_new_source(&request).unwrap();
    assert_eq!(
        library.recall_profile().unwrap(),
        RecallProfile::HanPhraseV1
    );
    library
        .change_recall_profile(RecallProfile::LegacyUnicode61)
        .unwrap();
    let (runtime, config) = library.close();
    let mut library = LocalLibrary::open(&path, runtime, config.clone()).unwrap();
    assert_eq!(
        library.recall_profile().unwrap(),
        RecallProfile::LegacyUnicode61
    );
    assert!(
        library
            .search_sources(text("蓝色"), 5, [Sensitivity::Personal])
            .unwrap()
            .is_empty()
    );
    // A connection opened before another connection changes the profile must
    // use the actual current format, not a cached tokenizer choice.
    let mut second = LocalLibrary::open(&path, Runtime::default(), config).unwrap();
    library
        .change_recall_profile(RecallProfile::HanPhraseV1)
        .unwrap();
    let hits = second
        .search_sources(text("蓝色 API"), 5, [Sensitivity::Personal])
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].source_id(), receipt.source_id());
    assert_eq!(hits[0].content().as_str(), ORIGINAL);
    assert_eq!(database_version(&root), 6);
    second
        .change_recall_profile(RecallProfile::LegacyUnicode61)
        .unwrap();
    assert!(
        library
            .search_sources(text("蓝色"), 5, [Sensitivity::Personal])
            .unwrap()
            .is_empty()
    );
    drop(second.close());
    library
        .change_recall_profile(RecallProfile::HanPhraseV1)
        .unwrap();
    let export = root.0.join("exact.md");
    library
        .export_source(
            receipt.source_id(),
            &FileExportRequest::new(&export, vec![root.0.clone()]).unwrap(),
        )
        .unwrap();
    assert_eq!(fs::read(export).unwrap(), ORIGINAL.as_bytes());
    let updated = input(&root, "updated.md", "新版本只保留红色主题。\n");
    library
        .update_source(receipt.lineage_id(), &updated)
        .unwrap();
    assert!(
        library
            .search_sources(text("蓝色"), 5, [Sensitivity::Personal])
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        library
            .search_sources(text("红色"), 5, [Sensitivity::Personal])
            .unwrap()
            .len(),
        1
    );
    let evidence = library.delete_source_lineage(receipt.lineage_id()).unwrap();
    assert_eq!(
        evidence.params().overall_status,
        DeletionOverallStatus::Completed
    );
    library
        .change_recall_profile(RecallProfile::LegacyUnicode61)
        .unwrap();
    library
        .change_recall_profile(RecallProfile::HanPhraseV1)
        .unwrap();
    assert!(
        library
            .search_sources(text("红色"), 5, [Sensitivity::Personal])
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        library
            .list_source_versions(receipt.lineage_id())
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn encrypted_profile_survives_bootstrap_migration_pending_capture_and_deletion_recovery() {
    let root = Directory::new();
    let request = input(&root, "synthetic.md", ORIGINAL);
    let mut plain =
        LocalLibrary::open(root.0.join("library.sqlite3"), Runtime::default(), config()).unwrap();
    let first = plain.import_new_source(&request).unwrap();
    let (runtime, config) = plain.close();
    let provider = SyntheticLibraryProvider::default();
    let location =
        EncryptedLibraryLocation::with_provider(&root.0, config, provider.clone()).unwrap();
    location.initialize_key().unwrap();
    assert!(
        location
            .change_recall_profile(RecallProfile::LegacyUnicode61)
            .is_err()
    );
    provider.interrupt_next_migration_after_publish();
    assert!(location.migrate_bodies().is_err());
    assert!(
        location
            .change_recall_profile(RecallProfile::LegacyUnicode61)
            .is_err()
    );
    location.migrate_bodies().unwrap();
    assert_eq!(
        location.verify_library().unwrap().recall_profile,
        RecallProfile::HanPhraseV1
    );
    let mut library = location.open(runtime).unwrap();
    assert_eq!(
        library
            .search_sources(text("蓝色 API"), 5, [Sensitivity::Personal])
            .unwrap()
            .len(),
        1
    );
    library
        .change_recall_profile(RecallProfile::LegacyUnicode61)
        .unwrap();
    let (runtime, location) = library.close();
    let mut library = location.open(runtime).unwrap();
    assert_eq!(
        library.read_snapshot(None, 20).unwrap().recall_profile,
        RecallProfile::LegacyUnicode61
    );
    assert!(
        library
            .search_sources(text("蓝色"), 5, [Sensitivity::Personal])
            .unwrap()
            .is_empty()
    );
    let version = database_version(&root);
    library
        .change_recall_profile(RecallProfile::HanPhraseV1)
        .unwrap();
    assert_eq!(database_version(&root), version);

    let updated = input(&root, "updated.md", "新版本只保留红色主题。\n");
    let capture = library
        .prepare_update_source(first.lineage_id(), &updated)
        .unwrap();
    provider.interrupt_next_capture_after_publish();
    assert!(library.capture_source(&capture).is_err());
    assert!(
        library
            .change_recall_profile(RecallProfile::LegacyUnicode61)
            .is_err()
    );
    library.capture_source(&capture).unwrap();
    assert!(
        library
            .search_sources(text("蓝色"), 5, [Sensitivity::Personal])
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        library
            .search_sources(text("红色"), 5, [Sensitivity::Personal])
            .unwrap()
            .len(),
        1
    );
    let export = root.0.join("historical.md");
    library
        .export_source(
            first.source_id(),
            &FileExportRequest::new(&export, vec![root.0.clone()]).unwrap(),
        )
        .unwrap();
    assert_eq!(fs::read(export).unwrap(), ORIGINAL.as_bytes());

    let deletion = library
        .prepare_source_lineage_deletion(first.lineage_id())
        .unwrap();
    provider.interrupt_next_deletion_evidence(false);
    assert!(library.execute_source_lineage_deletion(&deletion).is_err());
    assert!(
        library
            .change_recall_profile(RecallProfile::LegacyUnicode61)
            .is_err()
    );
    let evidence = library.execute_source_lineage_deletion(&deletion).unwrap();
    assert_eq!(
        evidence.params().overall_status,
        DeletionOverallStatus::Completed
    );
    assert!(
        library
            .search_sources(text("红色"), 5, [Sensitivity::Personal])
            .unwrap()
            .is_empty()
    );
    library
        .change_recall_profile(RecallProfile::LegacyUnicode61)
        .unwrap();
    library
        .change_recall_profile(RecallProfile::HanPhraseV1)
        .unwrap();
    assert_eq!(provider.key_creations(), 1);
    assert_eq!(database_version(&root), 11);
}

#[test]
fn production_rejects_disjoint_han_substrings_and_operator_injection() {
    for encrypted in [false, true] {
        let root = Directory::new();
        let mut plain =
            LocalLibrary::open(root.0.join("library.sqlite3"), Runtime::default(), config())
                .unwrap();
        for (i, body) in [
            "蓝色 哈哈",
            "蓝天的颜色 哈",
            "蓝，色",
            "色蓝",
            "蓝API色",
            "APIs RESTful",
            "h84dd b q01hanboundary",
            "foo中文bar",
            "foo-bar",
        ]
        .iter()
        .enumerate()
        {
            plain
                .import_new_source(&input(&root, &format!("synthetic-{i}.md"), body))
                .unwrap();
        }
        let queries = [
            ("蓝色", 1),
            ("哈哈", 1),
            ("API", 1),
            ("REST", 0),
            ("API OR RESTful", 0),
            ("h84dd", 1),
            ("q01hanboundary", 1),
            ("foo-bar", 1),
            ("foo bar", 2),
            ("，！", 0),
            ("han_terms : *", 0),
        ];
        if encrypted {
            let (runtime, config) = plain.close();
            let location = EncryptedLibraryLocation::with_provider(
                &root.0,
                config,
                SyntheticLibraryProvider::default(),
            )
            .unwrap();
            location.initialize_key().unwrap();
            location.migrate_bodies().unwrap();
            let mut library = location.open(runtime).unwrap();
            for (query, expected) in queries {
                assert_eq!(
                    library
                        .search_sources(text(query), 20, [Sensitivity::Personal])
                        .unwrap()
                        .len(),
                    expected,
                    "{query}"
                );
                assert!(
                    library
                        .search_sources(text(query), 20, [Sensitivity::Restricted])
                        .unwrap()
                        .is_empty()
                );
            }
        } else {
            for (query, expected) in queries {
                assert_eq!(
                    plain
                        .search_sources(text(query), 20, [Sensitivity::Personal])
                        .unwrap()
                        .len(),
                    expected,
                    "{query}"
                );
                assert!(
                    plain
                        .search_sources(text(query), 20, [Sensitivity::Restricted])
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }
}
