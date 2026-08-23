use std::error::Error;

use crate::{Identifier, SourceArtifact, SourceFragment};

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
