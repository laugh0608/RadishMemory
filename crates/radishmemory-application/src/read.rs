//! Shared application read use cases. The caller owns the complete read session,
//! including source resolution and publication of an authorized export.
use crate::{
    ApplicationError, ApplicationOperation, Citation, FileExportReceipt, FileExportRequest,
    Identifier, LocalLibraryConfig, MediaType, NonEmptyText, Sensitivity, SourceArtifact,
    SourceLineageSummary, SourceSearchResult, SourceVersionSummary, Timestamp,
};
use radishmemory_core::{
    DeletionState, Governance, GovernedCanonicalObject, LocalSearch, LocalSearchHit,
    LocalSearchRequest, ProducerType, SourceCatalog, SourceCatalogRequest, SourceSearchScope,
    SourceVault,
};
use radishmemory_file_entry::export_managed_source;
use radishmemory_source_vault::LibraryReader;
use radishmemory_sqlite::{ResolvedSourceCitation, SqliteDatabase};

pub const SOURCE_COMPARISON_LIMIT: usize = 5;

/// A currently authorized citation, with source details from the same read snapshot.
pub struct SourceCitationEvidence {
    pub result: SourceSearchResult,
    pub original: SourceArtifact,
    pub versions: Vec<SourceVersionSummary>,
}

pub(crate) fn validate_citation_batch(
    citations: &[Citation],
    allowed: &[Sensitivity],
) -> Result<(), ApplicationError> {
    if citations.len() > SOURCE_COMPARISON_LIMIT || allowed.is_empty() {
        return Err(ApplicationError::invalid_citation_batch());
    }
    Ok(())
}

pub(crate) fn visible_citations(
    resolved: Vec<Option<ResolvedSourceCitation>>,
    allowed: &[Sensitivity],
    now: &Timestamp,
) -> Vec<Option<SourceCitationEvidence>> {
    let visible = |governance: &Governance| {
        governance.deletion_state() == DeletionState::Active
            && allowed.contains(&governance.sensitivity())
            && governance
                .retention()
                .expires_at()
                .is_none_or(|end| end > now)
    };
    resolved
        .into_iter()
        .map(|entry| {
            let entry = entry?;
            if entry.source.params().captured_at > *now
                || !visible(&entry.source.params().governance)
                || !visible(entry.fragment.governance())
            {
                return None;
            }
            Some(SourceCitationEvidence {
                result: SourceSearchResult::from_resolved(&entry.fragment, &entry.source),
                original: entry.source,
                versions: entry.versions,
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceEntry {
    Note,
    FileImport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceSavedWithin {
    Day,
    Week,
    ThirtyDays,
}

impl SourceSavedWithin {
    const fn seconds(self) -> u64 {
        let days = match self {
            Self::Day => 1,
            Self::Week => 7,
            Self::ThirtyDays => 30,
        };
        days * 86_400
    }
}

/// Filters on the saved current version. Missing fields include every value in that dimension.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceSearchFilters {
    pub entry: Option<SourceEntry>,
    pub media_type: Option<MediaType>,
    pub saved_within: Option<SourceSavedWithin>,
}

/// A private read-only adapter boundary, not a second canonical storage port.
pub(crate) trait SourceReader: SourceCatalog + LocalSearch {
    fn source(
        &self,
        namespace: &Identifier,
        source: &Identifier,
        operation: ApplicationOperation,
    ) -> Result<Option<SourceArtifact>, ApplicationError>;
    fn catalog_error(
        operation: ApplicationOperation,
        error: <Self as SourceCatalog>::Error,
    ) -> ApplicationError;
    fn search_error(
        operation: ApplicationOperation,
        error: <Self as LocalSearch>::Error,
    ) -> ApplicationError;
}

impl SourceReader for SqliteDatabase {
    fn source(
        &self,
        namespace: &Identifier,
        source: &Identifier,
        operation: ApplicationOperation,
    ) -> Result<Option<SourceArtifact>, ApplicationError> {
        self.load_source_artifact(namespace, source)
            .map_err(|error| ApplicationError::storage(operation, error))
    }

    fn catalog_error(
        operation: ApplicationOperation,
        error: <Self as SourceCatalog>::Error,
    ) -> ApplicationError {
        ApplicationError::storage(operation, error)
    }

    fn search_error(
        operation: ApplicationOperation,
        error: <Self as LocalSearch>::Error,
    ) -> ApplicationError {
        ApplicationError::storage(operation, error)
    }
}

impl SourceReader for LibraryReader<'_> {
    fn source(
        &self,
        namespace: &Identifier,
        source: &Identifier,
        operation: ApplicationOperation,
    ) -> Result<Option<SourceArtifact>, ApplicationError> {
        self.load_source_artifact(namespace, source)
            .map_err(|error| ApplicationError::vault(operation, error))
    }

    fn catalog_error(
        operation: ApplicationOperation,
        error: <Self as SourceCatalog>::Error,
    ) -> ApplicationError {
        ApplicationError::vault(operation, error)
    }

    fn search_error(
        operation: ApplicationOperation,
        error: <Self as LocalSearch>::Error,
    ) -> ApplicationError {
        ApplicationError::vault(operation, error)
    }
}

pub(crate) fn list_sources<S: SourceReader>(
    reader: &S,
    config: &LocalLibraryConfig,
    offset: u64,
    limit: usize,
) -> Result<Vec<SourceLineageSummary>, ApplicationError> {
    let operation = ApplicationOperation::ListSources;
    let request = SourceCatalogRequest::new(config.namespace_id.clone(), offset, limit)
        .map_err(|error| ApplicationError::canonical(operation, error))?;
    reader
        .list_source_lineages(&request)
        .map_err(|error| S::catalog_error(operation, error))
}

pub(crate) fn list_source_versions<S: SourceReader>(
    reader: &S,
    config: &LocalLibraryConfig,
    lineage: &Identifier,
) -> Result<Vec<SourceVersionSummary>, ApplicationError> {
    reader
        .list_source_versions(&config.namespace_id, lineage)
        .map_err(|error| S::catalog_error(ApplicationOperation::ListSources, error))
}

pub(crate) fn get_source(
    reader: &impl SourceReader,
    config: &LocalLibraryConfig,
    source: &Identifier,
) -> Result<Option<SourceArtifact>, ApplicationError> {
    reader.source(
        &config.namespace_id,
        source,
        ApplicationOperation::GetSource,
    )
}

pub(crate) fn search_sources<S: SourceReader>(
    reader: &S,
    config: &LocalLibraryConfig,
    query: NonEmptyText,
    now: Timestamp,
    top_k: usize,
    allowed_sensitivities: impl IntoIterator<Item = Sensitivity>,
    filters: SourceSearchFilters,
) -> Result<Vec<SourceSearchResult>, ApplicationError> {
    let operation = ApplicationOperation::SearchSources;
    let user_input_producer = match filters.entry {
        Some(SourceEntry::Note) => Some((
            ProducerType::System,
            Identifier::new(crate::capture::NOTE_PRODUCER_ID)
                .map_err(|error| ApplicationError::canonical(operation, error))?,
        )),
        Some(SourceEntry::FileImport) => Some((
            config.source_producer.producer_type(),
            config.source_producer.producer_id().clone(),
        )),
        None => None,
    };
    let request = LocalSearchRequest::new(
        config.namespace_id.clone(),
        query,
        now,
        top_k,
        allowed_sensitivities,
    )
    .map_err(|error| ApplicationError::canonical(operation, error))?
    .for_sources(SourceSearchScope {
        user_input_producer,
        media_type: filters.media_type,
        saved_within_seconds: filters.saved_within.map(SourceSavedWithin::seconds),
    });
    let hits = reader
        .search(&request)
        .map_err(|error| S::search_error(operation, error))?;
    let _cost = radishmemory_sqlite::read_cost::span(
        radishmemory_sqlite::read_cost::Stage::ResultResolution,
    );
    let mut results = Vec::new();
    for hit in hits {
        let LocalSearchHit::SourceFragment(fragment) = hit else {
            continue;
        };
        let source = reader
            .source(
                &config.namespace_id,
                &fragment.params().source_id,
                operation,
            )?
            .ok_or_else(|| ApplicationError::source_not_found(operation))?;
        results.push(SourceSearchResult::from_resolved(&fragment, &source));
    }
    Ok(results)
}

pub(crate) fn export_source(
    reader: &impl SourceReader,
    config: &LocalLibraryConfig,
    source: &Identifier,
    request: &FileExportRequest,
) -> Result<FileExportReceipt, ApplicationError> {
    let operation = ApplicationOperation::ExportSource;
    let source = reader
        .source(&config.namespace_id, source, operation)?
        .ok_or_else(|| ApplicationError::source_not_found(operation))?;
    export_managed_source(&source, request)
        .map_err(|error| ApplicationError::file_entry(operation, error))
}
