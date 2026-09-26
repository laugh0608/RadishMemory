//! Explicit object-backed verification and atomic derived-row repair.
use crate::{
    KeyEncryptionKey, ObjectDirectory, PROVIDER_PROFILE, SourceVaultError, VaultMaintenanceError,
};
use radishmemory_sqlite::{CaptureObjectState, EncryptedCaptureDatabase};

type Result<T> = std::result::Result<T, VaultMaintenanceError>;

/// Path-free observations, not a product-readiness or deletion receipt.
#[derive(Debug, Eq, PartialEq)]
pub struct VerificationReport {
    pub committed_objects_verified: usize,
    pub pending_captures: usize,
    pub abandonment_pending: bool,
    pub abandoned_attempts: usize,
    pub deletion_pending_objects: usize,
    pub deleted_objects: usize,
    pub derivations_rebuilt: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Step {
    Authenticated,
    BeforeCommit,
    Committed,
    BeforeReadBack,
}

pub(crate) fn maintain(
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    rebuild: bool,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
) -> Result<VerificationReport> {
    maintain_with_step(directory, namespace, device, rebuild, load_existing, |_| {
        Ok(())
    })
}

fn maintain_with_step(
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    rebuild: bool,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
    mut step: impl FnMut(Step) -> Result<()>,
) -> Result<VerificationReport> {
    let path = directory.key_database_path()?;
    let identity = crate::filesystem_support::DatabaseObservation::open(&path)?;
    let db = EncryptedCaptureDatabase::open(&path, namespace, device, PROVIDER_PROFILE)?;
    let verify_identity = || -> Result<()> {
        directory.key_database_path()?;
        identity.verify_identity(&path)?;
        Ok(())
    };
    verify_identity()?;
    let key = load_existing()?;
    verify_identity()?;
    let sources = crate::capture::authenticate_all(&db, directory, &key)?;
    db.verify_recall_index_structure()?;
    step(Step::Authenticated)?;
    verify_identity()?;
    let verify_all = || -> Result<()> {
        let current = crate::capture::authenticate_all(&db, directory, &key)?;
        db.verify_facts(&current)?;
        db.verify_recall_index_structure()?;
        verify_identity()
    };
    if rebuild {
        db.rebuild_recall_derivations(&sources, || -> Result<()> {
            step(Step::BeforeCommit)?;
            verify_all()
        })?;
        step(Step::Committed)?;
    }
    step(Step::BeforeReadBack)?;
    verify_all()?;
    let items = db.objects()?;
    verify_identity()?;
    Ok(VerificationReport {
        committed_objects_verified: items
            .iter()
            .filter(|i| i.state == CaptureObjectState::Committed)
            .count(),
        pending_captures: items
            .iter()
            .filter(|i| i.state == CaptureObjectState::Prepared)
            .count(),
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
        derivations_rebuilt: rebuild,
    })
}

#[cfg(test)]
mod tests;
