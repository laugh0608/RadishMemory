use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::error::Error;

use super::*;
use crate::test_support::metadata;
use crate::{open_object, seal_object};

const NAMESPACE: &str = "namespace-0123456789abcdef0123456789abcdef";
const DEVICE: &str = "device-fedcba9876543210fedcba9876543210";
const ENCODED: &[u8] = b"rmkek1:000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

enum Read {
    Value(Vec<u8>),
    Fail(SourceVaultErrorCode),
}

#[derive(Default)]
struct Store {
    value: RefCell<Option<Vec<u8>>>,
    reads: RefCell<VecDeque<Read>>,
    write_count: Cell<usize>,
    label_count: Cell<usize>,
    label: RefCell<String>,
    write_failure: Cell<Option<SourceVaultErrorCode>>,
    label_failure: Cell<Option<SourceVaultErrorCode>>,
    written_override: RefCell<Option<Vec<u8>>>,
}

impl Store {
    fn valid() -> Self {
        Self {
            value: RefCell::new(Some(ENCODED.to_vec())),
            label: RefCell::new(LABEL.into()),
            ..Self::default()
        }
    }
    fn script(&self, reads: impl IntoIterator<Item = Read>) {
        self.reads.borrow_mut().extend(reads);
    }
    fn assert_no_writes(&self) {
        assert_eq!((self.write_count.get(), self.label_count.get()), (0, 0));
    }
}

impl KeyStore for Store {
    fn read_secret(&self) -> Result<Zeroizing<Vec<u8>>> {
        if let Some(read) = self.reads.borrow_mut().pop_front() {
            return match read {
                Read::Value(v) => Ok(Zeroizing::new(v)),
                Read::Fail(c) => Err(failure(c, "synthetic read")),
            };
        }
        self.value
            .borrow()
            .clone()
            .map(Zeroizing::new)
            .ok_or_else(|| failure(SourceVaultErrorCode::KeyMissing, "synthetic read"))
    }
    fn validate_label(&self) -> Result<()> {
        check_label(&self.label.borrow())
    }
    fn write_secret(&self, value: &[u8]) -> Result<()> {
        self.write_count.set(self.write_count.get() + 1);
        if let Some(code) = self.write_failure.get() {
            return Err(failure(code, "synthetic write"));
        }
        *self.value.borrow_mut() = Some(
            self.written_override
                .borrow_mut()
                .take()
                .unwrap_or_else(|| value.to_vec()),
        );
        Ok(())
    }
    fn set_label(&self) -> Result<()> {
        self.label_count.set(self.label_count.get() + 1);
        if let Some(code) = self.label_failure.get() {
            return Err(failure(code, "synthetic label"));
        }
        *self.label.borrow_mut() = LABEL.into();
        Ok(())
    }
}

#[derive(Default)]
struct Random {
    calls: usize,
    fail: bool,
}
impl RandomSource for Random {
    fn fill(&mut self, destination: &mut [u8]) -> Result<()> {
        self.calls += 1;
        for (i, byte) in destination.iter_mut().enumerate() {
            *byte = i as u8;
        }
        if self.fail {
            Err(failure(
                SourceVaultErrorCode::RandomSourceUnavailable,
                "synthetic random",
            ))
        } else {
            Ok(())
        }
    }
}

#[test]
fn slot_uses_exact_versioned_identity_and_rejects_ambiguous_inputs() {
    let slot = KeySlot::new(NAMESPACE, DEVICE).unwrap();
    assert_eq!(
        slot.account,
        "v1:namespace-0123456789abcdef0123456789abcdef:device-fedcba9876543210fedcba9876543210"
    );
    assert_eq!(
        slot.windows_target,
        "io.github.laugh0608.RadishMemory/source-vault/v1/namespace-0123456789abcdef0123456789abcdef/device-fedcba9876543210fedcba9876543210"
    );
    for invalid in [
        "",
        "namespace-",
        "namespace-0123456789abcdef0123456789abcde",
        "namespace-0123456789abcdef0123456789abcdef0",
        "namespace-0123456789ABCDEF0123456789abcdef",
        "namespace-0123456789abcdef0123456789abcde/",
        "namespace-0123456789abcdef0123456789abcde:",
        "namespace-0123456789abcdef0123456789abcdeé",
        DEVICE,
    ] {
        assert_eq!(
            KeySlot::new(invalid, DEVICE).unwrap_err().code(),
            SourceVaultErrorCode::InvalidKeySlot
        );
    }
    for invalid in [
        "",
        NAMESPACE,
        "device-0123456789abcdef0123456789abcdef\n",
        "device-0123456789abcdef0123456789abcde\0",
    ] {
        assert!(KeySlot::new(NAMESPACE, invalid).is_err());
    }
}

#[test]
fn value_known_answer_is_ascii_and_decoded_key_opens_existing_ciphertext() {
    let bytes = std::array::from_fn(|i| i as u8);
    assert_eq!(encode(&bytes).as_slice(), ENCODED);
    let decoded = decode(ENCODED).unwrap();
    let plaintext = b"synthetic provider compatibility";
    let metadata = metadata(plaintext);
    let object = seal_object(&KeyEncryptionKey::new(bytes), &metadata, plaintext).unwrap();
    assert_eq!(
        open_object(&decoded, &metadata, &object).unwrap(),
        plaintext
    );
}

#[test]
fn corrupt_values_reject_noncanonical_encodings() {
    let mut bad = vec![
        Vec::new(),
        ENCODED[..69].to_vec(),
        [ENCODED, b"\n"].concat(),
        [b" ", ENCODED].concat(),
        ENCODED.iter().flat_map(|b| [*b, 0]).collect(),
        ENCODED.to_ascii_uppercase(),
        vec![0; 32],
    ];
    for index in [0, 5, 6, 69] {
        for byte in [b'G', b'/', 0, 0xff, b' ', b'\n'] {
            let mut value = ENCODED.to_vec();
            value[index] = byte;
            bad.push(value);
        }
    }
    for value in bad {
        assert_eq!(
            decode(&value).unwrap_err().code(),
            SourceVaultErrorCode::KeyCorrupt
        );
    }
}

#[test]
fn load_only_reads_and_does_not_repair_missing_or_bad_metadata() {
    let store = Store::valid();
    load_existing(&store).unwrap();
    store.assert_no_writes();
    *store.label.borrow_mut() = "synthetic unexpected label".into();
    assert_eq!(
        load_existing(&store).unwrap_err().code(),
        SourceVaultErrorCode::KeyMetadataMismatch
    );
    store.assert_no_writes();
    *store.value.borrow_mut() = None;
    assert_eq!(
        load_existing(&store).unwrap_err().code(),
        SourceVaultErrorCode::KeyMissing
    );
    store.assert_no_writes();
}

#[test]
fn load_detects_key_replacement_during_metadata_checks() {
    let store = Store::valid();
    store.script([
        Read::Value(ENCODED.to_vec()),
        Read::Value(encode(&[0x44; 32]).to_vec()),
    ]);
    assert_eq!(
        load_existing(&store).unwrap_err().code(),
        SourceVaultErrorCode::KeyReadbackMismatch
    );
    store.assert_no_writes();
}

#[test]
fn bootstrap_reuses_existing_key_and_repairs_only_label() {
    let store = Store::valid();
    store.label.borrow_mut().clear();
    let mut random = Random {
        fail: true,
        ..Random::default()
    };
    bootstrap(&store, &mut random).unwrap();
    assert_eq!(random.calls, 0);
    assert_eq!(store.write_count.get(), 0);
    assert_eq!(store.value.borrow().as_deref(), Some(ENCODED));
    load_existing(&store).unwrap();
}

#[test]
fn bootstrap_missing_key_creates_once_and_retry_reuses_after_label_failure() {
    let store = Store::default();
    store
        .label_failure
        .set(Some(SourceVaultErrorCode::KeyStoreDenied));
    let mut random = Random::default();
    assert_eq!(
        bootstrap(&store, &mut random).unwrap_err().code(),
        SourceVaultErrorCode::KeyStoreDenied
    );
    assert_eq!(store.write_count.get(), 1);
    assert_eq!(store.value.borrow().as_deref(), Some(ENCODED));
    store.label_failure.set(None);
    bootstrap(&store, &mut random).unwrap();
    assert_eq!(store.write_count.get(), 1);
    assert_eq!(random.calls, 1);
    load_existing(&store).unwrap();
}

#[test]
fn bootstrap_recheck_reuses_competing_key_without_upsert() {
    let store = Store::valid();
    store.script([Read::Fail(SourceVaultErrorCode::KeyMissing)]);
    let mut random = Random::default();
    bootstrap(&store, &mut random).unwrap();
    assert_eq!(random.calls, 1);
    assert_eq!(store.write_count.get(), 0);
}

#[test]
fn bootstrap_never_writes_on_non_missing_failures_or_corrupt_values() {
    for code in [
        SourceVaultErrorCode::KeyAmbiguous,
        SourceVaultErrorCode::KeyCorrupt,
        SourceVaultErrorCode::KeyMetadataMismatch,
        SourceVaultErrorCode::KeyStoreLocked,
        SourceVaultErrorCode::KeyStoreDenied,
        SourceVaultErrorCode::KeyStoreCancelled,
        SourceVaultErrorCode::KeyStoreUnavailable,
        SourceVaultErrorCode::KeyStoreFailure,
    ] {
        let store = Store::valid();
        store.script([Read::Fail(code)]);
        let mut random = Random::default();
        assert_eq!(bootstrap(&store, &mut random).unwrap_err().code(), code);
        assert_eq!(random.calls, 0);
        store.assert_no_writes();
        store.script([
            Read::Fail(SourceVaultErrorCode::KeyMissing),
            Read::Fail(code),
        ]);
        assert_eq!(bootstrap(&store, &mut random).unwrap_err().code(), code);
        store.assert_no_writes();
    }
    let store = Store::valid();
    *store.value.borrow_mut() = Some(b"synthetic corrupt value".to_vec());
    assert_eq!(
        bootstrap(&store, &mut Random::default())
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::KeyCorrupt
    );
    store.assert_no_writes();
}

#[test]
fn bootstrap_reports_random_write_and_readback_failure_without_label_update() {
    let store = Store::default();
    assert_eq!(
        bootstrap(
            &store,
            &mut Random {
                fail: true,
                ..Random::default()
            }
        )
        .unwrap_err()
        .code(),
        SourceVaultErrorCode::RandomSourceUnavailable
    );
    store.assert_no_writes();
    store
        .write_failure
        .set(Some(SourceVaultErrorCode::KeyStoreDenied));
    assert_eq!(
        bootstrap(&store, &mut Random::default())
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::KeyStoreDenied
    );
    assert_eq!(store.label_count.get(), 0);
    store.write_failure.set(None);
    *store.written_override.borrow_mut() = Some(encode(&[0x44; 32]).to_vec());
    assert_eq!(
        bootstrap(&store, &mut Random::default())
            .unwrap_err()
            .code(),
        SourceVaultErrorCode::KeyReadbackMismatch
    );
    assert_eq!(store.label_count.get(), 0);
}

#[derive(Debug)]
struct SensitiveSource;
impl fmt::Display for SensitiveSource {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic!("must not format upstream error")
    }
}
impl Error for SensitiveSource {}

#[test]
fn upstream_error_payloads_are_cleared_and_sources_not_exposed() {
    for mut raw in [
        keyring_core::Error::BadEncoding(ENCODED.to_vec()),
        keyring_core::Error::BadDataFormat(ENCODED.to_vec(), Box::new(SensitiveSource)),
    ] {
        let error = map_keyring_error(&mut raw, "synthetic operation");
        assert_eq!(error.code(), SourceVaultErrorCode::KeyCorrupt);
        match raw {
            keyring_core::Error::BadEncoding(bytes)
            | keyring_core::Error::BadDataFormat(bytes, _) => assert!(bytes.is_empty()),
            _ => unreachable!(),
        }
        assert!(error.source().is_none());
        assert!(!format!("{error:?} {error}").contains("rmkek1"));
    }
    for raw in [
        keyring_core::Error::NoStorageAccess(Box::new(SensitiveSource)),
        keyring_core::Error::PlatformFailure(Box::new(SensitiveSource)),
    ] {
        let error = keyring_error(raw, "synthetic operation");
        assert_eq!(error.code(), SourceVaultErrorCode::KeyStoreFailure);
        assert!(error.source().is_none());
    }
    assert_eq!(
        keyring_error(keyring_core::Error::Ambiguous(vec![]), "synthetic read").code(),
        SourceVaultErrorCode::KeyAmbiguous
    );
}

#[test]
fn public_construction_and_debug_have_no_system_side_effects_or_identity() {
    let slot = KeySlot::new(NAMESPACE, DEVICE).unwrap();
    assert_eq!(format!("{slot:?}"), "KeySlot([REDACTED])");
    assert_eq!(
        format!("{:?}", PlatformKeyProvider::new()),
        "PlatformKeyProvider"
    );
    let key = decode(ENCODED).unwrap();
    assert_eq!(format!("{key:?}"), "KeyEncryptionKey([REDACTED])");
}
