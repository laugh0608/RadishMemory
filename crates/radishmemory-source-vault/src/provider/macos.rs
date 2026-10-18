use apple_native_keyring_store::keychain::{Cred, MacKeychainDomain};
use keyring_core::api::CredentialApi;
use security_framework::item::{
    ItemClass, ItemSearchOptions, ItemUpdateOptions, Limit, SearchResult, update_item,
};
use security_framework::os::macos::keychain::{SecKeychain, SecPreferencesDomain};
use zeroize::{Zeroize, Zeroizing};

use super::{KeySlot, KeyStore, LABEL, Result, SERVICE, check_label, failure, keyring_error};
use crate::key_read_observation::{Operation, call};
use crate::{SourceVaultError, SourceVaultErrorCode};

pub(super) struct Store {
    credential: Cred,
    keychain: SecKeychain,
}

impl Store {
    pub(super) fn connect(slot: &KeySlot) -> Result<Self> {
        let keychain = call(Operation::Domain, || {
            SecKeychain::default_for_domain(SecPreferencesDomain::User)
                .map_err(|e| os_error(e.code(), "open User keychain"))
        })?;
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
        let matches = checked_attributes(
            || self.check_domain(),
            || {
                self.query()
                    .search()
                    .map_err(|e| os_error(e.code(), "read key attributes"))
            },
        )?;
        let item = match matches.0.as_slice() {
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

    fn secret_query(&self) -> ItemSearchOptions {
        let mut options = ItemSearchOptions::new();
        // A separate data-only query: Limit::All is invalid for password data.
        // The surrounding attributes queries still establish uniqueness.
        options
            .keychains(std::slice::from_ref(&self.keychain))
            .class(ItemClass::generic_password())
            .service(SERVICE)
            .account(&self.credential.account)
            .limit(Limit::Max(1))
            .load_data(true);
        options
    }
}

impl KeyStore for Store {
    fn read_secret(&self) -> Result<Zeroizing<Vec<u8>>> {
        read_checked_secret(
            || self.attributes().map(|_| ()),
            || {
                self.secret_query()
                    .search()
                    .map_err(|e| os_error(e.code(), "read key secret"))
            },
        )
    }

    fn validate_label(&self) -> Result<()> {
        call(Operation::AttributesLabel, || {
            let attributes = self.attributes()?;
            check_label(attributes.get("labl").map(String::as_str).unwrap_or(""))
        })
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

// Keep checks and result handling shared with synthetic tests, without a
// production provider override or any key-store access in those tests.
fn read_checked_secret(
    mut attributes: impl FnMut() -> Result<()>,
    search: impl FnOnce() -> Result<Vec<SearchResult>>,
) -> Result<Zeroizing<Vec<u8>>> {
    call(Operation::AttributesPre, &mut attributes)?;
    let mut matches = call(Operation::SecretSearch, || search().map(GuardedResults))?;
    let value = secret_result(&mut matches.0)?;
    drop(matches);
    call(Operation::AttributesPost, &mut attributes)?;
    Ok(value)
}

// The returned search buffers acquire a wipe guard before the search end is emitted.
struct GuardedResults(Vec<SearchResult>);
impl Drop for GuardedResults {
    fn drop(&mut self) {
        drop(SecretResults(&mut self.0));
    }
}
fn checked_attributes(
    mut domain: impl FnMut() -> Result<()>,
    search: impl FnOnce() -> Result<Vec<SearchResult>>,
) -> Result<GuardedResults> {
    call(Operation::Domain, &mut domain)?;
    let matches = call(Operation::AttributesSearch, || search().map(GuardedResults))?;
    call(Operation::Domain, &mut domain)?;
    Ok(matches)
}

// SearchResult has a secret-bearing Debug implementation and no zeroizing Drop.
// Wipe every Rust data buffer, including unexpected multi-result responses.
// CF/OS-owned values are only released; their internal copies cannot be wiped here.
struct SecretResults<'a>(&'a mut [SearchResult]);

impl Drop for SecretResults<'_> {
    fn drop(&mut self) {
        for item in self.0.iter_mut() {
            if let SearchResult::Data(bytes) = item {
                bytes.zeroize();
            }
        }
        #[cfg(test)]
        tests::record_secret_cleanup(self.0);
    }
}

fn secret_result(matches: &mut [SearchResult]) -> Result<Zeroizing<Vec<u8>>> {
    let results = SecretResults(matches);
    match &mut *results.0 {
        [SearchResult::Data(bytes)] => Ok(Zeroizing::new(std::mem::take(bytes))),
        [] | [_] => Err(failure(
            // A malformed successful response must never authorize bootstrap.
            SourceVaultErrorCode::KeyStoreFailure,
            "read key secret result",
        )),
        _ => Err(failure(
            SourceVaultErrorCode::KeyAmbiguous,
            "read key secret result",
        )),
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
#[path = "macos/tests.rs"]
mod tests;
