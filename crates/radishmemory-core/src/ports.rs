use std::error::Error;

use crate::{
    Identifier, MemoryDecision, MemoryProposal, MemoryRecord, MemoryStateEvent, SourceArtifact,
    SourceFragment,
};

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
