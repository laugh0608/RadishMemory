//! Bounded manual measurement; no unreviewed performance threshold or CI timing gate.
#[path = "q01/support.rs"]
mod support;
use radishmemory_application::{EncryptedLibraryLocation, LocalLibrary, RecallProfile};
use radishmemory_core::{NonEmptyText, Sensitivity};
use radishmemory_file_entry::FileReadRequest;
use radishmemory_source_vault::acceptance::SyntheticLibraryProvider;
use radishmemory_sqlite as _;
use rusqlite::Connection;
use std::{fs, time::Instant};
use support::{Directory, Runtime, config};

fn measure(
    root: &Directory,
    mode: &str,
    phase: &str,
    payload: usize,
    mut search: impl FnMut(&str) -> usize,
) {
    let connection = Connection::open(root.0.join("library.sqlite3")).unwrap();
    let pages: i64 = connection
        .pragma_query_value(None, "page_count", |row| row.get(0))
        .unwrap();
    let free: i64 = connection
        .pragma_query_value(None, "freelist_count", |row| row.get(0))
        .unwrap();
    let size: i64 = connection
        .pragma_query_value(None, "page_size", |row| row.get(0))
        .unwrap();
    drop(connection);
    for (kind, query) in [("marker", "costmarker17"), ("han", "蓝色")] {
        let mut times = Vec::new();
        let mut hits = 0;
        for _ in 0..5 {
            let start = Instant::now();
            hits = search(query);
            times.push(start.elapsed().as_micros());
        }
        times.sort();
        assert_eq!(
            hits,
            if kind == "marker" {
                1
            } else if phase == "han" {
                5
            } else {
                0
            }
        );
        println!(
            "Q01-COST mode={mode} phase={phase} kind={kind} documents=32 payload_bytes={payload} database_bytes={} used_page_bytes={} samples=5 median_us={} max_us={} hits={hits}",
            pages * size,
            (pages - free) * size,
            times[2],
            times[4]
        );
    }
}

#[test]
#[ignore = "bounded manual cost measurement; not a performance gate"]
fn q01_long_documents_measure_storage_upgrade_and_authenticated_query_cost() {
    for encrypted in [false, true] {
        let root = Directory::new();
        let mut library =
            LocalLibrary::open(root.0.join("library.sqlite3"), Runtime::default(), config())
                .unwrap();
        library
            .change_recall_profile(RecallProfile::LegacyUnicode61)
            .unwrap();
        let mut payload = 0;
        for index in 0..32 {
            let body = format!(
                "costmarker{index}\n{}",
                "合成项目默认使用蓝色主题并保留来源版本。API UTF-8 本地资料。\n".repeat(256)
            );
            payload += body.len();
            let path = root.0.join(format!("synthetic-{index}.md"));
            fs::write(&path, body).unwrap();
            library
                .import_new_source(&FileReadRequest::new(path, vec![root.0.clone()]).unwrap())
                .unwrap();
        }
        if encrypted {
            let (runtime, config) = library.close();
            let location = EncryptedLibraryLocation::with_provider(
                &root.0,
                config,
                SyntheticLibraryProvider::default(),
            )
            .unwrap();
            location.initialize_key().unwrap();
            location.migrate_bodies().unwrap();
            let mut library = location.open(runtime).unwrap();
            measure(&root, "encrypted", "legacy", payload, |query| {
                library
                    .search_sources(
                        NonEmptyText::new(query).unwrap(),
                        5,
                        [Sensitivity::Personal],
                    )
                    .unwrap()
                    .len()
            });
            let start = Instant::now();
            library
                .change_recall_profile(RecallProfile::HanPhraseV1)
                .unwrap();
            println!(
                "Q01-COST-UPGRADE mode=encrypted elapsed_ms={}",
                start.elapsed().as_millis()
            );
            measure(&root, "encrypted", "han", payload, |query| {
                library
                    .search_sources(
                        NonEmptyText::new(query).unwrap(),
                        5,
                        [Sensitivity::Personal],
                    )
                    .unwrap()
                    .len()
            });
        } else {
            measure(&root, "plain", "legacy", payload, |query| {
                library
                    .search_sources(
                        NonEmptyText::new(query).unwrap(),
                        5,
                        [Sensitivity::Personal],
                    )
                    .unwrap()
                    .len()
            });
            let start = Instant::now();
            library
                .change_recall_profile(RecallProfile::HanPhraseV1)
                .unwrap();
            println!(
                "Q01-COST-UPGRADE mode=plain elapsed_ms={}",
                start.elapsed().as_millis()
            );
            measure(&root, "plain", "han", payload, |query| {
                library
                    .search_sources(
                        NonEmptyText::new(query).unwrap(),
                        5,
                        [Sensitivity::Personal],
                    )
                    .unwrap()
                    .len()
            });
        }
    }
}
