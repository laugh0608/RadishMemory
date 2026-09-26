//! Maintenance-only v7 -> v8 migration. Ordinary application reads remain v6.
use radishmemory_sqlite::{BodyMigrationDatabase, BodyMigrationItem, BodyMigrationState};
use zeroize::Zeroizing;

use crate::{
    AttemptId, AttemptState, KeyEncryptionKey, ObjectDirectory, ObjectLocator, ObjectMetadata,
    ObjectWrite, PROVIDER_PROFILE, SourceVaultError, SourceVaultErrorCode, VaultMaintenanceError,
};

type Result<T> = std::result::Result<T, VaultMaintenanceError>;

/// All inventoried bodies authenticated and retired. Not product/host readiness.
#[derive(Debug, Eq, PartialEq)]
pub struct BodyMigrationReport {
    pub objects_verified: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    Inventory,
    Prepared,
    Published,
    ReferenceCommitted,
    ReadBack,
    InlineRetired,
}

pub(crate) fn migrate(
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
) -> Result<BodyMigrationReport> {
    migrate_with_step(directory, namespace, device, load_existing, |_| Ok(()))
}

fn migrate_with_step(
    directory: &ObjectDirectory,
    namespace: &str,
    device: &str,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
    mut step: impl FnMut(Step) -> Result<()>,
) -> Result<BodyMigrationReport> {
    let path = directory.key_database_path()?;
    let identity = crate::filesystem_support::DatabaseObservation::open(&path)?;
    let mut db = BodyMigrationDatabase::open(&path, namespace, device, PROVIDER_PROFILE)?;
    let verify_identity = || -> Result<()> {
        directory.key_database_path()?;
        identity.verify_identity(&path)?;
        Ok(())
    };
    verify_identity()?;
    // No creation operation is available in this path, even with an empty inventory.
    let key = load_existing()?;
    verify_identity()?;
    if !db.has_inventory() {
        directory.verify_inventory(&[])?;
        db.initialize_inventory()?;
    }
    step(Step::Inventory)?;
    verify_identity()?;
    let items = db.items()?;
    verify_inventory(directory, &items)?;
    // Validate every remaining legacy body and every existing attempt before
    // mutating anything. Old versions and deletion residuals are included.
    for item in &items {
        let meta = metadata(item)?;
        drop(db.inline_body(item)?.map(Zeroizing::new));
        if item.state != BodyMigrationState::Planned {
            let (locator, attempt) = tokens(item)?;
            let state = directory.inspect_attempt(&locator, &attempt, &meta, &key)?;
            if matches!(
                item.state,
                BodyMigrationState::Referenced | BodyMigrationState::Retired
            ) {
                if !matches!(
                    state,
                    AttemptState::AuthenticatedPublishedCandidate
                        | AttemptState::AuthenticatedStagingAndPublishedCandidate
                ) {
                    return Err(invalid());
                }
                read_reference(&db, directory, item, &meta, &key)?;
            }
        }
    }
    for mut item in items {
        let meta = metadata(&item)?;
        verify_identity()?;
        if item.state == BodyMigrationState::Retired {
            continue;
        }
        if matches!(
            item.state,
            BodyMigrationState::Planned | BodyMigrationState::Prepared
        ) {
            let absent = if item.state == BodyMigrationState::Planned {
                true
            } else {
                let (locator, attempt) = tokens(&item)?;
                directory.inspect_attempt(&locator, &attempt, &meta, &key)? == AttemptState::Absent
            };
            if absent {
                let body = Zeroizing::new(db.inline_body(&item)?.ok_or_else(invalid)?);
                let write = ObjectWrite::seal(&key, &meta, &body)?;
                verify_identity()?;
                db.prepare(&item, write.locator().token(), write.attempt_id().token())?;
                item.locator = Some(write.locator().token().into());
                item.attempt_id = Some(write.attempt_id().token().into());
                item.state = BodyMigrationState::Prepared;
                step(Step::Prepared)?;
                verify_identity()?;
                directory.publish(&write, &key)?;
            } else {
                let (locator, attempt) = tokens(&item)?;
                directory.recover_attempt(&locator, &attempt, &meta, &key)?;
            }
            step(Step::Published)?;
            verify_identity()?;
            db.commit_reference(&item)?;
            item.state = BodyMigrationState::Referenced;
            step(Step::ReferenceCommitted)?;
        }
        verify_identity()?;
        // Resume referenced objects without recreating content, key, source or audit.
        let (locator, attempt) = tokens(&item)?;
        directory.recover_attempt(&locator, &attempt, &meta, &key)?;
        read_reference(&db, directory, &item, &meta, &key)?;
        step(Step::ReadBack)?;
        verify_identity()?;
        // Recheck the committed object immediately before retiring its inline copy.
        read_reference(&db, directory, &item, &meta, &key)?;
        db.retire_inline(&item)?;
        step(Step::InlineRetired)?;
    }
    verify_identity()?;
    let items = db.items()?;
    verify_inventory(directory, &items)?;
    for item in &items {
        read_reference(&db, directory, item, &metadata(item)?, &key)?;
    }
    verify_identity()?;
    db.finish()?;
    verify_identity()?;
    Ok(BodyMigrationReport {
        objects_verified: items.len(),
    })
}

fn read_reference(
    db: &BodyMigrationDatabase,
    directory: &ObjectDirectory,
    item: &BodyMigrationItem,
    meta: &ObjectMetadata,
    key: &KeyEncryptionKey,
) -> Result<()> {
    let (locator, attempt) = db.reference(item)?;
    let locator = ObjectLocator::from_token(&locator)?;
    let attempt = AttemptId::from_token(&attempt)?;
    let state = directory.inspect_attempt(&locator, &attempt, meta, key)?;
    if !matches!(
        state,
        AttemptState::AuthenticatedPublishedCandidate
            | AttemptState::AuthenticatedStagingAndPublishedCandidate
    ) {
        return Err(invalid());
    }
    let body = Zeroizing::new(directory.read(&locator, meta, key)?);
    db.verify_body(item, &body)?;
    Ok(())
}
fn tokens(item: &BodyMigrationItem) -> Result<(ObjectLocator, AttemptId)> {
    Ok((
        ObjectLocator::from_token(item.locator.as_deref().ok_or_else(invalid)?)?,
        AttemptId::from_token(item.attempt_id.as_deref().ok_or_else(invalid)?)?,
    ))
}
fn verify_inventory(directory: &ObjectDirectory, items: &[BodyMigrationItem]) -> Result<()> {
    let attempts = items
        .iter()
        .filter(|i| i.state != BodyMigrationState::Planned)
        .map(tokens)
        .collect::<Result<Vec<_>>>()?;
    Ok(directory.verify_inventory(&attempts)?)
}
fn metadata(item: &BodyMigrationItem) -> Result<ObjectMetadata> {
    Ok(ObjectMetadata::new(
        &item.namespace_id,
        &item.source_id,
        decode_digest(&item.digest_value)?,
        item.content_length,
        &item.media_type,
    )?)
}
pub(crate) fn decode_digest(value: &str) -> Result<[u8; 32]> {
    let mut digest = [0; 32];
    if value.len() != 64 || !value.is_ascii() {
        return Err(invalid());
    }
    for (i, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).map_err(|_| invalid())?;
    }
    Ok(digest)
}
fn invalid() -> VaultMaintenanceError {
    SourceVaultError::new(
        SourceVaultErrorCode::AttemptMismatch,
        "invalid migration state or object reference",
    )
    .into()
}

#[cfg(test)]
mod tests;
