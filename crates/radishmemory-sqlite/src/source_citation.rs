//! Resolve immutable citations and their version state in one read snapshot.
use radishmemory_core::{
    Citation, Identifier, SourceArtifact, SourceFragment, SourceVersionSummary,
};
use rusqlite::Connection;

use crate::source_store::{SourceLoader, load_fragments_for_source, load_source_artifact};
use crate::{SqliteDatabase, SqliteError, SqliteStorageReason};

pub struct ResolvedSourceCitation {
    pub source: SourceArtifact,
    pub fragment: SourceFragment,
    pub versions: Vec<SourceVersionSummary>,
}

impl SqliteDatabase {
    pub fn resolve_source_citations(
        &self,
        namespace: &Identifier,
        citations: &[Citation],
    ) -> Result<Vec<Option<ResolvedSourceCitation>>, SqliteError> {
        let snapshot = self
            .connection
            .unchecked_transaction()
            .map_err(SqliteError::storage)?;
        crate::source_capture::verify_origin_bindings(&snapshot)?;
        let resolved = resolve(&snapshot, namespace, citations, &load_source_artifact)?;
        snapshot.commit().map_err(SqliteError::storage)?;
        Ok(resolved)
    }
}

pub(crate) fn resolve(
    connection: &Connection,
    namespace: &Identifier,
    citations: &[Citation],
    load_source: SourceLoader<'_>,
) -> Result<Vec<Option<ResolvedSourceCitation>>, SqliteError> {
    citations
        .iter()
        .map(|citation| {
            let Some(source) = load_source(connection, namespace, citation.source_id())? else {
                return Ok(None);
            };
            let Some(fragment) = load_fragments_for_source(connection, &source)?
                .into_iter()
                .find(|fragment| &fragment.params().fragment_id == citation.fragment_id())
            else {
                return Ok(None);
            };
            if !citation.matches_fragment(&fragment) {
                return Err(SqliteError::invalid_stored(
                    SqliteStorageReason::SourceResolution,
                ));
            }
            let versions = crate::source_catalog::list_source_versions_with(
                connection,
                namespace,
                &source.params().lineage_id,
                load_source,
            )?;
            if !versions
                .iter()
                .any(|version| version.source_id() == citation.source_id())
            {
                return Err(SqliteError::invalid_stored(
                    SqliteStorageReason::LineageTipMismatch,
                ));
            }
            Ok(Some(ResolvedSourceCitation {
                source,
                fragment,
                versions,
            }))
        })
        .collect()
}
