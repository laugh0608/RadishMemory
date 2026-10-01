//! Queries over a caller-authenticated source set; never loads an inline body.
use super::*;
use radishmemory_core::{
    LocalSearch, LocalSearchHit, LocalSearchRequest, SourceCatalog, SourceCatalogRequest,
    SourceFragment, SourceLineageState, SourceLineageSummary, SourceVersionSummary,
};

/// Borrowed SQLite query view. The Source Vault coordinator authenticates bytes
/// before construction and rechecks file identities before releasing results.
/// This is not an independent filesystem authentication capability.
pub struct ObjectReadView<'a> {
    database: &'a EncryptedCaptureDatabase,
    sources: &'a [SourceArtifact],
}
impl EncryptedCaptureDatabase {
    pub fn read_view<'a>(&'a self, sources: &'a [SourceArtifact]) -> Result<ObjectReadView<'a>> {
        self.verify_facts(sources)?;
        self.verify_recall_index_structure()?;
        Ok(ObjectReadView {
            database: self,
            sources,
        })
    }
}
impl ObjectReadView<'_> {
    fn require_namespace(&self, namespace: &Identifier) -> Result<()> {
        if namespace.as_str() != self.database.namespace {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn mutation_pending(&self) -> Result<bool> {
        Ok(self.database.deletion_in_progress()?
            || self.database.objects()?.iter().any(|item| {
                matches!(
                    item.state,
                    CaptureObjectState::Prepared | CaptureObjectState::Abandoning
                )
            }))
    }
    pub fn resolve_source_lineage_deletion_targets(
        &self,
        namespace: &Identifier,
        lineage: &Identifier,
    ) -> Result<Vec<radishmemory_core::ObjectRef>> {
        self.require_namespace(namespace)?;
        crate::deletion_store::resolve_lineage_targets(
            &self.database.connection,
            namespace,
            lineage,
            &self.list_source_versions(namespace, lineage)?,
        )
    }
    pub fn load_delete_request(
        &self,
        namespace: &Identifier,
        request: &Identifier,
    ) -> Result<Option<radishmemory_core::DeleteRequest>> {
        self.require_namespace(namespace)?;
        crate::deletion_store::load_request(&self.database.connection, namespace, request)
    }
    pub fn load_deletion_evidence(
        &self,
        namespace: &Identifier,
        evidence: &Identifier,
    ) -> Result<Option<radishmemory_core::DeletionEvidence>> {
        self.require_namespace(namespace)?;
        crate::deletion_store::load_evidence(&self.database.connection, namespace, evidence)
    }
    /// Durable requests without a validated completed receipt, including execution
    /// completed before evidence commit. Discovery never authorizes execution.
    pub fn unfinished_delete_requests(
        &self,
        namespace: &Identifier,
    ) -> Result<Vec<radishmemory_core::DeleteRequest>> {
        self.require_namespace(namespace)?;
        let mut statement = self
            .database
            .connection
            .prepare(
                "SELECT delete_request_id FROM radishmemory_delete_requests
             WHERE namespace_id=?1 ORDER BY requested_at,delete_request_id",
            )
            .map_err(SqliteError::storage)?;
        let ids = statement
            .query_map([namespace.as_str()], |row| row.get::<_, String>(0))
            .map_err(SqliteError::storage)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(SqliteError::storage)?;
        let mut requests = Vec::new();
        for id in ids {
            let id = source_store::identifier(id)?;
            let request = self
                .load_delete_request(namespace, &id)?
                .ok_or_else(invalid)?;
            // A status cell alone cannot hide a request. Validate canonical evidence
            // and its persisted component results before excluding completed work.
            let completed =
                self.latest_deletion_evidence(namespace, &id)?
                    .is_some_and(|evidence| {
                        evidence.params().overall_status
                            == radishmemory_core::DeletionOverallStatus::Completed
                    });
            if !completed {
                requests.push(request);
            }
        }
        Ok(requests)
    }
    pub fn latest_deletion_evidence(
        &self,
        namespace: &Identifier,
        request: &Identifier,
    ) -> Result<Option<radishmemory_core::DeletionEvidence>> {
        self.require_namespace(namespace)?;
        let tip: Option<String> = self.database.connection.query_row(
            "SELECT deletion_evidence_id FROM radishmemory_deletion_evidence WHERE namespace_id=?1 AND delete_request_id=?2 ORDER BY execution_ordinal DESC LIMIT 1",
            params![namespace.as_str(), request.as_str()], |row| row.get(0),
        ).optional().map_err(SqliteError::storage)?;
        tip.map(|tip| self.load_deletion_evidence(namespace, &source_store::identifier(tip)?))
            .transpose()
            .map(Option::flatten)
    }
    pub fn load_source_artifact(
        &self,
        namespace: &Identifier,
        source: &Identifier,
    ) -> Result<Option<SourceArtifact>> {
        self.require_namespace(namespace)?;
        source_loader(self.sources)?(namespace, source)
    }
    pub fn load_source_fragments(
        &self,
        namespace: &Identifier,
        source: &Identifier,
    ) -> Result<Option<Vec<SourceFragment>>> {
        self.load_source_artifact(namespace, source)?
            .map(|source| {
                source_store::load_fragments_for_source(&self.database.connection, &source)
            })
            .transpose()
    }
}
impl SourceCatalog for ObjectReadView<'_> {
    type Error = SqliteError;
    fn resolve_source_lineage(
        &self,
        namespace: &Identifier,
        lineage: &Identifier,
    ) -> Result<Option<SourceLineageState>> {
        self.require_namespace(namespace)?;
        let load = source_loader(self.sources)?;
        crate::source_catalog::resolve_source_lineage_with(
            &self.database.connection,
            namespace,
            lineage,
            &|_, ns, id| load(ns, id),
        )
    }
    fn list_source_lineages(
        &self,
        request: &SourceCatalogRequest,
    ) -> Result<Vec<SourceLineageSummary>> {
        self.require_namespace(request.namespace_id())?;
        let load = source_loader(self.sources)?;
        crate::source_catalog::list_source_lineages_with(
            &self.database.connection,
            request,
            &|_, ns, id| load(ns, id),
        )
    }
    fn list_source_versions(
        &self,
        namespace: &Identifier,
        lineage: &Identifier,
    ) -> Result<Vec<SourceVersionSummary>> {
        self.require_namespace(namespace)?;
        let load = source_loader(self.sources)?;
        crate::source_catalog::list_source_versions_with(
            &self.database.connection,
            namespace,
            lineage,
            &|_, ns, id| load(ns, id),
        )
    }
}
impl LocalSearch for ObjectReadView<'_> {
    type Error = SqliteError;
    fn search(&self, request: &LocalSearchRequest) -> Result<Vec<LocalSearchHit>> {
        self.require_namespace(request.namespace_id())?;
        let load = source_loader(self.sources)?;
        crate::derived_index::search_with_sources(
            &self.database.connection,
            request,
            &|_, ns, id| load(ns, id),
        )
    }
}
impl fmt::Debug for ObjectReadView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ObjectReadView([REDACTED])")
    }
}
