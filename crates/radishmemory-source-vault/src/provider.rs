use std::fmt;

use zeroize::{Zeroize, Zeroizing};

use crate::random::{RandomSource, SystemRandom};
use crate::{KEY_BYTES, KeyEncryptionKey, SourceVaultError, SourceVaultErrorCode};

#[cfg(target_os = "macos")]
#[path = "provider/macos.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "provider/windows.rs"]
mod platform;
#[cfg(target_os = "linux")]
#[path = "provider/linux.rs"]
mod platform;

#[cfg(test)]
#[path = "provider/tests.rs"]
mod tests;

const SERVICE: &str = "io.github.laugh0608.RadishMemory.source-vault";
const LABEL: &str = "RadishMemory Source Vault key";
const VALUE_PREFIX: &[u8] = b"rmkek1:";
const VALUE_BYTES: usize = VALUE_PREFIX.len() + KEY_BYTES * 2;
type Result<T> = std::result::Result<T, SourceVaultError>;

/// A device-local slot derived from a verified host profile.
///
/// Construction validates identifier syntax only. It does not establish library
/// ownership, bootstrap eligibility, or permission to create a key.
#[derive(Clone, Eq, PartialEq)]
pub struct KeySlot {
    account: String,
    #[cfg(any(target_os = "windows", test))]
    windows_target: String,
}

impl KeySlot {
    pub fn new(namespace_id: &str, device_id: &str) -> Result<Self> {
        if !valid_id(namespace_id, "namespace-") || !valid_id(device_id, "device-") {
            return Err(failure(
                SourceVaultErrorCode::InvalidKeySlot,
                "validate key slot",
            ));
        }
        Ok(Self {
            account: format!("v1:{namespace_id}:{device_id}"),
            #[cfg(any(target_os = "windows", test))]
            windows_target: format!(
                "io.github.laugh0608.RadishMemory/source-vault/v1/{namespace_id}/{device_id}"
            ),
        })
    }
}

impl fmt::Debug for KeySlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KeySlot([REDACTED])")
    }
}

fn valid_id(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|suffix| {
        suffix.len() == 32
            && suffix
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// Explicit platform key-store access. Construction performs no OS operation.
///
/// `load_existing` may contact the session service or prompt the user. It must
/// run off the UI thread after host authorization. Before OS access, the host's
/// logger must suppress sensitive upstream targets (including `keyring_core`);
/// enabling their debug/trace output can disclose credential identifiers.
/// Creation is available only through the SQLite-coordinated library initializer;
/// there is no public slot-only creator, setter, or deletion API.
///
/// ```compile_fail
/// use radishmemory_source_vault::{KeySlot, PlatformKeyProvider};
/// let slot = KeySlot::new(
///     "namespace-0123456789abcdef0123456789abcdef",
///     "device-fedcba9876543210fedcba9876543210",
/// ).unwrap();
/// PlatformKeyProvider::new().create_if_absent_for_bootstrap(&slot);
/// ```
#[derive(Debug, Default)]
pub struct PlatformKeyProvider;

impl PlatformKeyProvider {
    pub const fn new() -> Self {
        Self
    }

    pub fn load_existing(&self, slot: &KeySlot) -> Result<KeyEncryptionKey> {
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            load_existing(&platform::Store::connect(slot)?)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = slot;
            Err(failure(
                SourceVaultErrorCode::KeyStoreUnavailable,
                "unsupported key store platform",
            ))
        }
    }

    /// Prepares the dedicated library's key checkpoint under its SQLite writer
    /// lock. May prompt/write the real OS key store. The host must supply verified
    /// profile identities, suspend ordinary operations, and suppress sensitive
    /// logging before explicitly invoking this maintenance operation.
    pub fn initialize_library_key(
        &self,
        directory: &crate::ObjectDirectory,
        namespace_id: &str,
        device_id: &str,
    ) -> std::result::Result<KeyEncryptionKey, crate::KeyInitializationError> {
        let slot = KeySlot::new(namespace_id, device_id)?;
        crate::bootstrap::initialize(directory, namespace_id, device_id, |initialized| {
            #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
            {
                let store = platform::Store::connect(&slot)?;
                load_or_bootstrap(&store, initialized, &mut SystemRandom)
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            {
                let _ = (&slot, initialized);
                Err(failure(
                    SourceVaultErrorCode::KeyStoreUnavailable,
                    "unsupported key store platform",
                ))
            }
        })
    }
}

// Each read checks actual identity/persistence/default collection before and
// after reading. Implementations never treat attribute access failure as absence.
pub(crate) trait KeyStore {
    fn read_secret(&self) -> Result<Zeroizing<Vec<u8>>>;
    fn validate_label(&self) -> Result<()>;
    fn write_secret(&self, value: &[u8]) -> Result<()>;
    fn set_label(&self) -> Result<()>;
}

pub(crate) fn load_or_bootstrap(
    store: &impl KeyStore,
    initialized: bool,
    random: &mut impl RandomSource,
) -> Result<KeyEncryptionKey> {
    if initialized {
        load_existing(store)
    } else {
        bootstrap(store, random)
    }
}

fn load_existing(store: &impl KeyStore) -> Result<KeyEncryptionKey> {
    let value = store.read_secret()?;
    let key = decode(&value)?;
    store.validate_label()?;
    let recheck = store.read_secret()?;
    if value.as_slice() != recheck.as_slice() {
        return Err(failure(
            SourceVaultErrorCode::KeyReadbackMismatch,
            "recheck key value",
        ));
    }
    store.validate_label()?;
    Ok(key)
}

fn bootstrap(store: &impl KeyStore, random: &mut impl RandomSource) -> Result<KeyEncryptionKey> {
    let expected = match store.read_secret() {
        Ok(value) => {
            decode(&value)?;
            value
        }
        Err(error) if error.code() == SourceVaultErrorCode::KeyMissing => {
            let mut bytes = Zeroizing::new([0; KEY_BYTES]);
            random.fill(bytes.as_mut())?;
            let generated = encode(&bytes);
            // Narrow the upsert race; this recheck cannot replace the required
            // cross-process SQLite IMMEDIATE transaction in the caller.
            match store.read_secret() {
                Ok(value) => {
                    decode(&value)?;
                    value
                }
                Err(error) if error.code() == SourceVaultErrorCode::KeyMissing => {
                    store.write_secret(&generated)?;
                    generated
                }
                Err(error) => return Err(error),
            }
        }
        Err(error) => return Err(error),
    };
    // Do not mutate decorations on a credential that changed after the read.
    if store.read_secret()?.as_slice() != expected.as_slice() {
        return Err(failure(
            SourceVaultErrorCode::KeyReadbackMismatch,
            "verify bootstrap key write",
        ));
    }
    match store.validate_label() {
        Ok(()) => {}
        Err(error) if error.code() == SourceVaultErrorCode::KeyMetadataMismatch => {
            store.set_label()?
        }
        Err(error) => return Err(error),
    }
    let key = load_existing(store)?;
    if store.read_secret()?.as_slice() != expected.as_slice() {
        return Err(failure(
            SourceVaultErrorCode::KeyReadbackMismatch,
            "verify bootstrap key value",
        ));
    }
    Ok(key)
}

fn encode(bytes: &[u8; KEY_BYTES]) -> Zeroizing<Vec<u8>> {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut value = Zeroizing::new(Vec::with_capacity(VALUE_BYTES));
    value.extend_from_slice(VALUE_PREFIX);
    for byte in bytes {
        value.push(HEX[(byte >> 4) as usize]);
        value.push(HEX[(byte & 15) as usize]);
    }
    value
}

fn decode(value: &[u8]) -> Result<KeyEncryptionKey> {
    let invalid = || failure(SourceVaultErrorCode::KeyCorrupt, "decode key value");
    if value.len() != VALUE_BYTES || !value.starts_with(VALUE_PREFIX) {
        return Err(invalid());
    }
    let mut key = Zeroizing::new([0; KEY_BYTES]);
    for (i, pair) in value[VALUE_PREFIX.len()..].chunks_exact(2).enumerate() {
        let digit = |b| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            _ => None,
        };
        key[i] = digit(pair[0]).ok_or_else(invalid)? * 16 + digit(pair[1]).ok_or_else(invalid)?;
    }
    Ok(KeyEncryptionKey::new(*key))
}

fn failure(code: SourceVaultErrorCode, operation: &'static str) -> SourceVaultError {
    SourceVaultError::provider(code, operation, None)
}

fn keyring_error(mut error: keyring_core::Error, operation: &'static str) -> SourceVaultError {
    map_keyring_error(&mut error, operation)
}

fn map_keyring_error(error: &mut keyring_core::Error, operation: &'static str) -> SourceVaultError {
    use keyring_core::Error;
    let code = match error {
        Error::NoEntry => SourceVaultErrorCode::KeyMissing,
        Error::Ambiguous(_) => SourceVaultErrorCode::KeyAmbiguous,
        Error::BadEncoding(bytes) | Error::BadDataFormat(bytes, _) => {
            bytes.zeroize();
            SourceVaultErrorCode::KeyCorrupt
        }
        Error::NoStorageAccess(source) | Error::PlatformFailure(source) => {
            #[cfg(target_os = "macos")]
            if let Some(error) = source.downcast_ref::<security_framework::base::Error>() {
                return platform::os_error(error.code(), operation);
            }
            #[cfg(target_os = "linux")]
            if let Some(error) = source.downcast_ref::<secret_service::Error>() {
                return platform::service_error(error, operation);
            }
            let _ = source;
            SourceVaultErrorCode::KeyStoreFailure
        }
        _ => SourceVaultErrorCode::KeyStoreFailure,
    };
    failure(code, operation)
}

fn check_label(label: &str) -> Result<()> {
    if label == LABEL {
        Ok(())
    } else {
        Err(failure(
            SourceVaultErrorCode::KeyMetadataMismatch,
            "verify key label",
        ))
    }
}
