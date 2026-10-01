//! Shared application read use cases. The caller owns the complete read session,
//! including source resolution and publication of an authorized export.
use crate::{
    ApplicationError, ApplicationOperation, FileExportReceipt, FileExportRequest, Identifier,
    LocalLibraryConfig, NonEmptyText, Sensitivity, SourceArtifact, SourceLineageSummary,
    SourceSearchResult, SourceVersionSummary, Timestamp,
};
use radishmemory_core::{
    LocalSearch, LocalSearchHit, LocalSearchRequest, SourceCatalog, SourceCatalogRequest,
    SourceVault,
};
use radishmemory_file_entry::export_managed_source;
use radishmemory_sqlite::SqliteDatabase;

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

pub(crate) fn list_sources(
    reader: &impl SourceReader,
    config: &LocalLibraryConfig,
    offset: u64,
    limit: usize,
) -> Result<Vec<SourceLineageSummary>, ApplicationError> {
    let operation = ApplicationOperation::ListSources;
    let request = SourceCatalogRequest::new(config.namespace_id.clone(), offset, limit)
        .map_err(|error| ApplicationError::canonical(operation, error))?;
    catalog(reader, operation, |reader| {
        reader.list_source_lineages(&request)
    })
}

fn catalog<S: SourceReader, T>(
    reader: &S,
    operation: ApplicationOperation,
    query: impl FnOnce(&S) -> Result<T, <S as SourceCatalog>::Error>,
) -> Result<T, ApplicationError> {
    query(reader).map_err(|error| S::catalog_error(operation, error))
}

pub(crate) fn list_source_versions(
    reader: &impl SourceReader,
    config: &LocalLibraryConfig,
    lineage: &Identifier,
) -> Result<Vec<SourceVersionSummary>, ApplicationError> {
    catalog(reader, ApplicationOperation::ListSources, |reader| {
        reader.list_source_versions(&config.namespace_id, lineage)
    })
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
) -> Result<Vec<SourceSearchResult>, ApplicationError> {
    let operation = ApplicationOperation::SearchSources;
    let request = LocalSearchRequest::new(
        config.namespace_id.clone(),
        query,
        now,
        top_k,
        allowed_sensitivities,
    )
    .map_err(|error| ApplicationError::canonical(operation, error))?;
    let hits = reader
        .search(&request)
        .map_err(|error| S::search_error(operation, error))?;
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
