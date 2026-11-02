//! Bounded, content-free read timing for the opt-in synthetic acceptance harness.
//! Default builds have no clock, collector, output, or runtime switch. Instrumented
//! builds collect only fixed stages on the observing thread, in memory. Guards
//! never change a business result or retain a query, source, path, key, or callback.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    ReaderOpen,
    KeyLoad,
    ObjectAuthentication,
    CanonicalValidation,
    CatalogLoad,
    DerivedValidation,
    QueryPreparation,
    FtsMatch,
    ResultResolution,
    SnapshotRefresh,
}

impl Stage {
    pub const ALL: [Self; 10] = [
        Self::ReaderOpen,
        Self::KeyLoad,
        Self::ObjectAuthentication,
        Self::CanonicalValidation,
        Self::CatalogLoad,
        Self::DerivedValidation,
        Self::QueryPreparation,
        Self::FtsMatch,
        Self::ResultResolution,
        Self::SnapshotRefresh,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ReaderOpen => "reader_open",
            Self::KeyLoad => "key_load",
            Self::ObjectAuthentication => "object_authentication",
            Self::CanonicalValidation => "canonical_validation",
            Self::CatalogLoad => "catalog_load",
            Self::DerivedValidation => "derived_validation",
            Self::QueryPreparation => "query_preparation",
            Self::FtsMatch => "fts_match",
            Self::ResultResolution => "result_resolution",
            Self::SnapshotRefresh => "snapshot_refresh",
        }
    }
}

#[cfg(not(feature = "acceptance-test-support"))]
pub struct Span;

#[cfg(not(feature = "acceptance-test-support"))]
impl Drop for Span {
    #[inline]
    fn drop(&mut self) {}
}

#[cfg(not(feature = "acceptance-test-support"))]
#[inline]
#[must_use]
pub fn span(_: Stage) -> Span {
    Span
}

#[cfg(feature = "acceptance-test-support")]
pub use enabled::{Observation, Span, StageCost, observe, span};

#[cfg(feature = "acceptance-test-support")]
mod enabled {
    use super::Stage;
    use std::{
        cell::RefCell,
        marker::PhantomData,
        rc::Rc,
        time::{Duration, Instant},
    };

    #[derive(Clone, Copy, Debug, Default)]
    pub struct StageCost {
        pub calls: u64,
        pub inclusive: Duration,
        pub exclusive: Duration,
    }

    pub struct Observation {
        pub elapsed: Duration,
        costs: [StageCost; Stage::ALL.len()],
    }

    impl Observation {
        #[must_use]
        pub fn cost(&self, stage: Stage) -> StageCost {
            self.costs[stage as usize]
        }
    }

    struct Frame {
        stage: Stage,
        started: Instant,
        children: Duration,
    }

    struct Collector {
        costs: [StageCost; Stage::ALL.len()],
        stack: Vec<Frame>,
    }

    thread_local! {
        static ACTIVE: RefCell<Option<Collector>> = const { RefCell::new(None) };
    }

    /// Not Send: a span must finish on the thread whose collector it entered.
    pub struct Span {
        active: bool,
        _thread: PhantomData<Rc<()>>,
    }

    #[must_use]
    pub fn span(stage: Stage) -> Span {
        let active = ACTIVE.with_borrow_mut(|active| {
            let Some(collector) = active else {
                return false;
            };
            collector.stack.push(Frame {
                stage,
                started: Instant::now(),
                children: Duration::ZERO,
            });
            true
        });
        Span {
            active,
            _thread: PhantomData,
        }
    }

    impl Drop for Span {
        fn drop(&mut self) {
            if !self.active {
                return;
            }
            ACTIVE.with_borrow_mut(|active| {
                let collector = active.as_mut().expect("active read-cost scope");
                let frame = collector.stack.pop().expect("balanced read-cost spans");
                let elapsed = frame.started.elapsed();
                let cost = &mut collector.costs[frame.stage as usize];
                cost.calls += 1;
                cost.inclusive += elapsed;
                cost.exclusive += elapsed
                    .checked_sub(frame.children)
                    .expect("nested duration");
                if let Some(parent) = collector.stack.last_mut() {
                    parent.children += elapsed;
                }
            });
        }
    }

    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ACTIVE.with_borrow_mut(|active| {
                active.take();
            });
        }
    }

    /// One synchronous operation, including its original error value. Nested
    /// observations are harness errors; unwind clears the collector for reuse.
    pub fn observe<T>(operation: impl FnOnce() -> T) -> (T, Observation) {
        ACTIVE.with_borrow_mut(|active| {
            assert!(active.is_none(), "nested read-cost observation");
            *active = Some(Collector {
                costs: [StageCost::default(); Stage::ALL.len()],
                stack: Vec::new(),
            });
        });
        let _reset = Reset;
        let started = Instant::now();
        let result = operation();
        let elapsed = started.elapsed();
        let collector = ACTIVE.with_borrow_mut(|active| active.take().expect("read-cost scope"));
        assert!(collector.stack.is_empty(), "unfinished read-cost span");
        (
            result,
            Observation {
                elapsed,
                costs: collector.costs,
            },
        )
    }
}

#[cfg(all(test, feature = "acceptance-test-support"))]
mod tests {
    use super::*;

    #[test]
    fn nested_and_repeated_spans_have_disjoint_exclusive_costs() {
        let (_, observation) = observe(|| {
            let _parent = span(Stage::ResultResolution);
            for _ in 0..3 {
                let _child = span(Stage::ObjectAuthentication);
                std::hint::black_box((0..100).sum::<u64>());
            }
        });
        let parent = observation.cost(Stage::ResultResolution);
        let child = observation.cost(Stage::ObjectAuthentication);
        assert_eq!(parent.calls, 1);
        assert_eq!(child.calls, 3);
        assert_eq!(parent.inclusive, parent.exclusive + child.inclusive);
        assert!(observation.elapsed >= parent.inclusive);
    }

    #[test]
    fn original_errors_and_unwind_leave_no_stale_observation() {
        let (result, observation) = observe(|| {
            let _span = span(Stage::CanonicalValidation);
            Err::<(), _>("original error")
        });
        assert_eq!(result, Err("original error"));
        assert_eq!(observation.cost(Stage::CanonicalValidation).calls, 1);
        assert!(
            std::panic::catch_unwind(|| observe(|| {
                let _span = span(Stage::FtsMatch);
                panic!("synthetic unwind");
            }))
            .is_err()
        );
        let (_, clean) = observe(|| ());
        assert!(
            Stage::ALL
                .into_iter()
                .all(|stage| clean.cost(stage).calls == 0)
        );
    }

    #[test]
    fn observations_are_thread_local_and_reject_nested_collection() {
        let (_, observation) = observe(|| {
            std::thread::spawn(|| {
                let _span = span(Stage::FtsMatch);
            })
            .join()
            .unwrap();
            assert!(std::panic::catch_unwind(|| observe(|| ())).is_err());
            let _span = span(Stage::ReaderOpen);
        });
        assert_eq!(observation.cost(Stage::FtsMatch).calls, 0);
        assert_eq!(observation.cost(Stage::ReaderOpen).calls, 1);
    }
}
