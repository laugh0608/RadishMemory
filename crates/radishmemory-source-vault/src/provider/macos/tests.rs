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
