//! Recovery of exact authenticated attempts only; unknown entries are never deleted.
use super::*;
use std::collections::BTreeSet;

impl ObjectDirectory {
    pub(crate) fn verify_inventory(&self, attempts: &[(ObjectLocator, AttemptId)]) -> Result<()> {
        self.verify()?;
        let objects: BTreeSet<_> = attempts
            .iter()
            .map(|(l, _)| format!("{}.rmo", l.token()))
            .collect();
        let staging: BTreeSet<_> = attempts
            .iter()
            .map(|(l, a)| format!("{}.{}.stage", l.token(), a.token()))
            .collect();
        if objects.len() != attempts.len() || staging.len() != attempts.len() {
            return Err(unknown());
        }
        for (directory, allowed) in [(&self.objects, objects), (&self.staging, staging)] {
            for entry in fs::read_dir(&directory.path)
                .map_err(|e| SourceVaultError::io("inspect migration inventory", e))?
            {
                let entry =
                    entry.map_err(|e| SourceVaultError::io("inspect migration entry", e))?;
                let name = entry.file_name();
                if !name.to_str().is_some_and(|s| allowed.contains(s)) {
                    return Err(unknown());
                }
                support::present(&entry.path())?;
            }
        }
        self.verify()
    }

    /// Re-establish durability even when the previous process died before sync.
    /// Never reseal, replace, or adopt a different attempt's object.
    pub(crate) fn recover_attempt(
        &self,
        locator: &ObjectLocator,
        attempt: &AttemptId,
        metadata: &ObjectMetadata,
        key: &KeyEncryptionKey,
    ) -> Result<()> {
        self.finish_attempt(locator, attempt, metadata, key, true)
            .map(|_| ())
    }

    /// Only remove the duplicate staging link of an existing committed object.
    /// Missing canonical objects must never be republished by reconciliation.
    pub(crate) fn cleanup_committed_staging(
        &self,
        locator: &ObjectLocator,
        attempt: &AttemptId,
        metadata: &ObjectMetadata,
        key: &KeyEncryptionKey,
    ) -> Result<bool> {
        self.finish_attempt(locator, attempt, metadata, key, false)
    }

    fn finish_attempt(
        &self,
        locator: &ObjectLocator,
        attempt: &AttemptId,
        metadata: &ObjectMetadata,
        key: &KeyEncryptionKey,
        allow_publication: bool,
    ) -> Result<bool> {
        let state = self.inspect_attempt(locator, attempt, metadata, key)?;
        if state == AttemptState::Absent
            || (!allow_publication && state == AttemptState::AuthenticatedStaging)
        {
            return Err(SourceVaultError::new(
                SourceVaultErrorCode::ObjectMissing,
                "required object missing",
            ));
        }
        let staging = self.staging_path(locator, attempt);
        let target = self.object_path(locator);
        let has_staging = matches!(
            state,
            AttemptState::AuthenticatedStaging
                | AttemptState::AuthenticatedStagingAndPublishedCandidate
        );
        let has_target = state != AttemptState::AuthenticatedStaging;
        let path = if has_target { &target } else { &staging };
        let (bytes, observed) = support::read_file(path)?;
        self.authenticate(&bytes, metadata, locator, attempt, key)?;
        support::sync_file(path, &observed)?;
        self.verify()?;
        if !has_target {
            support::verify_file(&staging, &observed)?;
            fs::hard_link(&staging, &target)
                .map_err(|e| SourceVaultError::io("publish recovered attempt", e))?;
        }
        self.objects.sync()?;
        let (published, reference) = support::read_file(&target)?;
        if published != bytes || reference != observed {
            return Err(support::changed());
        }
        self.authenticate(&published, metadata, locator, attempt, key)?;
        if has_staging {
            let (staged, staging_reference) = support::read_file(&staging)?;
            if staged != bytes || staging_reference != reference {
                return Err(support::changed());
            }
            self.verify()?;
            support::verify_file(&target, &reference)?;
            support::verify_file(&staging, &staging_reference)?;
            fs::remove_file(&staging)
                .map_err(|e| SourceVaultError::io("remove recovered staging link", e))?;
        }
        self.staging.sync()?;
        self.verify()?;
        support::verify_file(&target, &reference)?;
        Ok(has_staging)
    }
}
fn unknown() -> SourceVaultError {
    SourceVaultError::new(
        SourceVaultErrorCode::AttemptMismatch,
        "unrecognized or ambiguous migration inventory",
    )
}
