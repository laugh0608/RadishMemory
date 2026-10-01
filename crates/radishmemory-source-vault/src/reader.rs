//! Explicit read session for a migrated library, with no plaintext fallback.
use crate::filesystem_support::DatabaseObservation;
use crate::{
    KeyEncryptionKey, ObjectDirectory, PROVIDER_PROFILE, SourceVaultError, SourceVaultErrorCode,
    VaultMaintenanceError,
};
use radishmemory_core::{
    Identifier, LocalSearch, LocalSearchHit, LocalSearchRequest, SourceArtifact, SourceCatalog,
    SourceCatalogRequest, SourceFragment, SourceLineageState, SourceLineageSummary,
    SourceVersionSummary,
};
use radishmemory_sqlite::{EncryptedCaptureDatabase, ObjectReadView};
use std::fmt;
type Result<T> = std::result::Result<T, VaultMaintenanceError>;

/// Holds the existing key and an exclusive database session until dropped.
/// Every query authenticates objects and validates canonical/derived facts before
/// and after resolving results. No decrypted source cache survives a query.
/// Returned canonical values are owned plaintext; callers control their lifetime.
/// Keep this session alive through a user-authorized export to prevent concurrent
/// canonical deletion. This does not revoke previously returned copies.
pub struct LibraryReader<'a> {
    directory: &'a ObjectDirectory,
    database: EncryptedCaptureDatabase,
    identity: DatabaseObservation,
    key: KeyEncryptionKey,
    namespace: String,
}

pub(crate) fn open<'a>(
    directory: &'a ObjectDirectory,
    namespace: &str,
    device: &str,
    load_existing: impl FnOnce() -> std::result::Result<KeyEncryptionKey, SourceVaultError>,
) -> Result<LibraryReader<'a>> {
    let path = directory.key_database_path()?;
    let identity = DatabaseObservation::open(&path)?;
    let database = EncryptedCaptureDatabase::open(&path, namespace, device, PROVIDER_PROFILE)?;
    directory.key_database_path()?;
    identity.verify_identity(&path)?;
    let key = load_existing()?;
    let reader = LibraryReader {
        directory,
        database,
        identity,
        key,
        namespace: namespace.into(),
    };
    reader.authenticated_sources()?;
    Ok(reader)
}
impl LibraryReader<'_> {
    fn verify_identity(&self) -> Result<()> {
        let path = self.directory.key_database_path()?;
        self.identity.verify_identity(&path)?;
        Ok(())
    }
    fn authenticated_sources(&self) -> Result<Vec<SourceArtifact>> {
        self.verify_identity()?;
        let sources = crate::capture::authenticate_all(&self.database, self.directory, &self.key)?;
        self.database.read_view(&sources)?;
        self.verify_identity()?;
        Ok(sources)
    }
    fn read<T>(
        &self,
        namespace: &Identifier,
        query: impl FnOnce(
            &ObjectReadView<'_>,
        ) -> std::result::Result<T, radishmemory_sqlite::SqliteError>,
    ) -> Result<T> {
        self.read_with_check(namespace, query, || Ok(()))
    }
    fn read_with_check<T>(
        &self,
        namespace: &Identifier,
        query: impl FnOnce(
            &ObjectReadView<'_>,
        ) -> std::result::Result<T, radishmemory_sqlite::SqliteError>,
        before_return: impl FnOnce() -> Result<()>,
    ) -> Result<T> {
        if namespace.as_str() != self.namespace {
            return Err(SourceVaultError::new(
                SourceVaultErrorCode::MetadataMismatch,
                "reader namespace mismatch",
            )
            .into());
        }
        self.verify_identity()?;
        let sources = crate::capture::authenticate_all(&self.database, self.directory, &self.key)?;
        let view = self.database.read_view(&sources)?;
        let result = query(&view)?;
        before_return()?;
        self.authenticated_sources()?;
        Ok(result)
    }
    /// Authenticated pending work blocks preparing a different mutation.
    pub fn mutation_pending(&self, namespace: &Identifier) -> Result<bool> {
        self.read(namespace, |view| view.mutation_pending())
    }
    pub fn resolve_source_lineage_deletion_targets(
        &self,
        namespace: &Identifier,
        lineage: &Identifier,
    ) -> Result<Vec<radishmemory_core::ObjectRef>> {
        self.read(namespace, |view| {
            view.resolve_source_lineage_deletion_targets(namespace, lineage)
        })
    }
    pub fn load_delete_request(
        &self,
        namespace: &Identifier,
        request: &Identifier,
    ) -> Result<Option<radishmemory_core::DeleteRequest>> {
        self.read(namespace, |view| {
            view.load_delete_request(namespace, request)
        })
    }
    pub fn load_deletion_evidence(
        &self,
        namespace: &Identifier,
        evidence: &Identifier,
    ) -> Result<Option<radishmemory_core::DeletionEvidence>> {
        self.read(namespace, |view| {
            view.load_deletion_evidence(namespace, evidence)
        })
    }
    pub fn unfinished_delete_requests(
        &self,
        namespace: &Identifier,
    ) -> Result<Vec<radishmemory_core::DeleteRequest>> {
        self.read(namespace, |view| view.unfinished_delete_requests(namespace))
    }
    pub fn latest_deletion_evidence(
        &self,
        namespace: &Identifier,
        request: &Identifier,
    ) -> Result<Option<radishmemory_core::DeletionEvidence>> {
        self.read(namespace, |view| {
            view.latest_deletion_evidence(namespace, request)
        })
    }
    /// Resolves an active source by the session namespace and immutable ID.
    /// Missing and deletion-closed sources return None; corruption returns an error.
    pub fn load_source_artifact(
        &self,
        namespace: &Identifier,
        source: &Identifier,
    ) -> Result<Option<SourceArtifact>> {
        self.read(namespace, |view| {
            view.load_source_artifact(namespace, source)
        })
    }
    pub fn load_source_fragments(
        &self,
        namespace: &Identifier,
        source: &Identifier,
    ) -> Result<Option<Vec<SourceFragment>>> {
        self.read(namespace, |view| {
            view.load_source_fragments(namespace, source)
        })
    }
}
impl SourceCatalog for LibraryReader<'_> {
    type Error = VaultMaintenanceError;
    fn resolve_source_lineage(
        &self,
        namespace: &Identifier,
        lineage: &Identifier,
    ) -> Result<Option<SourceLineageState>> {
        self.read(namespace, |view| {
            view.resolve_source_lineage(namespace, lineage)
        })
    }
    fn list_source_lineages(
        &self,
        request: &SourceCatalogRequest,
    ) -> Result<Vec<SourceLineageSummary>> {
        self.read(request.namespace_id(), |view| {
            view.list_source_lineages(request)
        })
    }
    fn list_source_versions(
        &self,
        namespace: &Identifier,
        lineage: &Identifier,
    ) -> Result<Vec<SourceVersionSummary>> {
        self.read(namespace, |view| {
            view.list_source_versions(namespace, lineage)
        })
    }
}
impl LocalSearch for LibraryReader<'_> {
    type Error = VaultMaintenanceError;
    fn search(&self, request: &LocalSearchRequest) -> Result<Vec<LocalSearchHit>> {
        self.read(request.namespace_id(), |view| view.search(request))
    }
}
impl fmt::Debug for LibraryReader<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LibraryReader([REDACTED])")
    }
}

#[cfg(test)]
mod tests;
