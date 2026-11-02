//! Shared ownership and content-free reporting for explicit manual cost tests.
use super::support::{Directory, Runtime};
use radishmemory_application::{
    EncryptedLibrary, EncryptedLibraryLocation, LocalLibrary, LocalLibraryConfig,
    SourceSearchResult,
};
use radishmemory_core::{NonEmptyText, Sensitivity};
use radishmemory_source_vault::acceptance::SyntheticLibraryProvider;
use radishmemory_sqlite::read_cost::{Observation, Stage};
use std::time::Duration;

pub enum Library {
    Plain(LocalLibrary<Runtime>),
    Encrypted(EncryptedLibrary<Runtime, SyntheticLibraryProvider>),
}

pub enum ClosedLibrary {
    Plain(Runtime, LocalLibraryConfig),
    Encrypted(Runtime, EncryptedLibraryLocation<SyntheticLibraryProvider>),
}

impl Library {
    pub fn close(self) -> ClosedLibrary {
        match self {
            Self::Plain(library) => {
                let (runtime, config) = library.close();
                ClosedLibrary::Plain(runtime, config)
            }
            Self::Encrypted(library) => {
                let (runtime, location) = library.close();
                ClosedLibrary::Encrypted(runtime, location)
            }
        }
    }

    pub fn reopen(self, root: &Directory) -> Self {
        self.close().open(root)
    }

    pub fn search(
        &mut self,
        query: &str,
        top_k: usize,
        allowed: Sensitivity,
    ) -> Vec<SourceSearchResult> {
        let query = NonEmptyText::new(query).unwrap();
        match self {
            Self::Plain(library) => library.search_sources(query, top_k, [allowed]),
            Self::Encrypted(library) => library.search_sources(query, top_k, [allowed]),
        }
        .unwrap()
    }
}

impl ClosedLibrary {
    pub fn open(self, root: &Directory) -> Library {
        match self {
            Self::Plain(runtime, config) => Library::Plain(
                LocalLibrary::open(root.0.join("library.sqlite3"), runtime, config).unwrap(),
            ),
            Self::Encrypted(runtime, location) => {
                Library::Encrypted(location.open(runtime).unwrap())
            }
        }
    }
}

pub fn sample(
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

pub fn percentiles(observations: &[Observation]) -> (Duration, Duration) {
    assert!(!observations.is_empty());
    let mut times = observations
        .iter()
        .map(|observation| observation.elapsed)
        .collect::<Vec<_>>();
    times.sort();
    (
        times[times.len().div_ceil(2) - 1],
        times[(times.len() * 95).div_ceil(100) - 1],
    )
}
