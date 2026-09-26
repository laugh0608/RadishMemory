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
