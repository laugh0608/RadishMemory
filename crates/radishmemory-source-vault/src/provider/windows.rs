use std::collections::HashMap;

use keyring_core::api::CredentialApi;
use windows_native_keyring_store::{CredPersist, cred::Cred};
use zeroize::Zeroizing;

use super::{KeySlot, KeyStore, LABEL, Result, SERVICE, check_label, failure, keyring_error};
use crate::SourceVaultErrorCode;

pub(super) struct Store {
    credential: Cred,
    account: String,
}

impl Store {
    pub(super) fn connect(slot: &KeySlot) -> Result<Self> {
        // Upstream's explicit-target builder discards specifiers, which would
        // initialize an empty username. Set the concrete provider identity here.
        Ok(Self {
            credential: Cred {
                target_name: slot.windows_target.clone(),
                specifiers: Some((SERVICE.into(), slot.account.clone())),
                persistence: CredPersist::Local,
            },
            account: slot.account.clone(),
        })
    }

    fn attributes(&self) -> Result<HashMap<String, String>> {
        let attributes = self
            .credential
            .get_attributes()
            .map_err(|e| keyring_error(e, "read key attributes"))?;
        validate_attributes(&attributes, &self.credential.target_name, &self.account)?;
        Ok(attributes)
    }
}

fn validate_attributes(
    attributes: &HashMap<String, String>,
    target: &str,
    account: &str,
) -> Result<()> {
    for (name, expected) in [
        ("persistence", "Local"),
        ("target_name", target),
        ("username", account),
    ] {
        if attributes.get(name).map(String::as_str) != Some(expected) {
            return Err(failure(
                SourceVaultErrorCode::KeyMetadataMismatch,
                "verify key identity and Local persistence",
            ));
        }
    }
    Ok(())
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
        check_label(
            self.attributes()?
                .get("comment")
                .map(String::as_str)
                .unwrap_or(""),
        )
    }

    fn write_secret(&self, value: &[u8]) -> Result<()> {
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
        // Preserve the frozen ASCII blob: password APIs would use UTF-16LE.
        self.credential
            .set_secret(value)
            .map_err(|e| keyring_error(e, "write key secret"))?;
        self.attributes()?;
        Ok(())
    }

    fn set_label(&self) -> Result<()> {
        self.attributes()?;
        self.credential
            .update_attributes(&HashMap::from([("comment", LABEL)]))
            .map_err(|e| keyring_error(e, "write key label"))?;
        self.validate_label()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_identity_is_required_without_system_access() {
        let mut attributes = HashMap::from([
            ("persistence".into(), "Local".into()),
            ("target_name".into(), "synthetic-target".into()),
            ("username".into(), "synthetic-account".into()),
        ]);
        assert!(validate_attributes(&attributes, "synthetic-target", "synthetic-account").is_ok());
        for value in ["Session", "Enterprise", "", "local", "unknown"] {
            attributes.insert("persistence".into(), value.into());
            assert_eq!(
                validate_attributes(&attributes, "synthetic-target", "synthetic-account")
                    .unwrap_err()
                    .code(),
                SourceVaultErrorCode::KeyMetadataMismatch
            );
        }
        attributes.remove("persistence");
        assert!(validate_attributes(&attributes, "synthetic-target", "synthetic-account").is_err());
    }
}
