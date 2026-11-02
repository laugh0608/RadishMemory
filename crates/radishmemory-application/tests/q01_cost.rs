//! Bounded manual measurement; no unreviewed performance threshold or CI timing gate.
#[path = "q01/support.rs"]
mod support;

use radishmemory_application::{
    EncryptedLibrary, EncryptedLibraryLocation, LocalLibrary, RecallProfile,
};
use radishmemory_core::{NonEmptyText, Sensitivity};
use radishmemory_file_entry::FileReadRequest;
use radishmemory_source_vault::acceptance::SyntheticLibraryProvider;
use radishmemory_sqlite::read_cost::{self, Observation, Stage};
use rusqlite::Connection;
use std::fs;
use support::{Directory, Runtime, config};

const DOCUMENTS: usize = 32;
const SAMPLES: usize = 11;
const PAGE_SIZE: usize = 200;

struct Case {
    label: &'static str,
    query: &'static str,
    top_k: usize,
    allowed: Sensitivity,
}

const CASES: &[Case] = &[
    Case {
        label: "marker",
        query: "costmarker17",
        top_k: 5,
        allowed: Sensitivity::Personal,
    },
    Case {
        label: "han_one",
        query: "蓝色",
        top_k: 1,
        allowed: Sensitivity::Personal,
    },
    Case {
        label: "han_five",
        query: "蓝色",
        top_k: 5,
        allowed: Sensitivity::Personal,
    },
    Case {
        label: "absent",
        query: "量子菜园",
        top_k: 5,
        allowed: Sensitivity::Personal,
    },
    Case {
        label: "denied",
        query: "costmarker17",
        top_k: 5,
        allowed: Sensitivity::Restricted,
    },
];

// Only fixture ownership/dispatch. Every operation uses the existing application
// API and the real crypto/object/SQLite path with the synthetic key provider.
enum Library {
    Plain(LocalLibrary<Runtime>),
    Encrypted(EncryptedLibrary<Runtime, SyntheticLibraryProvider>),
}

impl Library {
    fn reopen(self, root: &Directory) -> Self {
        match self {
            Self::Plain(library) => {
                let (runtime, config) = library.close();
                Self::Plain(
                    LocalLibrary::open(root.0.join("library.sqlite3"), runtime, config).unwrap(),
                )
            }
            Self::Encrypted(library) => {
                let (runtime, location) = library.close();
                Self::Encrypted(location.open(runtime).unwrap())
            }
        }
    }

    fn change_profile(&mut self, profile: RecallProfile) {
        match self {
            Self::Plain(library) => {
                library.change_recall_profile(profile).unwrap();
            }
            Self::Encrypted(library) => {
                library.change_recall_profile(profile).unwrap();
            }
        }
    }

    fn search(&mut self, case: &Case) -> usize {
        let query = NonEmptyText::new(case.query).unwrap();
        match self {
            Self::Plain(library) => library.search_sources(query, case.top_k, [case.allowed]),
            Self::Encrypted(library) => library.search_sources(query, case.top_k, [case.allowed]),
        }
        .unwrap()
        .len()
    }

    fn refresh(&self) -> usize {
        match self {
            Self::Plain(library) => {
                let _cost = read_cost::span(Stage::SnapshotRefresh);
                let sources = library.list_sources(0, PAGE_SIZE).unwrap();
                let versions = library
                    .list_source_versions(sources[0].lineage_id())
                    .unwrap();
                assert_eq!(versions.len(), 1);
                sources.len()
            }
            Self::Encrypted(library) => {
                let snapshot = library.read_snapshot(None, PAGE_SIZE).unwrap();
                assert_eq!(snapshot.versions.len(), 1);
                assert!(snapshot.abandonment_target.is_none());
                assert!(snapshot.deletion_requests.is_empty());
                snapshot.sources.len()
            }
        }
    }
}

fn storage(root: &Directory, mode: &str, profile: &str, payload: usize) {
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
    let objects = root.0.join("source-objects-v1");
    // Only regular immutable object files; no guessed "whole library" size.
    let object_bytes: u64 = if objects.exists() {
        fs::read_dir(objects)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                assert!(entry.file_type().unwrap().is_file());
                entry.metadata().unwrap().len()
            })
            .sum()
    } else {
        0
    };
    println!(
        "Q04-STORAGE mode={mode} profile={profile} corpus=q04-repeat-v1 documents={DOCUMENTS} versions_per_source=1 payload_bytes={payload} database_bytes={} used_page_bytes={} object_bytes={object_bytes}",
        pages * size,
        (pages - free) * size
    );
}

fn sample(
    mode: &str,
    profile: &str,
    case: &str,
    cache: &str,
    index: usize,
    observation: &Observation,
) {
    let accounted = Stage::ALL
        .into_iter()
        .map(|stage| observation.cost(stage).exclusive)
        .sum();
    let unscoped = observation
        .elapsed
        .checked_sub(accounted)
        .expect("exclusive stages must not overlap");
    println!(
        "Q04-SAMPLE mode={mode} profile={profile} case={case} cache={cache} sample={index} wall_ns={} unscoped_ns={}",
        observation.elapsed.as_nanos(),
        unscoped.as_nanos()
    );
    for stage in Stage::ALL {
        let cost = observation.cost(stage);
        println!(
            "Q04-STAGE mode={mode} profile={profile} case={case} cache={cache} sample={index} stage={} calls={} inclusive_ns={} exclusive_ns={}",
            stage.label(),
            cost.calls,
            cost.inclusive.as_nanos(),
            cost.exclusive.as_nanos()
        );
    }
}

fn measure(
    mode: &str,
    profile: &str,
    case: &str,
    expected: usize,
    mut operation: impl FnMut() -> usize,
) {
    let mut observations = Vec::new();
    for index in 0..=SAMPLES {
        let (hits, observation) = read_cost::observe(&mut operation);
        assert_eq!(hits, expected, "case={case} mode={mode} profile={profile}");
        let cache = if index == 0 {
            "reopened_first"
        } else {
            "repeated"
        };
        sample(mode, profile, case, cache, index, &observation);
        if index > 0 {
            observations.push(observation);
        }
    }
    observations.sort_by_key(|observation| observation.elapsed);
    // Nearest-rank percentiles. With 11 repetitions p95 is the maximum; these
    // describe this bounded run, not a production tail-latency estimate.
    let p50 = &observations[SAMPLES.div_ceil(2) - 1];
    let p95 = &observations[(SAMPLES * 95).div_ceil(100) - 1];
    println!(
        "Q04-SUMMARY mode={mode} profile={profile} case={case} cache=repeated samples={SAMPLES} percentile=nearest_rank p50_us={} p95_us={} hits={expected}",
        p50.elapsed.as_micros(),
        p95.elapsed.as_micros()
    );
}

#[test]
#[ignore = "bounded manual release cost measurement; not a performance gate"]
fn q01_long_documents_measure_storage_upgrade_and_authenticated_query_cost() {
    if cfg!(debug_assertions) {
        panic!("run this measurement with --release");
    }
    println!(
        "Q04-RUN corpus=q04-repeat-v1 arch={} os={} profile=release samples={SAMPLES} cache_policy=reopen_without_os_cache_flush provider=synthetic top_k=1_or_5",
        std::env::consts::ARCH,
        std::env::consts::OS
    );
    for encrypted in [false, true] {
        let root = Directory::new();
        let mut library =
            LocalLibrary::open(root.0.join("library.sqlite3"), Runtime::default(), config())
                .unwrap();
        library
            .change_recall_profile(RecallProfile::LegacyUnicode61)
            .unwrap();
        let mut payload = 0;
        for index in 0..DOCUMENTS {
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
        let mut library = if encrypted {
            let (runtime, config) = library.close();
            let location = EncryptedLibraryLocation::with_provider(
                &root.0,
                config,
                SyntheticLibraryProvider::default(),
            )
            .unwrap();
            location.initialize_key().unwrap();
            location.migrate_bodies().unwrap();
            Library::Encrypted(location.open(runtime).unwrap())
        } else {
            Library::Plain(library)
        };
        let mode = if encrypted { "encrypted" } else { "plain" };
        for (profile, format) in [
            (RecallProfile::LegacyUnicode61, "legacy"),
            (RecallProfile::HanPhraseV1, "han"),
        ] {
            let (_, upgrade) = read_cost::observe(|| library.change_profile(profile));
            println!(
                "Q04-PROFILE mode={mode} profile={format} wall_us={}",
                upgrade.elapsed.as_micros()
            );
            storage(&root, mode, format, payload);
            for case in CASES {
                let (reopened, open) = read_cost::observe(|| library.reopen(&root));
                library = reopened;
                sample(mode, format, case.label, "reopen", 0, &open);
                let expected = match case.label {
                    "marker" => 1,
                    "han_one" | "han_five" if profile == RecallProfile::HanPhraseV1 => case.top_k,
                    _ => 0,
                };
                measure(mode, format, case.label, expected, || library.search(case));
            }
            let (reopened, open) = read_cost::observe(|| library.reopen(&root));
            library = reopened;
            sample(mode, format, "refresh", "reopen", 0, &open);
            measure(mode, format, "refresh", DOCUMENTS, || library.refresh());
        }
    }
}
