//! Reconcile the v8/v9/v10 inventory without deciding to abandon recoverable captures.
use crate::{
    AttemptId, AttemptState, KeyEncryptionKey, ObjectDirectory, ObjectLocator, PROVIDER_PROFILE,
    SourceVaultError, VaultMaintenanceError,
};
use radishmemory_sqlite::{CaptureObjectState, EncryptedCaptureDatabase};

type Result<T> = std::result::Result<T, VaultMaintenanceError>;

/// Path-free observations after complete authentication. A pending capture still
/// needs its original request; this report is not a product-readiness receipt.
#[derive(Debug, Eq, PartialEq)]
pub struct ReconciliationReport {
    pub committed_objects_verified: usize,
    pub pending_capture: Option<AttemptState>,
    pub abandonment_pending: bool,
    pub abandoned_attempts: usize,
    pub deletion_pending_objects: usize,
    pub deleted_objects: usize,
    pub committed_staging_links_removed: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    Validated,
    BeforeCleanup,
    AfterCleanup,
    BeforeReadBack,
}

pub(crate) fn reconcile(
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
) -> Result<ReconciliationReport> {
    reconcile_with_step(directory, namespace, device, load_existing, |_| Ok(()))
}

fn reconcile_with_step(
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
    mut step: impl FnMut(Step) -> Result<()>,
) -> Result<ReconciliationReport> {
    let path = directory.key_database_path()?;
    let identity = crate::filesystem_support::Observation::open(&path)?;
    let db = EncryptedCaptureDatabase::open(&path, namespace, device, PROVIDER_PROFILE)?;
    let verify_identity = || -> Result<()> {
        directory.key_database_path()?;
        identity.verify_identity(&path)?;
        Ok(())
    };
    verify_identity()?;
    let key = load_existing()?;
    verify_identity()?;
    let verify_all = || -> Result<()> {
        let sources = crate::capture::authenticate_all(&db, directory, &key)?;
        db.verify_facts(&sources)?;
        verify_identity()
    };
    // Validate the entire library before touching even a duplicate link.
    verify_all()?;
    step(Step::Validated)?;
    verify_identity()?;
    let mut removed = 0;
    let items = db.objects()?;
    for item in items
        .iter()
        .filter(|item| item.state == CaptureObjectState::Committed)
    {
        let (locator, attempt) = db.reference(&item.source_id)?;
        step(Step::BeforeCleanup)?;
        verify_identity()?;
        // Resolves the committed reference again and requires its existing file.
        // No publish, cancellation, orphan deletion or metadata mutation occurs.
        removed += usize::from(directory.cleanup_committed_staging(
            &ObjectLocator::from_token(&locator)?,
            &AttemptId::from_token(&attempt)?,
            &crate::capture::metadata(item)?,
            &key,
        )?);
        step(Step::AfterCleanup)?;
        verify_identity()?;
    }
    step(Step::BeforeReadBack)?;
    verify_all()?;
    let pending_capture = items
        .iter()
        .find(|item| item.state == CaptureObjectState::Prepared)
        .map(|item| -> Result<AttemptState> {
            Ok(directory.inspect_attempt(
                &ObjectLocator::from_token(&item.locator)?,
                &AttemptId::from_token(&item.attempt_id)?,
                &crate::capture::metadata(item)?,
                &key,
            )?)
        })
        .transpose()?;
    verify_identity()?;
    Ok(ReconciliationReport {
        committed_objects_verified: items
            .iter()
            .filter(|item| item.state == CaptureObjectState::Committed)
            .count(),
        pending_capture,
        abandonment_pending: items
            .iter()
            .any(|i| i.state == CaptureObjectState::Abandoning),
        abandoned_attempts: items
            .iter()
            .filter(|i| i.state == CaptureObjectState::Abandoned)
            .count(),
        deletion_pending_objects: items
            .iter()
            .filter(|i| i.state == CaptureObjectState::Deleting)
            .count(),
        deleted_objects: items
            .iter()
            .filter(|i| i.state == CaptureObjectState::Deleted)
            .count(),
        committed_staging_links_removed: removed,
    })
}

#[cfg(test)]
mod tests;
