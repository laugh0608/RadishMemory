//! Isolated FTS experiment. None of these transforms is used by production.
//! The small fixed corpus is a design aid, not a quality or migration gate.
#[path = "q01/corpus.rs"]
mod corpus;
#[path = "q01/metrics.rs"]
mod metrics;

use radishmemory_application as _;
use radishmemory_core as _;
use radishmemory_file_entry as _;
use radishmemory_source_vault as _;
use radishmemory_sqlite as _;
use rusqlite::{Connection, params};

use corpus::{CORPUS_VERSION, DOCUMENTS, QUERIES, TOP_K};
use metrics::Metrics;

const BOUNDARY: &str = "q01hanboundary";

#[derive(Clone, Copy, Debug)]
enum Candidate {
    Unicode61,
    Trigram,
    HanAnd,
    HanPhrase,
}

// Intentionally bounded prototype: basic Han and extension A only. Production
// requires a separately reviewed Unicode repertoire and normalization policy.
fn is_han(character: char) -> bool {
    matches!(character, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}')
}

fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\""))
}

fn runs(text: &str) -> Vec<(bool, String)> {
    let mut result: Vec<(bool, String)> = Vec::new();
    for character in text.chars() {
        let han = is_han(character);
        if let Some((last_han, run)) = result.last_mut()
            && *last_han == han
        {
            run.push(character);
        } else {
            result.push((han, character.to_string()));
        }
    }
    result
}

fn spaced_characters(text: &str) -> String {
    text.chars()
        .map(|character| character.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn fields(body: &str) -> (String, String) {
    let han = runs(body)
        .into_iter()
        .filter(|(han, _)| *han)
        .map(|(_, run)| spaced_characters(&run))
        .collect::<Vec<_>>()
        .join(&format!(" {BOUNDARY} "));
    // Keep Han runs as unqueryable barriers in the words column. Removing them
    // would make Latin phrases accidentally bridge intervening Han text.
    let words = runs(body)
        .into_iter()
        .map(|(han, run)| if han { format!(" {run} ") } else { run })
        .collect();
    (han, words)
}

impl Candidate {
    fn expression(self, query: &str) -> Option<String> {
        let terms: Vec<String> = match self {
            Self::Unicode61 | Self::Trigram => query.split_whitespace().map(quoted).collect(),
            Self::HanAnd | Self::HanPhrase => runs(query)
                .into_iter()
                .flat_map(|(han, run)| {
                    if han {
                        match self {
                            Self::HanAnd => run
                                .chars()
                                .map(|character| {
                                    format!("han : {}", quoted(&character.to_string()))
                                })
                                .collect(),
                            _ => vec![format!("han : {}", quoted(&spaced_characters(&run)))],
                        }
                    } else {
                        run.split_whitespace()
                            .filter(|word| word.chars().any(char::is_alphanumeric))
                            .map(|word| format!("words : {}", quoted(word)))
                            .collect::<Vec<_>>()
                    }
                })
                .collect(),
        };
        (!terms.is_empty()).then(|| terms.join(" AND "))
    }
}

struct Index {
    connection: Connection,
    candidate: Candidate,
}

impl Index {
    fn new(candidate: Candidate, documents: &[(&str, &str)]) -> Self {
        let connection = Connection::open_in_memory().unwrap();
        let sql = match candidate {
            Candidate::Unicode61 => {
                "CREATE VIRTUAL TABLE candidates USING fts5(label UNINDEXED, body, tokenize='unicode61')"
            }
            Candidate::Trigram => {
                "CREATE VIRTUAL TABLE candidates USING fts5(label UNINDEXED, body, tokenize='trigram')"
            }
            Candidate::HanAnd | Candidate::HanPhrase => {
                "CREATE VIRTUAL TABLE candidates USING fts5(label UNINDEXED, han, words, tokenize='unicode61')"
            }
        };
        connection.execute_batch(sql).unwrap();
        for (label, body) in documents {
            match candidate {
                Candidate::Unicode61 | Candidate::Trigram => connection
                    .execute(
                        "INSERT INTO candidates(label, body) VALUES (?1, ?2)",
                        params![label, body],
                    )
                    .unwrap(),
                Candidate::HanAnd | Candidate::HanPhrase => {
                    let (han, words) = fields(body);
                    connection
                        .execute(
                            "INSERT INTO candidates(label, han, words) VALUES (?1, ?2, ?3)",
                            params![label, han, words],
                        )
                        .unwrap()
                }
            };
        }
        Self {
            connection,
            candidate,
        }
    }

    fn search(&self, query: &str, limit: usize) -> Vec<String> {
        let Some(expression) = self.candidate.expression(query) else {
            return vec![];
        };
        self.connection
            .prepare("SELECT label FROM candidates WHERE candidates MATCH ?1 ORDER BY bm25(candidates), label LIMIT ?2")
            .unwrap()
            .query_map(params![expression, i64::try_from(limit).unwrap()], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }
}

#[test]
fn q01_compare_candidates_without_changing_relevance_labels() {
    let documents = DOCUMENTS
        .iter()
        .map(|document| (document.label, document.body))
        .collect::<Vec<_>>();
    for candidate in [
        Candidate::Unicode61,
        Candidate::Trigram,
        Candidate::HanAnd,
        Candidate::HanPhrase,
    ] {
        let index = Index::new(candidate, &documents);
        let mut metrics = Metrics::default();
        for query in QUERIES {
            let actual = index.search(query.text, TOP_K);
            let labels = actual.iter().map(String::as_str).collect::<Vec<_>>();
            metrics.add(query.relevant, &labels);
            println!(
                "Q01-CANDIDATE candidate={candidate:?} query={} category={} actual={labels:?}",
                query.label, query.category
            );
        }
        println!(
            "Q01-CANDIDATE-SUMMARY corpus={CORPUS_VERSION} sqlite={} candidate={candidate:?} top_k={TOP_K} metrics={metrics:?}",
            rusqlite::version()
        );
        // Assert the proposed candidate against all frozen labels. Controls are
        // measured, not required to keep failing if SQLite behavior evolves.
        if matches!(candidate, Candidate::HanPhrase) {
            assert_eq!(metrics.true_positives, metrics.relevant);
            assert_eq!(metrics.false_positives, 0);
            assert_eq!(metrics.exact_queries, QUERIES.len());
        }
    }
}

#[test]
fn q01_phrase_preserves_adjacency_order_and_repeated_characters() {
    let documents = [
        ("exact", "蓝色"),
        ("distant", "蓝天的颜色"),
        ("punctuation", "蓝，色"),
        ("space", "蓝 色"),
        ("reverse", "色蓝"),
        ("mixed", "蓝API色"),
        ("single", "哈"),
        ("double", "哈哈"),
    ];
    let phrase = Index::new(Candidate::HanPhrase, &documents);
    assert_eq!(phrase.search("蓝色", 20), ["exact"]);
    assert_eq!(phrase.search("哈哈", 20), ["double"]);
    assert_eq!(phrase.search("色蓝", 20), ["reverse"]);
    assert_eq!(phrase.search("哈", 20).len(), 2);
    let naive = Index::new(Candidate::HanAnd, &documents);
    let naive_blue = naive.search("蓝色", 20);
    assert_eq!(naive_blue.len(), 6);
    assert!(naive_blue.iter().any(|label| label == "distant"));
    assert!(
        naive
            .search("哈哈", 20)
            .iter()
            .any(|label| label == "single")
    );
    // Punctuation in a query separates concepts; a contiguous phrase does not.
    assert_eq!(phrase.search("蓝，色", 20).len(), 6);
}

#[test]
fn q01_trigram_does_not_resolve_short_terms_or_whole_latin_words() {
    let index = Index::new(
        Candidate::Trigram,
        &[("short", "蓝色"), ("api", "API"), ("plural", "APIs")],
    );
    assert!(index.search("蓝色", 20).is_empty());
    assert_eq!(index.search("API", 20).len(), 2);
}

#[test]
fn q01_phrase_has_separate_literal_word_space_and_no_fts_injection() {
    let documents = [
        ("mixed", "本地API接口使用UTF-8编码"),
        ("near", "APIs APIx RESTful"),
        ("marker", "q01hanboundary"),
        ("split", "甲，乙"),
        ("operator", "OR"),
        ("english", "café local storage"),
        ("latin-phrase", "foo-bar"),
        ("latin-split", "foo中文bar"),
    ];
    let index = Index::new(Candidate::HanPhrase, &documents);
    assert_eq!(index.search("api 接口", 20), ["mixed"]);
    assert_eq!(index.search("UTF-8", 20), ["mixed"]);
    assert!(index.search("REST", 20).is_empty());
    assert_eq!(index.search(BOUNDARY, 20), ["marker"]);
    assert_eq!(index.search("OR", 20), ["operator"]);
    assert!(index.search("API OR RESTful", 20).is_empty());
    assert_eq!(index.search("\" OR \"", 20), ["operator"]);
    assert!(index.search("words : *", 20).is_empty());
    assert_eq!(index.search("\"API\"", 20), ["mixed"]);
    assert!(index.search("\" : * - ，！", 20).is_empty());
    assert!(index.search("   ", 20).is_empty());
    assert_eq!(index.search("cafe", 20), ["english"]);
    assert_eq!(index.search("foo-bar", 20), ["latin-phrase"]);
    // Space-separated query terms intentionally require only co-occurrence.
    assert_eq!(index.search("foo bar", 20).len(), 2);
}
