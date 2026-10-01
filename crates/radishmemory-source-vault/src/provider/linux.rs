use std::collections::HashMap;

use keyring_core::{Entry, api::CredentialStoreApi};
use secret_service::{EncryptionType, blocking::SecretService};
use zbus_secret_service_keyring_store::cred::Specifier;
use zeroize::Zeroizing;

use super::{KeySlot, KeyStore, LABEL, Result, SERVICE, check_label, failure, keyring_error};
use crate::{SourceVaultError, SourceVaultErrorCode};

pub(super) struct Store {
    entry: Entry,
    guard: SecretService<'static>,
    default_path: String,
}

impl Store {
    pub(super) fn connect(slot: &KeySlot) -> Result<Self> {
        let guard = SecretService::connect(EncryptionType::Dh)
            .map_err(|e| service_error(&e, "connect default collection guard"))?;
        let default_path = default_collection(&guard)?;
        let store = zbus_secret_service_keyring_store::Store::new()
            .map_err(|e| keyring_error(e, "connect Secret Service provider"))?;
        let entry = store
            .build(
                SERVICE,
                &slot.account,
                Some(&HashMap::from([("label", LABEL)])),
            )
            .map_err(|e| keyring_error(e, "build key entry"))?;
        let result = Self {
            entry,
            guard,
            default_path,
        };
        result.check_default()?;
        Ok(result)
    }

    fn check_default(&self) -> Result<()> {
        if default_collection(&self.guard)? != self.default_path {
            return Err(failure(
                SourceVaultErrorCode::KeyMetadataMismatch,
                "default collection changed",
            ));
        }
        Ok(())
    }

    fn specifier(&self) -> Result<&Specifier> {
        self.entry
            .as_any()
            .downcast_ref::<Specifier>()
            .ok_or_else(|| {
                failure(
                    SourceVaultErrorCode::KeyStoreFailure,
                    "verify concrete Secret Service provider",
                )
            })
    }
}

fn default_collection(service: &SecretService<'_>) -> Result<String> {
    let collection = service
        .get_default_collection()
        .map_err(|e| service_error(&e, "read default collection"))?;
    // This is a read-only guard; do not unlock or create a replacement collection.
    collection
        .ensure_unlocked()
        .map_err(|e| service_error(&e, "check default collection access"))?;
    Ok(collection.collection_path.to_string())
}

impl KeyStore for Store {
    fn read_secret(&self) -> Result<Zeroizing<Vec<u8>>> {
        self.check_default()?;
        // Keep the provider's cross-collection exact service/username search.
        let value = Zeroizing::new(
            self.entry
                .get_secret()
                .map_err(|e| keyring_error(e, "read key secret"))?,
        );
        self.check_default()?;
        Ok(value)
    }

    fn validate_label(&self) -> Result<()> {
        self.check_default()?;
        let label = self
            .specifier()?
            .get_label()
            .map_err(|e| keyring_error(e, "read key label"))?;
        self.check_default()?;
        check_label(&label)
    }

    fn write_secret(&self, value: &[u8]) -> Result<()> {
        self.check_default()?;
        match self.read_secret() {
            Err(e) if e.code() == SourceVaultErrorCode::KeyMissing => {}
            Err(e) => return Err(e),
            Ok(_) => {
                return Err(failure(
                    SourceVaultErrorCode::KeyMetadataMismatch,
                    "key appeared before write",
                ));
            }
        }
        self.entry
            .set_secret(value)
            .map_err(|e| keyring_error(e, "write key secret"))?;
        self.check_default()
    }

    fn set_label(&self) -> Result<()> {
        self.check_default()?;
        self.specifier()?
            .set_label(LABEL)
            .map_err(|e| keyring_error(e, "write key label"))?;
        self.validate_label()
    }
}

pub(super) fn service_error(
    error: &secret_service::Error,
    operation: &'static str,
) -> SourceVaultError {
    let code = match error {
        secret_service::Error::Locked => SourceVaultErrorCode::KeyStoreLocked,
        secret_service::Error::Prompt => SourceVaultErrorCode::KeyStoreCancelled,
        secret_service::Error::Unavailable | secret_service::Error::NoResult => {
            SourceVaultErrorCode::KeyStoreUnavailable
        }
        _ => SourceVaultErrorCode::KeyStoreFailure,
    };
    failure(code, operation)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prompt_disconnect_is_not_reported_as_cancel_or_missing() {
        for (raw, expected) in [
            (
                secret_service::Error::Prompt,
                SourceVaultErrorCode::KeyStoreCancelled,
            ),
            (
                secret_service::Error::PromptDisconnected,
                SourceVaultErrorCode::KeyStoreFailure,
            ),
            (
                secret_service::Error::NoResult,
                SourceVaultErrorCode::KeyStoreUnavailable,
            ),
            (
                secret_service::Error::Locked,
                SourceVaultErrorCode::KeyStoreLocked,
            ),
        ] {
            assert_eq!(service_error(&raw, "synthetic operation").code(), expected);
        }
    }
}
