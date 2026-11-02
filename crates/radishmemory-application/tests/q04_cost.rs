//! Explicit 100-lineage mixed-size/multi-version release baseline, not a CI time gate.
#[path = "q04/corpus.rs"]
mod corpus;
#[path = "cost/mod.rs"]
mod cost;
#[path = "q01/support.rs"]
mod support;

use corpus::{COMMON_QUERY, CORPUS, SOURCES, body, marker, versions};
use cost::{Library, percentiles, sample};
use radishmemory_application::{
    EncryptedLibraryLocation, LocalLibrary, RecallProfile, SourceSearchResult,
};
use radishmemory_core::{Identifier, Sensitivity, SourceLineageSummary, SourceVersionSummary};
use radishmemory_file_entry::{FileCaptureReceipt, FileReadRequest};
use radishmemory_source_vault::{VerificationReport, acceptance::SyntheticLibraryProvider};
use radishmemory_sqlite::read_cost::{self, Observation};
use rusqlite::Connection;
use std::{collections::BTreeSet, fs};
use support::{Directory, Runtime, config};

const REPEATS: usize = 11;
const PAGE_SIZE: usize = 200;

struct Entry {
    index: usize,
    lineage: Identifier,
    sources: Vec<Identifier>,
}

impl Entry {
    fn current(&self) -> &Identifier {
        self.sources.last().unwrap()
    }
}

impl Library {
    fn import(&mut self, request: &FileReadRequest) -> FileCaptureReceipt {
        match self {
            Self::Plain(library) => library.import_new_source(request).unwrap(),
            Self::Encrypted(library) => {
                let capture = library.prepare_import_new_source(request).unwrap();
                library.capture_source(&capture).unwrap()
            }
        }
    }

    fn update(&mut self, lineage: &Identifier, request: &FileReadRequest) -> FileCaptureReceipt {
        match self {
            Self::Plain(library) => library.update_source(lineage, request).unwrap(),
            Self::Encrypted(library) => {
                let capture = library.prepare_update_source(lineage, request).unwrap();
                library.capture_source(&capture).unwrap()
            }
        }
    }

    fn catalog(&self) -> Vec<SourceLineageSummary> {
        match self {
            Self::Plain(library) => library.list_sources(0, PAGE_SIZE).unwrap(),
            Self::Encrypted(library) => library.list_sources(0, PAGE_SIZE).unwrap(),
        }
    }

    fn history(&self, lineage: &Identifier) -> Vec<SourceVersionSummary> {
        match self {
            Self::Plain(library) => library.list_source_versions(lineage).unwrap(),
            Self::Encrypted(library) => library.list_source_versions(lineage).unwrap(),
        }
    }

    fn maintain(&mut self, rebuild: bool) -> Option<VerificationReport> {
        match self {
            Self::Plain(library) => {
                if rebuild {
                    library.rebuild_recall().unwrap();
                } else {
                    library.verify_library().unwrap();
                }
                None
            }
            Self::Encrypted(library) => Some(if rebuild {
                library.rebuild_recall().unwrap()
            } else {
                library.verify_library().unwrap()
            }),
        }
    }
}

fn empty(root: &Directory, encrypted: bool) -> Library {
    let plain =
        LocalLibrary::open(root.0.join("library.sqlite3"), Runtime::default(), config()).unwrap();
    assert_eq!(plain.recall_profile().unwrap(), RecallProfile::HanPhraseV1);
    if !encrypted {
        return Library::Plain(plain);
    }
    let (runtime, config) = plain.close();
    let location = EncryptedLibraryLocation::with_provider(
        &root.0,
        config,
        SyntheticLibraryProvider::default(),
    )
    .unwrap();
    location.initialize_key().unwrap();
    location.migrate_bodies().unwrap();
    Library::Encrypted(location.open(runtime).unwrap())
}

fn request(root: &Directory, index: usize, version: usize) -> FileReadRequest {
    let path = root.0.join(format!("synthetic-q04-{index:03}.md"));
    fs::write(&path, body(index, version)).unwrap();
    FileReadRequest::new(path, vec![root.0.clone()]).unwrap()
}

fn summary(mode: &str, case: &str, cache: &str, observations: &[Observation], count: usize) {
    let (p50, p95) = percentiles(observations);
    println!(
        "Q04-SUMMARY mode={mode} profile=han case={case} cache={cache} samples={} percentile=nearest_rank p50_us={} p95_us={} hits={count}",
        observations.len(),
        p50.as_micros(),
        p95.as_micros()
    );
}

fn populate(root: &Directory, library: &mut Library, mode: &str, count: usize) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut imports = Vec::new();
    for index in 0..count {
        let request = request(root, index, 1);
        // Fixture generation / writing is outside the API timer.
        let (receipt, observation) = read_cost::observe(|| library.import(&request));
        assert_eq!(receipt.version().get(), 1);
        assert_eq!(receipt.content_length(), body(index, 1).len() as u64);
        entries.push(Entry {
            index,
            lineage: receipt.lineage_id().clone(),
            sources: vec![receipt.source_id().clone()],
        });
        let ordinal = imports.len() + 1;
        sample(mode, "han", "import", "growing", ordinal, &observation);
        println!(
            "Q04-WRITE mode={mode} case=import sample={ordinal} sources={} versions={} body_bytes={}",
            entries.len(),
            entries.len(),
            receipt.content_length()
        );
        imports.push(observation);
    }
    summary(mode, "import", "growing", &imports, count);
    let mut updates = Vec::new();
    for version in 2..=3 {
        for entry in &mut entries {
            if versions(entry.index) < version {
                continue;
            }
            let request = request(root, entry.index, version);
            let (receipt, observation) =
                read_cost::observe(|| library.update(&entry.lineage, &request));
            assert_eq!(receipt.lineage_id(), &entry.lineage);
            assert_eq!(receipt.version().get(), version as u64);
            assert_eq!(
                receipt.content_length(),
                body(entry.index, version).len() as u64
            );
            assert!(!entry.sources.contains(receipt.source_id()));
            entry.sources.push(receipt.source_id().clone());
            let ordinal = updates.len() + 1;
            sample(mode, "han", "update", "growing", ordinal, &observation);
            println!(
                "Q04-WRITE mode={mode} case=update sample={ordinal} sources={count} versions={} body_bytes={}",
                count + ordinal,
                receipt.content_length()
            );
            updates.push(observation);
        }
    }
    summary(mode, "update", "growing", &updates, updates.len());
    entries
}

fn check_catalog(actual: &[SourceLineageSummary], entries: &[Entry]) -> usize {
    assert_eq!(actual.len(), entries.len());
    let mut seen = BTreeSet::new();
    for source in actual {
        let expected = entries
            .iter()
            .find(|entry| &entry.lineage == source.lineage_id())
            .unwrap();
        assert!(seen.insert(source.lineage_id()));
        assert_eq!(source.current_source_id(), expected.current());
        assert_eq!(
            source.current_version().get(),
            versions(expected.index) as u64
        );
        assert_eq!(source.version_count(), expected.sources.len() as u64);
        assert_eq!(
            source.content_length(),
            body(expected.index, versions(expected.index)).len() as u64
        );
    }
    actual.len()
}

fn check_history(actual: &[SourceVersionSummary], entry: &Entry) -> usize {
    assert_eq!(actual.len(), entry.sources.len());
    let mut seen = BTreeSet::new();
    for source in actual {
        let version = source.version().get() as usize;
        assert!(seen.insert(version));
        assert_eq!(source.lineage_id(), &entry.lineage);
        assert_eq!(source.source_id(), &entry.sources[version - 1]);
        assert_eq!(source.current(), version == entry.sources.len());
        assert_eq!(
            source.content_length(),
            body(entry.index, version).len() as u64
        );
    }
    actual.len()
}

struct Query {
    label: &'static str,
    text: String,
    top_k: usize,
    allowed: Sensitivity,
    // Some(index) restricts the lineage; hits == 0 requires an empty result.
    expected: Option<usize>,
    hits: usize,
}

fn queries(count: usize) -> Vec<Query> {
    vec![
        Query {
            label: "han_one",
            text: COMMON_QUERY.into(),
            top_k: 1,
            allowed: Sensitivity::Personal,
            expected: None,
            hits: 1,
        },
        Query {
            label: "han_five",
            text: COMMON_QUERY.into(),
            top_k: 5,
            allowed: Sensitivity::Personal,
            expected: None,
            hits: 5.min(count),
        },
        Query {
            label: "current_long",
            text: marker(0, 3),
            top_k: 5,
            allowed: Sensitivity::Personal,
            expected: Some(0),
            hits: 1,
        },
        Query {
            label: "current_last",
            text: marker(count - 1, versions(count - 1)),
            top_k: 5,
            allowed: Sensitivity::Personal,
            expected: Some(count - 1),
            hits: 1,
        },
        Query {
            label: "old_version",
            text: marker(0, 1),
            top_k: 5,
            allowed: Sensitivity::Personal,
            expected: None,
            hits: 0,
        },
        Query {
            label: "absent",
            text: "q04nomatchzz".into(),
            top_k: 5,
            allowed: Sensitivity::Personal,
            expected: None,
            hits: 0,
        },
        Query {
            label: "denied",
            text: COMMON_QUERY.into(),
            top_k: 5,
            allowed: Sensitivity::Restricted,
            expected: None,
            hits: 0,
        },
    ]
}

fn check_hits(actual: &[SourceSearchResult], query: &Query, entries: &[Entry]) -> usize {
    assert_eq!(actual.len(), query.hits, "case={}", query.label);
    let mut seen = BTreeSet::new();
    for hit in actual {
        let entry = entries
            .iter()
            .find(|entry| entry.current() == hit.source_id())
            .unwrap();
        if let Some(expected) = query.expected {
            assert_eq!(entry.index, expected);
        }
        assert_eq!(hit.lineage_id(), &entry.lineage);
        assert_eq!(hit.version().get(), versions(entry.index) as u64);
        assert!(seen.insert(hit.source_id()));
        let expected = body(entry.index, versions(entry.index));
        assert_eq!(hit.byte_start(), 0);
        assert_eq!(hit.byte_end(), expected.len() as u64);
        assert_eq!(hit.content().as_str(), expected);
    }
    actual.len()
}

fn check_maintenance(
    report: &Option<VerificationReport>,
    entries: &[Entry],
    rebuild: bool,
) -> usize {
    if let Some(report) = report {
        assert_eq!(
            report.committed_objects_verified,
            entries
                .iter()
                .map(|entry| entry.sources.len())
                .sum::<usize>()
        );
        assert_eq!(report.pending_captures, 0);
        assert_eq!(report.abandoned_attempts, 0);
        assert!(!report.abandonment_pending);
        assert_eq!(report.deletion_pending_objects, 0);
        assert_eq!(report.deleted_objects, 0);
        assert_eq!(report.derivations_rebuilt, rebuild);
        assert_eq!(report.recall_profile, RecallProfile::HanPhraseV1);
    }
    // The unit-returning plain API has already returned a real Ok, not a runner gate.
    1
}

fn repeat<T>(
    root: &Directory,
    mut library: Library,
    mode: &str,
    case: &str,
    mut operation: impl FnMut(&mut Library) -> T,
    check: impl Fn(&T) -> usize,
) -> Library {
    library = library.reopen(root);
    let mut observations = Vec::new();
    let mut count = 0;
    for index in 0..=REPEATS {
        let (result, observation) = read_cost::observe(|| operation(&mut library));
        count = check(&result); // Oracle / result comparisons are outside the API timer.
        let cache = if index == 0 {
            "reopened_first"
        } else {
            "repeated"
        };
        sample(mode, "han", case, cache, index, &observation);
        if index != 0 {
            observations.push(observation);
        }
    }
    summary(mode, case, "repeated", &observations, count);
    library
}

fn measure_open(root: &Directory, mut library: Library, mode: &str) -> Library {
    let mut observations = Vec::new();
    for index in 0..=REPEATS {
        let closed = library.close();
        let (opened, observation) = read_cost::observe(|| closed.open(root));
        library = opened;
        sample(mode, "han", "open", "reopened", index, &observation);
        if index != 0 {
            observations.push(observation);
        }
    }
    summary(mode, "open", "reopened", &observations, 1);
    library
}

fn storage(root: &Directory, mode: &str, entries: &[Entry]) {
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
    let facts: i64 = connection
        .query_row(
            "SELECT count(*) FROM radishmemory_source_artifacts",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let total_versions = entries
        .iter()
        .map(|entry| entry.sources.len())
        .sum::<usize>();
    assert_eq!(facts, total_versions as i64);
    let object_dir = root.0.join("source-objects-v1");
    let object_bytes: u64 = if object_dir.exists() {
        let files = fs::read_dir(object_dir)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                assert!(entry.file_type().unwrap().is_file());
                entry.metadata().unwrap().len()
            })
            .collect::<Vec<_>>();
        assert_eq!(files.len(), total_versions);
        files.into_iter().sum()
    } else {
        0
    };
    println!(
        "Q04-STORAGE mode={mode} profile=han corpus={CORPUS} sources={} versions={total_versions} database_bytes={} used_page_bytes={} object_bytes={object_bytes}",
        entries.len(),
        pages * size,
        (pages - free) * size
    );
}

fn verify_queries(library: &mut Library, entries: &[Entry], mode: &str, phase: &str) {
    for query in queries(entries.len()) {
        let hits = library.search(&query.text, query.top_k, query.allowed);
        check_hits(&hits, &query, entries);
    }
    println!(
        "Q04-QUALITY mode={mode} phase={phase} sources={} queries=7 current_versions=matched old_version_hits=0 denied_hits=0 absent_hits=0 exact_bytes=matched",
        entries.len()
    );
}

#[test]
#[ignore = "bounded 100-source release baseline; not a performance gate"]
fn q04_hundred_sources_mixed_sizes_and_versions_release_baseline() {
    if cfg!(debug_assertions) {
        panic!("run this measurement with --release");
    }
    println!(
        "Q04-RUN corpus={CORPUS} arch={} os={} profile=release sources=100 versions=160 short_bytes=768 short_sources=75 long_bytes=16384 long_sources=25 latest_bytes=467200 all_version_bytes=747520 samples={REPEATS} provider=synthetic cache_policy=no_os_cache_flush",
        std::env::consts::ARCH,
        std::env::consts::OS
    );
    for encrypted in [false, true] {
        let root = Directory::new();
        let mode = if encrypted { "encrypted" } else { "plain" };
        let (mut library, setup) = read_cost::observe(|| empty(&root, encrypted));
        sample(mode, "han", "setup", "empty", 0, &setup);
        let entries = populate(&root, &mut library, mode, SOURCES);
        check_catalog(&library.catalog(), &entries);
        check_maintenance(&library.maintain(false), &entries, false);
        verify_queries(&mut library, &entries, mode, "imported");
        storage(&root, mode, &entries);
        library = measure_open(&root, library, mode);
        library = repeat(
            &root,
            library,
            mode,
            "catalog",
            |library| library.catalog(),
            |actual| check_catalog(actual, &entries),
        );
        library = repeat(
            &root,
            library,
            mode,
            "versions",
            |library| library.history(&entries[0].lineage),
            |actual| check_history(actual, &entries[0]),
        );
        for query in queries(SOURCES) {
            library = repeat(
                &root,
                library,
                mode,
                query.label,
                |library| library.search(&query.text, query.top_k, query.allowed),
                |actual| check_hits(actual, &query, &entries),
            );
        }
        verify_queries(&mut library, &entries, mode, "reopened");
        library = repeat(
            &root,
            library,
            mode,
            "verify",
            |library| library.maintain(false),
            |report| check_maintenance(report, &entries, false),
        );
        library = repeat(
            &root,
            library,
            mode,
            "rebuild",
            |library| library.maintain(true),
            |report| check_maintenance(report, &entries, true),
        );
        check_catalog(&library.catalog(), &entries);
        verify_queries(&mut library, &entries, mode, "rebuilt");
        storage(&root, mode, &entries);
    }
}

#[test]
fn q04_small_multiversion_fixture_preserves_current_history_and_denial_after_rebuild() {
    for encrypted in [false, true] {
        let root = Directory::new();
        let mode = if encrypted { "encrypted" } else { "plain" };
        let mut library = empty(&root, encrypted);
        let entries = populate(&root, &mut library, mode, 6);
        check_catalog(&library.catalog(), &entries);
        for entry in &entries {
            check_history(&library.history(&entry.lineage), entry);
        }
        verify_queries(&mut library, &entries, mode, "imported");
        library = library.reopen(&root);
        verify_queries(&mut library, &entries, mode, "reopened");
        check_maintenance(&library.maintain(true), &entries, true);
        check_catalog(&library.catalog(), &entries);
        verify_queries(&mut library, &entries, mode, "rebuilt");
    }
}
