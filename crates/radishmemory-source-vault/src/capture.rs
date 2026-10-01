//! Explicit internal capture coordination. The application/host remains on v6.
use crate::{
    AttemptId, AttemptState, KeyEncryptionKey, ObjectDirectory, ObjectLocator, ObjectMetadata,
    ObjectWrite, PROVIDER_PROFILE, SourceVaultError, SourceVaultErrorCode, VaultMaintenanceError,
};
use radishmemory_core::{SourceArtifact, SourceCapture, SourceCaptureOutcome, SourceCaptureResult};
use radishmemory_sqlite::{CaptureObject, CaptureObjectState, EncryptedCaptureDatabase};
use zeroize::Zeroizing;

type Result<T> = std::result::Result<T, VaultMaintenanceError>;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Step {
    Prepared,
    Published,
    Committed,
    ReadBack,
}

pub(crate) fn capture(
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    request: &SourceCapture,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
) -> Result<SourceCaptureResult> {
    capture_with_step(directory, namespace, device, request, load_existing, |_| {
        Ok(())
    })
}
fn capture_with_step(
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    request: &SourceCapture,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
    mut step: impl FnMut(Step) -> Result<()>,
) -> Result<SourceCaptureResult> {
    let path = directory.key_database_path()?;
    let identity = crate::filesystem_support::DatabaseObservation::open(&path)?;
    let mut db = EncryptedCaptureDatabase::open(&path, namespace, device, PROVIDER_PROFILE)?;
    let verify_identity = || -> Result<()> {
        directory.key_database_path()?;
        identity.verify_identity(&path)?;
        Ok(())
    };
    verify_identity()?;
    let key = load_existing()?;
    verify_identity()?;
    let sources = authenticate_all(&db, directory, &key)?;
    db.verify_facts(&sources)?;
    let (selected, outcome) = db.decide(request, &sources)?;
    if outcome == SourceCaptureOutcome::Idempotent {
        verify_identity()?;
        return Ok(SourceCaptureResult::from_source(&selected, outcome));
    }
    db.initialize_capture_schema()?;
    verify_identity()?;
    let pending = db
        .objects()?
        .into_iter()
        .find(|o| o.state == CaptureObjectState::Prepared);
    let absent = if let Some(item) = &pending {
        directory.inspect_attempt(
            &ObjectLocator::from_token(&item.locator)?,
            &AttemptId::from_token(&item.attempt_id)?,
            &metadata(item)?,
            &key,
        )? == AttemptState::Absent
    } else {
        true
    };
    if absent {
        let p = request.source().params();
        let meta = metadata_for_source(request.source())?;
        let write = ObjectWrite::seal(&key, &meta, p.content.as_str().as_bytes())?;
        db.prepare_capture(request, write.locator().token(), write.attempt_id().token())?;
        step(Step::Prepared)?;
        verify_identity()?;
        directory.publish(&write, &key)?;
    } else {
        let item = pending.ok_or_else(invalid)?;
        directory.recover_attempt(
            &ObjectLocator::from_token(&item.locator)?,
            &AttemptId::from_token(&item.attempt_id)?,
            &metadata(&item)?,
            &key,
        )?;
    }
    step(Step::Published)?;
    verify_identity()?;
    // Re-authenticate the published attempt immediately before committing facts.
    let item = db
        .objects()?
        .into_iter()
        .find(|o| o.state == CaptureObjectState::Prepared)
        .ok_or_else(invalid)?;
    directory.recover_attempt(
        &ObjectLocator::from_token(&item.locator)?,
        &AttemptId::from_token(&item.attempt_id)?,
        &metadata(&item)?,
        &key,
    )?;
    let committed = db.commit_capture(request, &sources)?;
    step(Step::Committed)?;
    verify_identity()?;
    let reloaded = authenticate_all(&db, directory, &key)?;
    db.verify_facts(&reloaded)?;
    step(Step::ReadBack)?;
    verify_identity()?;
    let source = reloaded
        .iter()
        .find(|s| s.params().source_id == request.source().params().source_id)
        .ok_or_else(invalid)?;
    Ok(SourceCaptureResult::from_source(source, committed))
}

pub(crate) fn authenticate_all(
    db: &EncryptedCaptureDatabase,
    directory: &ObjectDirectory,
    key: &KeyEncryptionKey,
) -> Result<Vec<SourceArtifact>> {
    let items = db.objects()?;
    let tokens = items
        .iter()
        .map(|i| {
            Ok((
                ObjectLocator::from_token(&i.locator)?,
                AttemptId::from_token(&i.attempt_id)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    directory.verify_inventory(&tokens)?;
    let mut sources = Vec::new();
    for item in &items {
        let meta = metadata(item)?;
        if item.state != CaptureObjectState::Committed {
            let state = directory.inspect_attempt(
                &ObjectLocator::from_token(&item.locator)?,
                &AttemptId::from_token(&item.attempt_id)?,
                &meta,
                key,
            )?;
            if matches!(
                item.state,
                CaptureObjectState::Abandoned | CaptureObjectState::Deleted
            ) && state != AttemptState::Absent
            {
                return Err(invalid());
            }
            continue;
        }
        // The read path is selected by the committed reference, never by a
        // publication receipt or uncommitted request supplied by the caller.
        let (locator, attempt) = db.reference(&item.source_id)?;
        let locator = ObjectLocator::from_token(&locator)?;
        let attempt = AttemptId::from_token(&attempt)?;
        let state = directory.inspect_attempt(&locator, &attempt, &meta, key)?;
        if !matches!(
            state,
            AttemptState::AuthenticatedPublishedCandidate
                | AttemptState::AuthenticatedStagingAndPublishedCandidate
        ) {
            return Err(invalid());
        }
        let body = Zeroizing::new(directory.read(&locator, &meta, key)?);
        if let Some(source) = db.resolve_object(item, &body)? {
            sources.push(source);
        }
    }
    directory.verify_inventory(&tokens)?;
    Ok(sources)
}
pub(crate) fn metadata(item: &CaptureObject) -> Result<ObjectMetadata> {
    Ok(ObjectMetadata::new(
        &item.namespace_id,
        &item.source_id,
        crate::body_migration::decode_digest(&item.digest_value)?,
        item.content_length,
        &item.media_type,
    )?)
}
fn metadata_for_source(source: &SourceArtifact) -> Result<ObjectMetadata> {
    let p = source.params();
    Ok(ObjectMetadata::new(
        p.namespace_id.as_str(),
        p.source_id.as_str(),
        crate::body_migration::decode_digest(p.content_digest.value())?,
        p.content_length,
        p.media_type.as_str(),
    )?)
}
fn invalid() -> VaultMaintenanceError {
    SourceVaultError::new(
        SourceVaultErrorCode::AttemptMismatch,
        "encrypted capture state mismatch",
    )
    .into()
}

#[cfg(test)]
pub(crate) mod tests;
