use super::*;
use security_framework::item::Reference;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

#[test]
fn os_status_mapping_preserves_codes_without_guessing_locked() {
    for (raw, expected) in [
        (-25300, SourceVaultErrorCode::KeyMissing),
        (-25299, SourceVaultErrorCode::KeyAmbiguous),
        (-128, SourceVaultErrorCode::KeyStoreCancelled),
        (-61, SourceVaultErrorCode::KeyStoreDenied),
        (-25244, SourceVaultErrorCode::KeyStoreDenied),
        (-25292, SourceVaultErrorCode::KeyStoreDenied),
        (-25293, SourceVaultErrorCode::KeyStoreDenied),
        (-25308, SourceVaultErrorCode::KeyStoreDenied),
        (-25291, SourceVaultErrorCode::KeyStoreUnavailable),
        (-25294, SourceVaultErrorCode::KeyStoreUnavailable),
        (-25295, SourceVaultErrorCode::KeyStoreUnavailable),
        (-9999, SourceVaultErrorCode::KeyStoreFailure),
    ] {
        let error = os_error(raw, "synthetic operation");
        assert_eq!(error.code(), expected);
        assert_eq!(error.os_code(), Some(raw));
    }
}

#[test]
fn single_data_moves_to_zeroizing_owner_and_keeps_decode_validation() {
    let bytes = crate::provider::encode(&[0x35; 32]).to_vec();
    let address = bytes.as_ptr();
    let mut matches = vec![SearchResult::Data(bytes)];
    let value = secret_result(&mut matches).unwrap();
    assert_eq!(value.as_ptr(), address);
    crate::provider::decode(&value).unwrap();
    assert!(matches!(&matches[0], SearchResult::Data(bytes) if bytes.is_empty()));

    for bytes in [vec![], b"synthetic corrupt data".to_vec()] {
        let value = secret_result(&mut [SearchResult::Data(bytes)]).unwrap();
        assert_eq!(
            crate::provider::decode(&value).unwrap_err().code(),
            SourceVaultErrorCode::KeyCorrupt
        );
    }
}

fn malformed_results() -> Vec<(Vec<SearchResult>, SourceVaultErrorCode)> {
    vec![
        (vec![], SourceVaultErrorCode::KeyStoreFailure),
        (
            vec![SearchResult::Other],
            SourceVaultErrorCode::KeyStoreFailure,
        ),
        (
            vec![SearchResult::Ref(Reference::__NonExhaustive)],
            SourceVaultErrorCode::KeyStoreFailure,
        ),
        (
            vec![
                SearchResult::Data(b"synthetic first secret".to_vec()),
                SearchResult::Other,
                SearchResult::Data(b"synthetic second secret".to_vec()),
            ],
            SourceVaultErrorCode::KeyAmbiguous,
        ),
    ]
}

#[test]
fn malformed_success_is_not_missing_and_cleans_every_data_result() {
    for (mut matches, expected) in malformed_results() {
        let error = secret_result(&mut matches).unwrap_err();
        assert_eq!(error.code(), expected);
        assert_eq!(error.os_code(), None);
        assert!(std::error::Error::source(&error).is_none());
        assert!(!format!("{error:?} {error}").contains("synthetic"));
        for item in &matches {
            if let SearchResult::Data(bytes) = item {
                assert!(bytes.is_empty());
            }
        }
    }
}

#[test]
fn result_guard_cleans_data_during_unwind() {
    let mut matches = vec![
        SearchResult::Data(b"synthetic first secret".to_vec()),
        SearchResult::Other,
        SearchResult::Data(b"synthetic second secret".to_vec()),
    ];
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = SecretResults(&mut matches);
        panic!("synthetic unwind");
    }));
    assert!(outcome.is_err());
    for item in &matches {
        if let SearchResult::Data(bytes) = item {
            assert!(bytes.is_empty());
        }
    }
}

#[test]
fn data_is_delivered_only_after_both_attribute_checks() {
    let steps = RefCell::new(Vec::new());
    let value = read_checked_secret(
        || {
            steps.borrow_mut().push("attributes");
            Ok(())
        },
        || {
            steps.borrow_mut().push("secret");
            Ok(vec![SearchResult::Data(b"synthetic value".to_vec())])
        },
    )
    .unwrap();
    assert_eq!(&*steps.borrow(), &["attributes", "secret", "attributes"]);
    assert_eq!(value.as_slice(), b"synthetic value");
}

#[test]
fn either_attribute_failure_withholds_secret_without_retry() {
    for failure in [
        SourceVaultErrorCode::KeyMissing,
        SourceVaultErrorCode::KeyAmbiguous,
        SourceVaultErrorCode::KeyMetadataMismatch,
        SourceVaultErrorCode::KeyStoreCancelled,
        SourceVaultErrorCode::KeyStoreDenied,
        SourceVaultErrorCode::KeyStoreUnavailable,
        SourceVaultErrorCode::KeyStoreFailure,
    ] {
        for position in [1, 2] {
            let checks = Cell::new(0);
            let reads = Cell::new(0);
            let error = read_checked_secret(
                || {
                    checks.set(checks.get() + 1);
                    if checks.get() == position {
                        Err(SourceVaultError::provider(
                            failure,
                            "synthetic check",
                            Some(-9999),
                        ))
                    } else {
                        Ok(())
                    }
                },
                || {
                    reads.set(reads.get() + 1);
                    Ok(vec![SearchResult::Data(b"synthetic value".to_vec())])
                },
            )
            .unwrap_err();
            assert_eq!(error.code(), failure);
            assert_eq!(error.os_code(), Some(-9999));
            assert_eq!(checks.get(), position);
            assert_eq!(reads.get(), position - 1);
        }
    }
}

#[test]
fn search_errors_and_malformed_success_stop_before_post_check() {
    let outcomes = [-25300, -25299, -128, -25308, -25291, -9999]
        .into_iter()
        .map(|raw| {
            let error = os_error(raw, "read key secret");
            let code = error.code();
            (Err(error), code, Some(raw))
        })
        .chain(
            malformed_results()
                .into_iter()
                .map(|(items, code)| (Ok(items), code, None)),
        );
    for (outcome, code, raw) in outcomes {
        let checks = Cell::new(0);
        let error = read_checked_secret(
            || {
                checks.set(checks.get() + 1);
                Ok(())
            },
            || outcome,
        )
        .unwrap_err();
        assert_eq!(checks.get(), 1);
        assert_eq!(error.code(), code);
        assert_eq!(error.os_code(), raw);
    }
}

// Exercise real bootstrap control flow with synthetic SecItem results only.
struct BootstrapStore(RefCell<VecDeque<Result<Vec<SearchResult>>>>);

impl KeyStore for BootstrapStore {
    fn read_secret(&self) -> Result<Zeroizing<Vec<u8>>> {
        read_checked_secret(|| Ok(()), || self.0.borrow_mut().pop_front().unwrap())
    }

    fn validate_label(&self) -> Result<()> {
        Ok(())
    }

    fn write_secret(&self, _: &[u8]) -> Result<()> {
        panic!("malformed read must not write a key");
    }

    fn set_label(&self) -> Result<()> {
        panic!("malformed read must not repair a label");
    }
}

#[derive(Default)]
struct Random(usize);

impl crate::random::RandomSource for Random {
    fn fill(&mut self, destination: &mut [u8]) -> Result<()> {
        self.0 += 1;
        destination.fill(0xa5);
        Ok(())
    }
}

#[test]
fn malformed_results_never_authorize_bootstrap_or_write_recheck() {
    // Existing-key bootstrap makes five reads, including its final readback.
    for position in 1..=5 {
        for (items, code) in malformed_results() {
            let reads = (1..position)
                .map(|_| {
                    Ok(vec![SearchResult::Data(
                        crate::provider::encode(&[0xa5; 32]).to_vec(),
                    )])
                })
                .chain(std::iter::once(Ok(items)))
                .collect();
            let store = BootstrapStore(RefCell::new(reads));
            let mut random = Random::default();
            let error = crate::provider::bootstrap(&store, &mut random).unwrap_err();
            assert_eq!(error.code(), code);
            assert_eq!(random.0, 0);
            assert!(store.0.borrow().is_empty());
        }
    }
    for (items, code) in malformed_results() {
        let store = BootstrapStore(RefCell::new(VecDeque::from([
            Err(os_error(-25300, "read key secret")),
            Ok(items),
        ])));
        let mut random = Random::default();
        assert_eq!(
            crate::provider::bootstrap(&store, &mut random)
                .unwrap_err()
                .code(),
            code
        );
        assert_eq!(random.0, 1);
        assert!(store.0.borrow().is_empty());
    }
}

// All OS-facing helpers below are the production helpers; only their OS closures
// are synthetic. No Keychain construction or metadata API is called.
struct ObservedStore {
    calls: Cell<usize>,
    fail_at: Option<usize>,
    domains: Cell<usize>,
    attributes: Cell<usize>,
    secrets: Cell<usize>,
}
impl ObservedStore {
    fn step(&self) -> Result<()> {
        self.calls.set(self.calls.get() + 1);
        if self.fail_at == Some(self.calls.get()) {
            Err(os_error(-25308, "synthetic-private-marker"))
        } else {
            Ok(())
        }
    }
    fn domain(&self) -> Result<()> {
        self.domains.set(self.domains.get() + 1);
        self.step()
    }
    fn attributes(&self) -> Result<()> {
        checked_attributes(
            || self.domain(),
            || {
                self.attributes.set(self.attributes.get() + 1);
                self.step()?;
                Ok(vec![SearchResult::Other])
            },
        )
        .map(|_| ())
    }
}
impl KeyStore for ObservedStore {
    fn read_secret(&self) -> Result<Zeroizing<Vec<u8>>> {
        read_checked_secret(
            || self.attributes(),
            || {
                self.secrets.set(self.secrets.get() + 1);
                self.step()?;
                Ok(vec![SearchResult::Data(
                    crate::provider::encode(&[0xa5; 32]).to_vec(),
                )])
            },
        )
    }
    fn validate_label(&self) -> Result<()> {
        call(Operation::AttributesLabel, || self.attributes())
    }
    fn write_secret(&self, _: &[u8]) -> Result<()> {
        panic!("no writes")
    }
    fn set_label(&self) -> Result<()> {
        panic!("no writes")
    }
}
fn observed_snapshot_sequence(store: &ObservedStore) -> Result<()> {
    call(Operation::Entry, || {
        call(Operation::Domain, || store.domain())?;
        crate::provider::load_existing(store)
    })?;
    call(Operation::Release, || crate::provider::load_existing(store))?;
    Ok(())
}
fn observed_store(fail_at: Option<usize>) -> ObservedStore {
    ObservedStore {
        calls: Cell::new(0),
        fail_at,
        domains: Cell::new(0),
        attributes: Cell::new(0),
        secrets: Cell::new(0),
    }
}
#[test]
fn observed_real_helpers_keep_four_twelve_twenty_five_and_stop_at_every_os_failure() {
    use crate::key_read_observation::{TraceFailure, tests::recording};
    let store = observed_store(None);
    let (result, log) = recording(None, None, || observed_snapshot_sequence(&store));
    result.unwrap();
    assert_eq!(
        (
            store.secrets.get(),
            store.attributes.get(),
            store.domains.get()
        ),
        (4, 12, 25)
    );
    let text = String::from_utf8(log.borrow().bytes.clone()).unwrap();
    assert!(!text.contains("synthetic"));
    assert_eq!(
        text.matches("\"operation\":\"SecretSearch\",\"edge\":\"begin\"")
            .count(),
        4
    );
    let lines = text.lines().count();
    let system_begins = text
        .lines()
        .map(|line| {
            line.contains("\"edge\":\"begin\"")
                && ["Domain", "AttributesSearch", "SecretSearch"]
                    .iter()
                    .any(|operation| line.contains(&format!("\"operation\":\"{operation}\"")))
        })
        .collect::<Vec<_>>();
    for at in 1..=41 {
        let store = observed_store(Some(at));
        let (result, log) = recording(None, None, || observed_snapshot_sequence(&store));
        let Err(TraceFailure::Business(error)) = result else {
            panic!("business failure lost")
        };
        assert_eq!(error.code(), SourceVaultErrorCode::KeyStoreDenied);
        assert_eq!(store.calls.get(), at);
        let text = String::from_utf8(log.borrow().bytes.clone()).unwrap();
        assert!(!text.contains("synthetic-private-marker"));
        assert!(text.contains("\"result\":\"denied\""));
    }
    for flush in [false, true] {
        for at in 1..=lines {
            let store = observed_store(None);
            let (result, _) = recording((!flush).then_some(at), flush.then_some(at), || {
                observed_snapshot_sequence(&store)
            });
            assert!(result.is_err(), "event {at}");
            let calls = store.calls.get();
            let expected = system_begins
                .iter()
                .take(at - 1)
                .filter(|begin| **begin)
                .count();
            assert_eq!(calls, expected, "fault at event {at}, flush={flush}");
        }
    }
}
#[test]
fn real_secret_helper_guards_results_before_failed_end_and_never_reaches_post_check() {
    use crate::key_read_observation::tests::recording;
    // Action begin, pre begin/end, secret begin/end: fail secret end.
    for malformed in [false, true] {
        let checks = Cell::new(0);
        let cleanup = std::rc::Rc::new(Cell::new((0, true)));
        CLEANUP.with(|current| *current.borrow_mut() = Some(cleanup.clone()));
        let (result, log) = recording(Some(5), None, || {
            read_checked_secret(
                || {
                    checks.set(checks.get() + 1);
                    Ok(())
                },
                || {
                    Ok(if malformed {
                        vec![
                            SearchResult::Data(b"synthetic-private-one".to_vec()),
                            SearchResult::Data(b"synthetic-private-two".to_vec()),
                        ]
                    } else {
                        vec![SearchResult::Data(b"synthetic-private-one".to_vec())]
                    })
                },
            )
        });
        assert!(result.is_err());
        assert_eq!(checks.get(), 1);
        CLEANUP.with(|current| current.borrow_mut().take());
        assert_eq!(cleanup.get(), (if malformed { 2 } else { 1 }, true));
        assert!(
            !String::from_utf8(log.borrow().bytes.clone())
                .unwrap()
                .contains("synthetic")
        );
    }
}

type CleanupRecord = std::rc::Rc<Cell<(usize, bool)>>;
thread_local! {
    static CLEANUP: RefCell<Option<CleanupRecord>> = const { RefCell::new(None) };
}
pub(super) fn record_secret_cleanup(results: &[SearchResult]) {
    CLEANUP.with(|cleanup| {
        if let Some(cleanup) = cleanup.borrow().as_ref() {
            let (mut count, mut clear) = cleanup.get();
            for result in results {
                if let SearchResult::Data(bytes) = result {
                    count += 1;
                    clear &= bytes.is_empty();
                }
            }
            cleanup.set((count, clear));
        }
    });
}
