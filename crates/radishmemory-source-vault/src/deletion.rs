//! Explicit local deletion: durable recall exclusion precedes physical retirement.
use crate::filesystem::RetirementStep;
use crate::{
    AttemptId, KeyEncryptionKey, ObjectDirectory, ObjectLocator, PROVIDER_PROFILE,
    SourceVaultError, VaultMaintenanceError,
};
use radishmemory_core::{ComponentResult, DeleteRequest, DeletionEvidence, LocalDeletionExecution};
use radishmemory_sqlite::{CaptureObjectState, EncryptedCaptureDatabase};

type Result<T> = std::result::Result<T, VaultMaintenanceError>;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Step {
    Validated,
    IntentCommitted,
    Filesystem(RetirementStep),
    BeforeRetirementCommit,
    RetirementCommitted,
    BeforeExecution,
    Executed,
    BeforeReadBack,
}

pub(crate) fn execute(
    directory: &ObjectDirectory,
    request: &DeleteRequest,
    execution: &LocalDeletionExecution,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
) -> Result<Vec<ComponentResult>> {
    execute_with_step(directory, request, execution, load_existing, |_| Ok(()))
}

fn execute_with_step(
    directory: &ObjectDirectory,
    request: &DeleteRequest,
    execution: &LocalDeletionExecution,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
    mut step: impl FnMut(Step) -> Result<()>,
) -> Result<Vec<ComponentResult>> {
    let path = directory.key_database_path()?;
    let identity = crate::filesystem_support::Observation::open(&path)?;
    let p = request.params();
    let mut db = EncryptedCaptureDatabase::open(
        &path,
        p.namespace_id.as_str(),
        p.device_id.as_str(),
        PROVIDER_PROFILE,
    )?;
    let verify_identity = || -> Result<()> {
        identity.verify_identity(&directory.key_database_path()?)?;
        Ok(())
    };
    verify_identity()?;
    let key = load_existing()?;
    verify_identity()?;
    let sources = crate::capture::authenticate_all(&db, directory, &key)?;
    db.verify_facts(&sources)?;
    db.verify_recall_index_structure()?;
    step(Step::Validated)?;
    verify_identity()?;
    db.begin_object_deletion(request, &sources)?;
    step(Step::IntentCommitted)?;
    verify_identity()?;
    let verify_all = |db: &EncryptedCaptureDatabase| -> Result<()> {
        db.verify_facts(&crate::capture::authenticate_all(db, directory, &key)?)?;
        db.verify_recall_index_structure()?;
        verify_identity()
    };
    verify_all(&db)?;
    for item in db.deletion_objects(request)? {
        if item.state == CaptureObjectState::Deleted {
            continue;
        }
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
        step(Step::BeforeRetirementCommit)?;
        verify_identity()?;
        // Recheck durable absence even when a test/host interruption occurred
        // between unlink and checkpoint. Never mark a reappeared file as deleted.
        if directory.inspect_attempt(
            &ObjectLocator::from_token(&item.locator)?,
            &AttemptId::from_token(&item.attempt_id)?,
            &crate::capture::metadata(&item)?,
            &key,
        )? != crate::AttemptState::Absent
        {
            return Err(SourceVaultError::new(
                crate::SourceVaultErrorCode::AttemptMismatch,
                "object reappeared before retirement commit",
            )
            .into());
        }
        db.finish_object_retirement(request, &item.source_id)?;
        step(Step::RetirementCommitted)?;
        verify_all(&db)?;
    }
    step(Step::BeforeExecution)?;
    verify_all(&db)?;
    let results = db.execute_object_deletion(request, execution)?;
    step(Step::Executed)?;
    step(Step::BeforeReadBack)?;
    verify_all(&db)?;
    Ok(results)
}

pub(crate) fn store_evidence(
    directory: &ObjectDirectory,
    evidence: &DeletionEvidence,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
) -> Result<()> {
    let path = directory.key_database_path()?;
    let identity = crate::filesystem_support::Observation::open(&path)?;
    let p = evidence.params();
    let mut db = EncryptedCaptureDatabase::open(
        &path,
        p.namespace_id.as_str(),
        p.device_id.as_str(),
        PROVIDER_PROFILE,
    )?;
    identity.verify_identity(&directory.key_database_path()?)?;
    let key = load_existing()?;
    db.verify_facts(&crate::capture::authenticate_all(&db, directory, &key)?)?;
    db.verify_recall_index_structure()?;
    identity.verify_identity(&directory.key_database_path()?)?;
    db.store_object_deletion_evidence(evidence)?;
    db.verify_facts(&crate::capture::authenticate_all(&db, directory, &key)?)?;
    db.verify_recall_index_structure()?;
    if db
        .load_object_deletion_evidence(&p.deletion_evidence_id)?
        .as_ref()
        != Some(evidence)
    {
        return Err(SourceVaultError::new(
            crate::SourceVaultErrorCode::AttemptMismatch,
            "deletion evidence readback mismatch",
        )
        .into());
    }
    identity.verify_identity(&directory.key_database_path()?)?;
    Ok(())
}

#[cfg(test)]
mod tests;
