//! Explicit, durable cancellation of uncommitted captures only.
use crate::filesystem::RetirementStep;
use crate::{
    AttemptId, KeyEncryptionKey, ObjectDirectory, ObjectLocator, PROVIDER_PROFILE,
    SourceVaultError, SourceVaultErrorCode, VaultMaintenanceError,
};
pub use radishmemory_sqlite::CaptureAbandonmentTarget;
use radishmemory_sqlite::{CaptureObjectState, EncryptedCaptureDatabase};

type Result<T> = std::result::Result<T, VaultMaintenanceError>;

/// Returned only after directory durability, terminal commit and full read-back.
#[derive(Debug, Eq, PartialEq)]
pub struct AbandonmentReport {
    pub already_abandoned: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    Validated,
    IntentCommitted,
    Filesystem(RetirementStep),
    BeforeTerminalCommit,
    TerminalCommitted,
    ReadBack,
}

pub(crate) fn inspect(
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
) -> Result<Option<CaptureAbandonmentTarget>> {
    let path = directory.key_database_path()?;
    let identity = crate::filesystem_support::DatabaseObservation::open(&path)?;
    let db = EncryptedCaptureDatabase::open(&path, namespace, device, PROVIDER_PROFILE)?;
    identity.verify_identity(&directory.key_database_path()?)?;
    let key = load_existing()?;
    let sources = crate::capture::authenticate_all(&db, directory, &key)?;
    db.verify_facts(&sources)?;
    let target = db.pending_abandonment_target()?;
    identity.verify_identity(&directory.key_database_path()?)?;
    Ok(target)
}

pub(crate) fn abandon(
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    target: &CaptureAbandonmentTarget,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
) -> Result<AbandonmentReport> {
    abandon_with_step(directory, namespace, device, target, load_existing, |_| {
        Ok(())
    })
}

fn abandon_with_step(
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    target: &CaptureAbandonmentTarget,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
    mut step: impl FnMut(Step) -> Result<()>,
) -> Result<AbandonmentReport> {
    let path = directory.key_database_path()?;
    let identity = crate::filesystem_support::DatabaseObservation::open(&path)?;
    let mut db = EncryptedCaptureDatabase::open(&path, namespace, device, PROVIDER_PROFILE)?;
    let verify_identity = || -> Result<()> {
        identity.verify_identity(&directory.key_database_path()?)?;
        Ok(())
    };
    verify_identity()?;
    let item = db.abandonment_item(target)?;
    let key = load_existing()?;
    verify_identity()?;
    db.verify_facts(&crate::capture::authenticate_all(&db, directory, &key)?)?;
    step(Step::Validated)?;
    verify_identity()?;
    let already_abandoned = item.state == CaptureObjectState::Abandoned;
    if !already_abandoned {
        db.begin_abandonment(target)?;
        step(Step::IntentCommitted)?;
        verify_identity()?;
        // Revalidate after the durable decision. Failure leaves abandoning and
        // cannot re-enable capture, even when no file has been removed yet.
        let item = db.abandonment_item(target)?;
        db.verify_facts(&crate::capture::authenticate_all(&db, directory, &key)?)?;
        directory.retire_attempt(
            &ObjectLocator::from_token(&item.locator)?,
            &AttemptId::from_token(&item.attempt_id)?,
            &crate::capture::metadata(&item)?,
            &key,
            |point| -> Result<()> {
                step(Step::Filesystem(point))?;
                verify_identity()
            },
        )?;
        verify_identity()?;
        db.verify_facts(&crate::capture::authenticate_all(&db, directory, &key)?)?;
        step(Step::BeforeTerminalCommit)?;
        verify_identity()?;
        if directory.inspect_attempt(
            &ObjectLocator::from_token(&item.locator)?,
            &AttemptId::from_token(&item.attempt_id)?,
            &crate::capture::metadata(&item)?,
            &key,
        )? != crate::AttemptState::Absent
        {
            return Err(SourceVaultError::new(
                SourceVaultErrorCode::AttemptMismatch,
                "abandoned paths reappeared",
            )
            .into());
        }
        db.finish_abandonment(target)?;
        step(Step::TerminalCommitted)?;
    }
    step(Step::ReadBack)?;
    verify_identity()?;
    db.verify_facts(&crate::capture::authenticate_all(&db, directory, &key)?)?;
    if db.abandonment_item(target)?.state != CaptureObjectState::Abandoned {
        return Err(SourceVaultError::new(
            SourceVaultErrorCode::AttemptMismatch,
            "abandonment incomplete",
        )
        .into());
    }
    verify_identity()?;
    Ok(AbandonmentReport { already_abandoned })
}

#[cfg(test)]
mod tests;
