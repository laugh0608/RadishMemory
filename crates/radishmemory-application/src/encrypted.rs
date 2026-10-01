//! Explicit vault preparation and per-use-case sessions. The desktop's default
//! v6 entry remains unchanged until host acceptance authorizes its transition.
mod write;

use std::{fmt, path::Path};

use radishmemory_source_vault::{
    BodyMigrationReport, KeySlot, LibraryProvider, LibraryReader, ObjectDirectory,
    PlatformKeyProvider,
};

use crate::{
    ApplicationError, ApplicationOperation, ApplicationRuntime, FileExportReceipt,
    FileExportRequest, Identifier, LocalLibraryConfig, NonEmptyText, Sensitivity, SourceArtifact,
    SourceLineageSummary, SourceSearchResult, SourceVersionSummary, read,
};

/// An explicit application root and verified host profile, with no live database
/// connection or cached key. Its database is always the root's `library.sqlite3`.
/// The caller must close legacy handles and suspend other operations before
/// initialization or migration. Real provider calls require platform authorization,
/// off-UI-thread execution, and the provider's upstream-log filtering policy.
pub struct EncryptedLibraryLocation<P = PlatformKeyProvider> {
    directory: ObjectDirectory,
    provider: P,
    config: LocalLibraryConfig,
}

impl EncryptedLibraryLocation {
    /// Opens the dedicated directory capability; does not access the OS key store,
    /// create a database, initialize a key, or migrate bodies.
    pub fn new(
        application_root: impl AsRef<Path>,
        config: LocalLibraryConfig,
    ) -> Result<Self, ApplicationError> {
        Self::with_provider(application_root, config, PlatformKeyProvider::new())
    }
}

impl<P: LibraryProvider> EncryptedLibraryLocation<P> {
    /// Provider substitution is sealed by Source Vault; synthetic substitution is
    /// only available through its explicit acceptance-test-support feature.
    pub fn with_provider(
        application_root: impl AsRef<Path>,
        config: LocalLibraryConfig,
        provider: P,
    ) -> Result<Self, ApplicationError> {
        let operation = ApplicationOperation::OpenLibrary;
        KeySlot::new(
            config.namespace_id.as_str(),
            config.deletion.device_id.as_str(),
        )
        .map_err(|error| ApplicationError::vault(operation, error.into()))?;
        let directory = ObjectDirectory::open_application_directory(application_root)
            .map_err(|error| ApplicationError::vault(operation, error.into()))?;
        Ok(Self {
            directory,
            provider,
            config,
        })
    }

    /// Explicit initial preparation, including SQLite-owned bootstrap eligibility.
    /// An existing key checkpoint only loads its existing key. Never call this as
    /// recovery from a failed ordinary open; it may create a new library/key.
    pub fn initialize_key(&self) -> Result<(), ApplicationError> {
        let key = self
            .provider
            .initialize_library_key(
                &self.directory,
                self.config.namespace_id.as_str(),
                self.config.deletion.device_id.as_str(),
            )
            .map_err(|error| {
                ApplicationError::vault(ApplicationOperation::InitializeLibraryKey, error)
            })?;
        drop(key);
        Ok(())
    }

    /// Explicit v7/v8 body migration or exact recovery of its existing checkpoint.
    /// Only loads an existing key; later v9-v12 libraries use ordinary open.
    pub fn migrate_bodies(&self) -> Result<BodyMigrationReport, ApplicationError> {
        self.provider
            .migrate_library_bodies(
                &self.directory,
                self.config.namespace_id.as_str(),
                self.config.deletion.device_id.as_str(),
            )
            .map_err(|error| {
                ApplicationError::vault(ApplicationOperation::MigrateLibraryBodies, error)
            })
    }

    /// Explicit verification is available even when ordinary open fails.
    pub fn verify_library(
        &self,
    ) -> Result<radishmemory_source_vault::VerificationReport, ApplicationError> {
        self.provider
            .verify_library_objects(
                &self.directory,
                self.config.namespace_id.as_str(),
                self.config.deletion.device_id.as_str(),
            )
            .map_err(|error| ApplicationError::vault(ApplicationOperation::VerifyLibrary, error))
    }

    /// Repair only rebuildable derivations from authenticated canonical objects.
    /// Never initializes a key, repairs canonical data, or resumes pending writes.
    pub fn rebuild_recall(
        &self,
    ) -> Result<radishmemory_source_vault::VerificationReport, ApplicationError> {
        self.provider
            .rebuild_library_derivations(
                &self.directory,
                self.config.namespace_id.as_str(),
                self.config.deletion.device_id.as_str(),
            )
            .map_err(|error| ApplicationError::vault(ApplicationOperation::RebuildRecall, error))
    }

    /// Select pending uncommitted work; selection does not authorize its removal.
    pub fn inspect_capture_abandonment(
        &self,
    ) -> Result<Option<radishmemory_source_vault::CaptureAbandonmentTarget>, ApplicationError> {
        self.provider
            .inspect_library_capture_abandonment(
                &self.directory,
                self.config.namespace_id.as_str(),
                self.config.deletion.device_id.as_str(),
            )
            .map_err(|error| ApplicationError::vault(ApplicationOperation::InspectRecovery, error))
    }

    /// Explicitly authorized exact abandonment; original active sources are never targets.
    pub fn abandon_capture(
        &self,
        target: &radishmemory_source_vault::CaptureAbandonmentTarget,
    ) -> Result<radishmemory_source_vault::AbandonmentReport, ApplicationError> {
        self.provider
            .abandon_library_capture(
                &self.directory,
                self.config.namespace_id.as_str(),
                self.config.deletion.device_id.as_str(),
                target,
            )
            .map_err(|error| ApplicationError::vault(ApplicationOperation::AbandonCapture, error))
    }

    /// Authenticate a migrated library, then release the opening session before
    /// returning. Missing/unready/damaged libraries fail without fallback or repair.
    pub fn open<R: ApplicationRuntime>(
        self,
        runtime: R,
    ) -> Result<EncryptedLibrary<R, P>, ApplicationError> {
        let library = EncryptedLibrary {
            location: self,
            runtime,
        };
        library.with_reader(ApplicationOperation::OpenLibrary, |_| Ok(()))?;
        Ok(library)
    }
}

/// Explicit application use cases over the same canonical migrated library.
/// Each operation acquires a fresh authenticated session; no exclusive lock or key
/// survives between operations. Returned values and exports are caller-owned plaintext.
pub struct EncryptedLibrary<R, P = PlatformKeyProvider> {
    location: EncryptedLibraryLocation<P>,
    runtime: R,
}

impl<R: ApplicationRuntime, P: LibraryProvider> EncryptedLibrary<R, P> {
    fn with_reader<T>(
        &self,
        operation: ApplicationOperation,
        use_case: impl FnOnce(&LibraryReader<'_>) -> Result<T, ApplicationError>,
    ) -> Result<T, ApplicationError> {
        let reader = self
            .location
            .provider
            .open_library_reader(
                &self.location.directory,
                self.location.config.namespace_id.as_str(),
                self.location.config.deletion.device_id.as_str(),
            )
            .map_err(|error| ApplicationError::vault(operation, error))?;
        // Keep this named session until the complete use case returns (also on
        // errors/unwind). In particular, export publication runs under this lock.
        use_case(&reader)
    }

    #[must_use]
    pub fn close(self) -> (R, EncryptedLibraryLocation<P>) {
        (self.runtime, self.location)
    }

    pub fn list_sources(
        &self,
        offset: u64,
        limit: usize,
    ) -> Result<Vec<SourceLineageSummary>, ApplicationError> {
        self.with_reader(ApplicationOperation::ListSources, |reader| {
            read::list_sources(reader, &self.location.config, offset, limit)
        })
    }

    pub fn list_source_versions(
        &self,
        lineage: &Identifier,
    ) -> Result<Vec<SourceVersionSummary>, ApplicationError> {
        self.with_reader(ApplicationOperation::ListSources, |reader| {
            read::list_source_versions(reader, &self.location.config, lineage)
        })
    }

    pub fn get_source(
        &self,
        source: &Identifier,
    ) -> Result<Option<SourceArtifact>, ApplicationError> {
        self.with_reader(ApplicationOperation::GetSource, |reader| {
            read::get_source(reader, &self.location.config, source)
        })
    }

    pub fn search_sources(
        &mut self,
        query: NonEmptyText,
        top_k: usize,
        allowed_sensitivities: impl IntoIterator<Item = Sensitivity>,
    ) -> Result<Vec<SourceSearchResult>, ApplicationError> {
        let operation = ApplicationOperation::SearchSources;
        let now = self
            .runtime
            .now()
            .map_err(|error| ApplicationError::clock(operation, error))?;
        self.with_reader(operation, |reader| {
            read::search_sources(
                reader,
                &self.location.config,
                query,
                now,
                top_k,
                allowed_sensitivities,
            )
        })
    }

    pub fn export_source(
        &self,
        source: &Identifier,
        request: &FileExportRequest,
    ) -> Result<FileExportReceipt, ApplicationError> {
        self.with_reader(ApplicationOperation::ExportSource, |reader| {
            read::export_source(reader, &self.location.config, source, request)
        })
    }
}

impl<P> fmt::Debug for EncryptedLibraryLocation<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EncryptedLibraryLocation([REDACTED])")
    }
}

impl<R, P> fmt::Debug for EncryptedLibrary<R, P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EncryptedLibrary([REDACTED])")
    }
}

#[cfg(test)]
mod tests;
