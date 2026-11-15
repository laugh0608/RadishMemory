//! Read-only startup eligibility for an existing local library.
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OpenFlags};

use crate::{
    SQLITE_SCHEMA_VERSION, SqliteConfigurationReason, SqliteError, SqliteStorageReason,
    encrypted_capture, migration, source_vault_migration, vault_session,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibraryFormat {
    Plaintext,
    MigrationPending,
    EncryptedObjects,
}

/// Checks the persisted format and migration checkpoint without creating a file,
/// opening the object directory, or contacting the key provider. Authentication
/// and complete library verification are still required at explicit open.
pub fn inspect_library_format(
    path: &Path,
    namespace: &str,
    device: &str,
    provider: &str,
) -> Result<LibraryFormat, SqliteError> {
    let mut connection = open_existing(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    require_delete_journal(&connection)?;
    let readback = connection.transaction().map_err(SqliteError::storage)?;
    let supported = migration::LEGACY_DELETION_SCHEMA_VERSION;
    let found = migration::preflight_to(&readback, supported)?;
    let version = u32::try_from(found)
        .map_err(|_| SqliteError::unsupported_schema_version(found, supported))?;
    let format = match version {
        SQLITE_SCHEMA_VERSION => LibraryFormat::Plaintext,
        migration::KEY_INITIALIZATION_SCHEMA_VERSION => {
            vault_session::verify_key_profile(&readback, namespace, device, provider)?;
            LibraryFormat::MigrationPending
        }
        migration::OBJECT_MIGRATION_SCHEMA_VERSION..=migration::LEGACY_DELETION_SCHEMA_VERSION => {
            vault_session::verify_key_profile(&readback, namespace, device, provider)?;
            if source_vault_migration::body_migration_complete(&readback)? {
                encrypted_capture::require_no_inline_bodies(&readback)?;
                LibraryFormat::EncryptedObjects
            } else if version == migration::OBJECT_MIGRATION_SCHEMA_VERSION {
                LibraryFormat::MigrationPending
            } else {
                return Err(SqliteError::invalid_stored(
                    SqliteStorageReason::StoredIntegrityMismatch,
                ));
            }
        }
        _ => return Err(SqliteError::unsupported_schema_version(found, supported)),
    };
    readback.commit().map_err(SqliteError::storage)?;
    Ok(format)
}

pub(crate) fn resolve_database_path(path: &Path) -> Result<PathBuf, SqliteError> {
    let invalid_path = || SqliteError::open_path(io::ErrorKind::InvalidInput.into());
    let name = path.file_name().ok_or_else(invalid_path)?;
    let directory = path.parent().ok_or_else(invalid_path)?;
    if !path.is_absolute()
        || !fs::symlink_metadata(directory)
            .map_err(SqliteError::open_path)?
            .is_dir()
    {
        return Err(invalid_path());
    }
    // Resolve platform ancestor aliases such as macOS /var. The application
    // directory and database themselves must not be symlinks.
    Ok(directory
        .canonicalize()
        .map_err(SqliteError::open_path)?
        .join(name))
}

pub(crate) fn open_existing(path: &Path, access: OpenFlags) -> Result<Connection, SqliteError> {
    let path = resolve_database_path(path)?;
    if !fs::symlink_metadata(&path)
        .map_err(SqliteError::open_path)?
        .is_file()
    {
        return Err(SqliteError::open_path(io::ErrorKind::InvalidInput.into()));
    }
    Connection::open_with_flags(
        path,
        access | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(SqliteError::open)
}

pub(crate) fn require_delete_journal(connection: &Connection) -> Result<(), SqliteError> {
    // Reject unexpected journal modes without converting an existing file.
    let journal: String = connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(SqliteError::configuration_source)?;
    if journal != "delete" {
        return Err(SqliteError::configuration(
            SqliteConfigurationReason::PersistentJournalMode,
        ));
    }
    Ok(())
}

pub(crate) fn require_v6(connection: &Connection) -> Result<(), SqliteError> {
    let version = migration::preflight(connection)?;
    if version != i64::from(SQLITE_SCHEMA_VERSION) {
        return Err(SqliteError::unsupported_schema_version(
            version,
            SQLITE_SCHEMA_VERSION,
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod support;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SqliteDatabase, SqliteErrorCode};

    #[test]
    fn startup_requires_current_plaintext_without_creating_or_upgrading() {
        let fixture = support::SyntheticDatabase::new("startup-plain");
        let path = fixture.path();
        assert!(SqliteDatabase::open_current(path).is_err());
        assert!(!path.exists());

        let mut connection = Connection::open(path).unwrap();
        let tx = connection.transaction().unwrap();
        migration::apply_pending(&tx, 0, SQLITE_SCHEMA_VERSION - 1).unwrap();
        tx.commit().unwrap();
        drop(connection);

        let before = fs::read(path).unwrap();
        let error = inspect_library_format(path, "ns", "device", "provider").unwrap_err();
        assert_eq!(error.code(), SqliteErrorCode::UnsupportedSchemaVersion);
        let error = SqliteDatabase::open_current(path).unwrap_err();
        assert_eq!(error.code(), SqliteErrorCode::UnsupportedSchemaVersion);
        assert!(SqliteDatabase::create_new(path).is_err());
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[test]
    fn object_formats_use_the_migration_checkpoint_and_key_profile_binding() {
        let fixture = support::SyntheticDatabase::new("startup-objects");
        let path = fixture.path();
        drop(SqliteDatabase::create_new(path).unwrap());
        let mut connection = Connection::open(path).unwrap();
        let mut previous = SQLITE_SCHEMA_VERSION;
        let inspect = || inspect_library_format(path, "ns", "device", "provider");
        for version in
            migration::KEY_INITIALIZATION_SCHEMA_VERSION..=migration::LEGACY_DELETION_SCHEMA_VERSION
        {
            let tx = connection.transaction().unwrap();
            migration::apply_pending(&tx, i64::from(previous), version).unwrap();
            if version == migration::KEY_INITIALIZATION_SCHEMA_VERSION {
                tx.execute(
                    "INSERT INTO radishmemory_source_vault_key_profile VALUES (1,'ns','device','provider','key_ready')",
                    [],
                ).unwrap();
            }
            if version == migration::OBJECT_MIGRATION_SCHEMA_VERSION {
                tx.execute(
                    "INSERT INTO radishmemory_source_vault_migration VALUES (1,'objects_ready')",
                    [],
                )
                .unwrap();
            }
            tx.commit().unwrap();
            let expected = if version == migration::KEY_INITIALIZATION_SCHEMA_VERSION {
                LibraryFormat::MigrationPending
            } else {
                LibraryFormat::EncryptedObjects
            };
            let before = fs::read(path).unwrap();
            assert_eq!(inspect().unwrap(), expected, "version {version}");
            let rejected =
                inspect_library_format(path, "ns", "other-device", "provider").unwrap_err();
            assert_eq!(
                rejected.storage_reason(),
                Some(SqliteStorageReason::KeyProfileMismatch)
            );
            assert_eq!(fs::read(path).unwrap(), before);
            previous = version;
        }
        connection
            .execute(
                "UPDATE radishmemory_source_vault_migration SET state='migrating'",
                [],
            )
            .unwrap();
        assert!(inspect().is_err());
        connection
            .execute("DELETE FROM radishmemory_source_vault_migration", [])
            .unwrap();
        assert!(inspect().is_err());
    }
}
