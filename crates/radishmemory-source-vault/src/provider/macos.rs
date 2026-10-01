use apple_native_keyring_store::keychain::{Cred, MacKeychainDomain};
use keyring_core::api::CredentialApi;
use security_framework::item::{
    ItemClass, ItemSearchOptions, ItemUpdateOptions, Limit, update_item,
};
use security_framework::os::macos::keychain::{SecKeychain, SecPreferencesDomain};
use zeroize::Zeroizing;

use super::{KeySlot, KeyStore, LABEL, Result, SERVICE, check_label, failure, keyring_error};
use crate::{SourceVaultError, SourceVaultErrorCode};

pub(super) struct Store {
    credential: Cred,
    keychain: SecKeychain,
}

impl Store {
    pub(super) fn connect(slot: &KeySlot) -> Result<Self> {
        let keychain = SecKeychain::default_for_domain(SecPreferencesDomain::User)
            .map_err(|e| os_error(e.code(), "open User keychain"))?;
        // The concrete provider avoids Entry's identity-bearing debug logging.
        Ok(Self {
            credential: Cred {
                domain: MacKeychainDomain::User,
                service: SERVICE.into(),
                account: slot.account.clone(),
            },
            keychain,
        })
    }

    fn check_domain(&self) -> Result<()> {
        let current = SecKeychain::default_for_domain(SecPreferencesDomain::User)
            .map_err(|e| os_error(e.code(), "recheck User keychain"))?;
        if current != self.keychain {
            return Err(failure(
                SourceVaultErrorCode::KeyMetadataMismatch,
                "User keychain changed",
            ));
        }
        Ok(())
    }

    fn query(&self) -> ItemSearchOptions {
        let mut options = ItemSearchOptions::new();
        options
            .keychains(std::slice::from_ref(&self.keychain))
            .class(ItemClass::generic_password())
            .service(SERVICE)
            .account(&self.credential.account)
            .limit(Limit::All)
            .load_attributes(true);
        options
    }

    fn attributes(&self) -> Result<std::collections::HashMap<String, String>> {
        self.check_domain()?;
        let matches = self
            .query()
            .search()
            .map_err(|e| os_error(e.code(), "read key attributes"))?;
        self.check_domain()?;
        let item = match matches.as_slice() {
            [] => {
                return Err(failure(
                    SourceVaultErrorCode::KeyMissing,
                    "read key attributes",
                ));
            }
            [item] => item,
            _ => {
                return Err(failure(
                    SourceVaultErrorCode::KeyAmbiguous,
                    "read key attributes",
                ));
            }
        };
        // No load_data/load_ref: never stringify secret-bearing search results.
        let attributes = item.simplify_dict().ok_or_else(|| {
            failure(
                SourceVaultErrorCode::KeyMetadataMismatch,
                "read key attributes",
            )
        })?;
        if attributes.get("svce").map(String::as_str) != Some(SERVICE)
            || attributes.get("acct") != Some(&self.credential.account)
        {
            return Err(failure(
                SourceVaultErrorCode::KeyMetadataMismatch,
                "verify key identity",
            ));
        }
        Ok(attributes)
    }
}

impl KeyStore for Store {
    fn read_secret(&self) -> Result<Zeroizing<Vec<u8>>> {
        self.attributes()?;
        let value = Zeroizing::new(
            self.credential
                .get_secret()
                .map_err(|e| keyring_error(e, "read key secret"))?,
        );
        self.attributes()?;
        Ok(value)
    }

    fn validate_label(&self) -> Result<()> {
        let attributes = self.attributes()?;
        check_label(attributes.get("labl").map(String::as_str).unwrap_or(""))
    }

    fn write_secret(&self, value: &[u8]) -> Result<()> {
        self.check_domain()?;
        match self.attributes() {
            Err(e) if e.code() == SourceVaultErrorCode::KeyMissing => {}
            Err(e) => return Err(e),
            Ok(_) => {
                return Err(failure(
                    SourceVaultErrorCode::KeyMetadataMismatch,
                    "key appeared before write",
                ));
            }
        }
        self.credential
            .set_secret(value)
            .map_err(|e| keyring_error(e, "write key secret"))?;
        self.check_domain()
    }

    fn set_label(&self) -> Result<()> {
        self.attributes()?;
        let mut update = ItemUpdateOptions::new();
        update.set_label(LABEL);
        // SecItemUpdate takes match predicates, not return/limit parameters.
        let mut query = ItemSearchOptions::new();
        query
            .keychains(std::slice::from_ref(&self.keychain))
            .class(ItemClass::generic_password())
            .service(SERVICE)
            .account(&self.credential.account);
        update_item(&query, &update).map_err(|e| os_error(e.code(), "write key label"))?;
        self.validate_label()
    }
}

pub(super) fn os_error(code: i32, operation: &'static str) -> SourceVaultError {
    let reason = match code {
        -25300 => SourceVaultErrorCode::KeyMissing,
        -25299 => SourceVaultErrorCode::KeyAmbiguous,
        -128 => SourceVaultErrorCode::KeyStoreCancelled,
        -61 | -25244 | -25292 | -25293 | -25308 => SourceVaultErrorCode::KeyStoreDenied,
        -25291 | -25294 | -25295 => SourceVaultErrorCode::KeyStoreUnavailable,
        _ => SourceVaultErrorCode::KeyStoreFailure,
    };
    SourceVaultError::provider(reason, operation, Some(code))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn os_status_mapping_preserves_codes_without_guessing_locked() {
        for (raw, expected) in [
            (-25300, SourceVaultErrorCode::KeyMissing),
            (-128, SourceVaultErrorCode::KeyStoreCancelled),
            (-25308, SourceVaultErrorCode::KeyStoreDenied),
            (-25291, SourceVaultErrorCode::KeyStoreUnavailable),
            (-9999, SourceVaultErrorCode::KeyStoreFailure),
        ] {
            let error = os_error(raw, "synthetic operation");
            assert_eq!(error.code(), expected);
            assert_eq!(error.os_code(), Some(raw));
        }
    }
}
