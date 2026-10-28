//! A measurement baseline, not a Q01 quality gate. Test success establishes
//! reproducible observations and safety checks; Q01 gaps are printed explicitly.
#[path = "q01/corpus.rs"]
mod corpus;
#[path = "q01/support.rs"]
mod support;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

use radishmemory_application::{
    ApplicationError, EncryptedLibraryLocation, LocalLibrary, SourceSearchResult,
};
use radishmemory_core::{Identifier, NonEmptyText, Sensitivity};
use radishmemory_file_entry::{FileCaptureReceipt, FileReadRequest};
use radishmemory_source_vault::acceptance::SyntheticLibraryProvider;
use radishmemory_sqlite as _;
use rusqlite::{Connection, OpenFlags};

use corpus::{CORPUS_VERSION, DOCUMENTS, QUERIES, TOP_K};
use support::{Directory, Runtime, config};

type SourceLabels = BTreeMap<Identifier, &'static str>;
type Observation = Vec<Vec<&'static str>>;

#[derive(Debug, Default, Eq, PartialEq)]
struct Metrics {
    relevant: usize,
    retrieved: usize,
    true_positives: usize,
    false_positives: usize,
    answered_queries: usize,
    hit_queries: usize,
    exact_queries: usize,
    no_answer_queries: usize,
    no_answer_false_hits: usize,
}

impl Metrics {
    fn add(&mut self, expected: &[&str], actual: &[&str]) {
        let expected = expected.iter().copied().collect::<BTreeSet<_>>();
        let actual = actual.iter().copied().collect::<BTreeSet<_>>();
        let tp = expected.intersection(&actual).count();
        self.relevant += expected.len();
        self.retrieved += actual.len();
        self.true_positives += tp;
        self.false_positives += actual.difference(&expected).count();
        self.exact_queries += usize::from(actual == expected);
        if expected.is_empty() {
            self.no_answer_queries += 1;
            self.no_answer_false_hits += usize::from(!actual.is_empty());
        } else {
            self.answered_queries += 1;
            self.hit_queries += usize::from(tp > 0);
        }
    }
}

fn remember(labels: &mut SourceLabels, label: &'static str, receipt: &FileCaptureReceipt) {
    assert_eq!(receipt.version().get(), 1);
    assert!(labels.insert(receipt.source_id().clone(), label).is_none());
}

fn inputs(directory: &Directory) -> Vec<FileReadRequest> {
    let root = directory.0.join("input");
    fs::create_dir(&root).unwrap();
    DOCUMENTS
        .iter()
        .map(|document| {
            let path = root.join(format!("{}.md", document.label));
            fs::write(&path, document.body).unwrap();
            FileReadRequest::new(&path, vec![root.clone()]).unwrap()
        })
        .collect()
}

fn assert_storage_mode(directory: &Directory, version: u32, inline_bodies: usize, objects: usize) {
    // Read only, after closing the application connection, with no fixture DML.
    let connection = Connection::open_with_flags(
        directory.0.join("library.sqlite3"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        version
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM radishmemory_source_bodies",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        i64::try_from(inline_bodies).unwrap()
    );
    if objects > 0 {
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM radishmemory_source_vault_references",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            i64::try_from(objects).unwrap()
        );
    }
}

fn observe(
    mode: &str,
    phase: &str,
    labels: &SourceLabels,
    mut search: impl FnMut(
        NonEmptyText,
        usize,
        Vec<Sensitivity>,
    ) -> Result<Vec<SourceSearchResult>, ApplicationError>,
) -> Observation {
    let mut metrics = Metrics::default();
    let mut observations = Vec::new();
    for query in QUERIES {
        // Pass only the unmodified query. Relevance labels never reach search.
        let hits = search(
            NonEmptyText::new(query.text).unwrap(),
            TOP_K,
            vec![Sensitivity::Personal],
        )
        .unwrap();
        assert!(hits.len() <= TOP_K);
        let actual = hits
            .iter()
            .map(|hit| {
                let label = *labels
                    .get(hit.source_id())
                    .expect("unknown source returned");
                let document = DOCUMENTS
                    .iter()
                    .find(|document| document.label == label)
                    .unwrap();
                assert_eq!(hit.version().get(), 1);
                assert_eq!(hit.byte_start(), 0);
                assert_eq!(hit.byte_end(), u64::try_from(document.body.len()).unwrap());
                assert_eq!(hit.content().as_str(), document.body);
                label
            })
            .collect::<Vec<_>>();
        assert_eq!(actual.len(), actual.iter().collect::<BTreeSet<_>>().len());
        let tp = query
            .relevant
            .iter()
            .filter(|label| actual.contains(label))
            .count();
        let extras = actual
            .iter()
            .filter(|label| !query.relevant.contains(label))
            .count();
        metrics.add(query.relevant, &actual);
        println!(
            "Q01 case mode={mode} phase={phase} id={} category={} expected={} actual={} tp={tp} fp={extras}",
            query.label,
            query.category,
            query.relevant.join(","),
            actual.join(",")
        );
        // A separate authorized-sensitivity set must not reveal Personal data.
        assert!(
            search(
                NonEmptyText::new(query.text).unwrap(),
                TOP_K,
                vec![Sensitivity::Restricted]
            )
            .unwrap()
            .is_empty()
        );
        observations.push(actual);
    }
    let quality = if metrics.exact_queries == QUERIES.len() {
        "all_labels_matched"
    } else {
        "gaps_observed"
    };
    println!(
        "Q01 summary corpus={CORPUS_VERSION} mode={mode} phase={phase} top_k={TOP_K} documents={} queries={} tp={} relevant={} retrieved={} fp={} hit_queries={} answered_queries={} exact_queries={} no_answer_false_hits={} no_answer_queries={} quality={quality}",
        DOCUMENTS.len(),
        QUERIES.len(),
        metrics.true_positives,
        metrics.relevant,
        metrics.retrieved,
        metrics.false_positives,
        metrics.hit_queries,
        metrics.answered_queries,
        metrics.exact_queries,
        metrics.no_answer_false_hits,
        metrics.no_answer_queries
    );
    observations
}

#[test]
fn q01_production_baseline_reports_quality_separately_from_reproducibility() {
    let plain_root = Directory::new();
    let requests = inputs(&plain_root);
    let database = plain_root.0.join("library.sqlite3");
    let mut plain = LocalLibrary::open(&database, Runtime::default(), config()).unwrap();
    let mut plain_labels = SourceLabels::new();
    for (document, request) in DOCUMENTS.iter().zip(&requests) {
        remember(
            &mut plain_labels,
            document.label,
            &plain.import_new_source(request).unwrap(),
        );
    }
    plain.verify_library().unwrap();
    let baseline = observe("v6", "imported", &plain_labels, |q, k, s| {
        plain.search_sources(q, k, s)
    });
    // Positive controls keep the denied-sensitivity checks non-vacuous. These
    // are required lexical behavior, not assertions that known CJK misses stay.
    for (query_id, expected) in [("q03", "d01"), ("q18", "d11"), ("q19", "d11")] {
        let position = QUERIES
            .iter()
            .position(|query| query.label == query_id)
            .unwrap();
        assert_eq!(baseline[position], vec![expected]);
    }
    let (runtime, config) = plain.close();
    assert_storage_mode(&plain_root, 6, DOCUMENTS.len(), 0);
    let mut plain = LocalLibrary::open(&database, runtime, config.clone()).unwrap();
    assert_eq!(
        baseline,
        observe("v6", "reopened", &plain_labels, |q, k, s| plain
            .search_sources(q, k, s))
    );
    plain.rebuild_recall().unwrap();
    plain.verify_library().unwrap();
    assert_eq!(
        baseline,
        observe("v6", "rebuilt", &plain_labels, |q, k, s| plain
            .search_sources(q, k, s))
    );
    drop(plain.close());

    let encrypted_root = Directory::new();
    let requests = inputs(&encrypted_root);
    let empty = LocalLibrary::open(
        encrypted_root.0.join("library.sqlite3"),
        Runtime::default(),
        config.clone(),
    )
    .unwrap();
    let (runtime, config) = empty.close();
    let provider = SyntheticLibraryProvider::default();
    let location =
        EncryptedLibraryLocation::with_provider(&encrypted_root.0, config, provider.clone())
            .unwrap();
    location.initialize_key().unwrap();
    assert_eq!(location.migrate_bodies().unwrap().objects_verified, 0);
    let mut encrypted = location.open(runtime).unwrap();
    let mut encrypted_labels = SourceLabels::new();
    for (document, request) in DOCUMENTS.iter().zip(&requests) {
        let capture = encrypted.prepare_import_new_source(request).unwrap();
        remember(
            &mut encrypted_labels,
            document.label,
            &encrypted.capture_source(&capture).unwrap(),
        );
    }
    assert_eq!(
        baseline,
        observe("encrypted", "imported", &encrypted_labels, |q, k, s| {
            encrypted.search_sources(q, k, s)
        })
    );
    let (runtime, location) = encrypted.close();
    assert_storage_mode(&encrypted_root, 9, 0, DOCUMENTS.len());
    let mut encrypted = location.open(runtime).unwrap();
    assert_eq!(
        baseline,
        observe("encrypted", "reopened", &encrypted_labels, |q, k, s| {
            encrypted.search_sources(q, k, s)
        })
    );
    let (runtime, location) = encrypted.close();
    location.rebuild_recall().unwrap();
    location.verify_library().unwrap();
    let mut encrypted = location.open(runtime).unwrap();
    assert_eq!(
        baseline,
        observe("encrypted", "rebuilt", &encrypted_labels, |q, k, s| {
            encrypted.search_sources(q, k, s)
        })
    );
    assert_eq!(provider.key_creations(), 1);
    drop(encrypted.close());
}

#[test]
fn q01_metrics_distinguish_misses_extras_and_no_answer_queries() {
    let mut metrics = Metrics::default();
    metrics.add(&["a", "b"], &["a", "c"]);
    metrics.add(&["d"], &[]);
    metrics.add(&[], &["c"]);
    metrics.add(&[], &[]);
    assert_eq!(
        metrics,
        Metrics {
            relevant: 3,
            retrieved: 3,
            true_positives: 1,
            false_positives: 2,
            answered_queries: 2,
            hit_queries: 1,
            exact_queries: 1,
            no_answer_queries: 2,
            no_answer_false_hits: 1,
        }
    );
}

#[test]
fn q01_fixture_labels_are_unique_resolvable_and_fit_fixed_top_k() {
    let labels = DOCUMENTS
        .iter()
        .map(|document| document.label)
        .collect::<BTreeSet<_>>();
    assert_eq!(labels.len(), DOCUMENTS.len());
    let queries = QUERIES
        .iter()
        .map(|query| query.label)
        .collect::<BTreeSet<_>>();
    assert_eq!(queries.len(), QUERIES.len());
    for query in QUERIES {
        assert!(query.relevant.len() <= TOP_K);
        assert_eq!(
            query.relevant.len(),
            query.relevant.iter().collect::<BTreeSet<_>>().len()
        );
        assert!(query.relevant.iter().all(|label| labels.contains(label)));
        assert!(!query.text.trim().is_empty());
    }
}
