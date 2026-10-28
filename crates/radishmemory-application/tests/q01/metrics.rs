//! Shared counting rules for production observations and isolated candidates.
use std::collections::BTreeSet;

#[derive(Debug, Default, Eq, PartialEq)]
pub struct Metrics {
    pub relevant: usize,
    pub retrieved: usize,
    pub true_positives: usize,
    pub false_positives: usize,
    pub answered_queries: usize,
    pub hit_queries: usize,
    pub exact_queries: usize,
    pub no_answer_queries: usize,
    pub no_answer_false_hits: usize,
}

impl Metrics {
    pub fn add(&mut self, expected: &[&str], actual: &[&str]) {
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
