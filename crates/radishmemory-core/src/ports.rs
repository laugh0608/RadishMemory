use std::error::Error;

use crate::{
    ComponentResult, DeleteRequest, DeletionEvidence, Identifier, LocalDeletionExecution,
    LocalSearchHit, LocalSearchRequest, MemoryDecision, MemoryProposal, MemoryRecord,
    MemoryStateEvent, ObjectRef, SourceArtifact, SourceCapture, SourceCaptureResult,
    SourceCatalogRequest, SourceFragment, SourceLineageState, SourceLineageSummary,
    SourceVersionSummary,
};

/// Minimal local retrieval boundary required by the frozen M0 search operation.
pub trait LocalSearch {
    type Error: Error + Send + Sync + 'static;

    /// Fails closed on derived-data drift, then returns deterministic local top-k hits.
    fn search(&self, request: &LocalSearchRequest) -> Result<Vec<LocalSearchHit>, Self::Error>;
}

/// Atomic application boundary for a complete, user-authorized source capture.
pub trait SourceCaptureStore {
    type Error: Error + Send + Sync + 'static;

    /// Commits source facts, fragments, recall data, binding, tip, and audit as one result.
    fn capture_source(
        &mut self,
        capture: &SourceCapture,
    ) -> Result<SourceCaptureResult, Self::Error>;
}

/// Read-only application boundary for current source lineages and explicit history.
pub trait SourceCatalog {
    type Error: Error + Send + Sync + 'static;

    /// Resolves one active explicit lineage to its opaque binding and current tip.
    fn resolve_source_lineage(
        &self,
        namespace_id: &Identifier,
        lineage_id: &Identifier,
    ) -> Result<Option<SourceLineageState>, Self::Error>;

    /// Lists active current lineages in stable captured-time order.
    fn list_source_lineages(
        &self,
        request: &SourceCatalogRequest,
    ) -> Result<Vec<SourceLineageSummary>, Self::Error>;

    /// Lists all active versions for one lineage in ascending version order.
    fn list_source_versions(
        &self,
        namespace_id: &Identifier,
        lineage_id: &Identifier,
    ) -> Result<Vec<SourceVersionSummary>, Self::Error>;
}

/// Minimal persistence boundary required by M0 capture and segmentation.
pub trait SourceVault {
    type Error: Error + Send + Sync + 'static;

    /// Persists one immutable source metadata row and its exact body atomically.
    fn store_source_artifact(&mut self, source: &SourceArtifact) -> Result<(), Self::Error>;

    /// Persists one nonempty, single-source fragment batch atomically.
    fn store_source_fragments(&mut self, fragments: &[SourceFragment]) -> Result<(), Self::Error>;

    /// Loads a source only when both namespace and immutable source ID match.
    fn load_source_artifact(
        &self,
        namespace_id: &Identifier,
        source_id: &Identifier,
    ) -> Result<Option<SourceArtifact>, Self::Error>;

    /// Returns `None` when the source is absent and `Some` for its ordered fragments.
    fn load_source_fragments(
        &self,
        namespace_id: &Identifier,
        source_id: &Identifier,
    ) -> Result<Option<Vec<SourceFragment>>, Self::Error>;
}

/// Minimal persistence boundary required by M0 proposal, decision, and state operations.
pub trait MemoryStore {
    type Error: Error + Send + Sync + 'static;

    /// Persists one immutable proposal after resolving its complete source closure.
    fn store_memory_proposal(&mut self, proposal: &MemoryProposal) -> Result<(), Self::Error>;

    /// Appends one decision to the proposal's unbranched decision chain.
    fn store_memory_decision(&mut self, decision: &MemoryDecision) -> Result<(), Self::Error>;

    /// Atomically materializes an accepted record and all state events caused by it.
    fn materialize_accepted_memory(
        &mut self,
        record: &MemoryRecord,
        initial_event: &MemoryStateEvent,
        superseded_events: &[MemoryStateEvent],
    ) -> Result<(), Self::Error>;

    /// Appends one non-initial event to an existing unbranched memory event chain.
    fn append_memory_state_event(&mut self, event: &MemoryStateEvent) -> Result<(), Self::Error>;

    /// Loads an immutable proposal only when namespace and proposal ID match.
    fn load_memory_proposal(
        &self,
        namespace_id: &Identifier,
        proposal_id: &Identifier,
    ) -> Result<Option<MemoryProposal>, Self::Error>;

    /// Loads one immutable decision and validates its complete proposal chain.
    fn load_memory_decision(
        &self,
        namespace_id: &Identifier,
        decision_id: &Identifier,
    ) -> Result<Option<MemoryDecision>, Self::Error>;

    /// Rebuilds a memory record's current state from its validated event chain.
    fn load_memory_record(
        &self,
        namespace_id: &Identifier,
        memory_id: &Identifier,
    ) -> Result<Option<MemoryRecord>, Self::Error>;

    /// Returns `None` when the memory is absent and `Some` for its ordered event chain.
    fn load_memory_state_events(
        &self,
        namespace_id: &Identifier,
        memory_id: &Identifier,
    ) -> Result<Option<Vec<MemoryStateEvent>>, Self::Error>;
}

/// Local persistence and execution boundary for the frozen M0 deletion workflow.
pub trait DeletionStore {
    type Error: Error + Send + Sync + 'static;

    /// Resolves one active source lineage and every active memory dependency to exact targets.
    fn resolve_source_lineage_deletion_targets(
        &self,
        namespace_id: &Identifier,
        lineage_id: &Identifier,
    ) -> Result<Vec<ObjectRef>, Self::Error>;

    /// Persists an exact deletion plan and atomically closes all planned targets to recall.
    fn store_delete_request(&mut self, request: &DeleteRequest) -> Result<(), Self::Error>;

    /// Executes every planned component and persists the complete immutable attempt result.
    fn execute_deletion(
        &mut self,
        namespace_id: &Identifier,
        delete_request_id: &Identifier,
        execution: &LocalDeletionExecution,
    ) -> Result<Vec<ComponentResult>, Self::Error>;

    /// Appends evidence bound to one already-persisted execution attempt.
    fn store_deletion_evidence(&mut self, evidence: &DeletionEvidence) -> Result<(), Self::Error>;

    /// Loads an immutable deletion request only when namespace and ID match.
    fn load_delete_request(
        &self,
        namespace_id: &Identifier,
        delete_request_id: &Identifier,
    ) -> Result<Option<DeleteRequest>, Self::Error>;

    /// Loads one immutable evidence snapshot and its exact component results.
    fn load_deletion_evidence(
        &self,
        namespace_id: &Identifier,
        deletion_evidence_id: &Identifier,
    ) -> Result<Option<DeletionEvidence>, Self::Error>;
}
