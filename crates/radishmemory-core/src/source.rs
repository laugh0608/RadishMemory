use crate::{
    CanonicalObject, CanonicalObjectType, CoreError, Digest, DigestProfile, Governance,
    GovernedCanonicalObject, Identifier, InvalidCanonicalObjectReason, NonEmptyText, ProducerRef,
    Timestamp, Version, compute_exact_bytes_digest, require_profile, require_unique,
};

pub const SOURCE_ORIGIN_BINDING_PREFIX: &str = "origin-binding-";

/// One complete, already-validated source capture submitted to durable storage.
#[derive(Clone)]
pub struct SourceCapture {
    origin_binding_id: Identifier,
    source: SourceArtifact,
    fragments: Vec<SourceFragment>,
}

impl SourceCapture {
    pub fn new(
        origin_binding_id: Identifier,
        source: SourceArtifact,
        fragments: Vec<SourceFragment>,
    ) -> Result<Self, CoreError> {
        let source_params = source.params();
        if source_params.origin_kind != SourceOriginKind::ExplicitUserInput
            || !source_origin_binding_id_is_valid(origin_binding_id.as_str())
            || source_params.origin_ref.as_ref().map(NonEmptyText::as_str)
                != Some(origin_binding_id.as_str())
        {
            return Err(CoreError::cross_object_invariant(
                crate::CrossObjectInvariantReason::OriginBindingMismatch,
            ));
        }
        crate::validate_complete_source_fragment_set(&source, &fragments)?;
        Ok(Self {
            origin_binding_id,
            source,
            fragments,
        })
    }

    #[must_use]
    pub const fn origin_binding_id(&self) -> &Identifier {
        &self.origin_binding_id
    }

    #[must_use]
    pub const fn source(&self) -> &SourceArtifact {
        &self.source
    }

    #[must_use]
    pub fn fragments(&self) -> &[SourceFragment] {
        &self.fragments
    }
}

#[must_use]
pub fn source_origin_binding_id_is_valid(value: &str) -> bool {
    let Some(suffix) = value.strip_prefix(SOURCE_ORIGIN_BINDING_PREFIX) else {
        return false;
    };
    !suffix.is_empty()
        && value.len() <= 128
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

impl std::fmt::Debug for SourceCapture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceCapture")
            .field("namespace_id", &self.source.params().namespace_id)
            .field("source_id", &self.source.params().source_id)
            .field("lineage_id", &self.source.params().lineage_id)
            .field("version", &self.source.params().version)
            .field("content_length", &self.source.params().content_length)
            .field("fragment_count", &self.fragments.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceCaptureOutcome {
    Created,
    Idempotent,
    Versioned,
}

/// Path-free facts returned only after an atomic source capture succeeds.
#[derive(Clone, Eq, PartialEq)]
pub struct SourceCaptureResult {
    namespace_id: Identifier,
    source_id: Identifier,
    lineage_id: Identifier,
    version: Version,
    content_digest: Digest,
    content_length: u64,
    media_type: MediaType,
    outcome: SourceCaptureOutcome,
}

impl std::fmt::Debug for SourceCaptureResult {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceCaptureResult")
            .field("namespace_id", &self.namespace_id)
            .field("source_id", &self.source_id)
            .field("lineage_id", &self.lineage_id)
            .field("version", &self.version)
            .field("content_length", &self.content_length)
            .field("media_type", &self.media_type)
            .field("outcome", &self.outcome)
            .finish_non_exhaustive()
    }
}

impl SourceCaptureResult {
    #[must_use]
    pub fn from_source(source: &SourceArtifact, outcome: SourceCaptureOutcome) -> Self {
        let params = source.params();
        Self {
            namespace_id: params.namespace_id.clone(),
            source_id: params.source_id.clone(),
            lineage_id: params.lineage_id.clone(),
            version: params.version,
            content_digest: params.content_digest.clone(),
            content_length: params.content_length,
            media_type: params.media_type,
            outcome,
        }
    }

    #[must_use]
    pub const fn namespace_id(&self) -> &Identifier {
        &self.namespace_id
    }

    #[must_use]
    pub const fn source_id(&self) -> &Identifier {
        &self.source_id
    }

    #[must_use]
    pub const fn lineage_id(&self) -> &Identifier {
        &self.lineage_id
    }

    #[must_use]
    pub const fn version(&self) -> Version {
        self.version
    }

    #[must_use]
    pub const fn content_digest(&self) -> &Digest {
        &self.content_digest
    }

    #[must_use]
    pub const fn content_length(&self) -> u64 {
        self.content_length
    }

    #[must_use]
    pub const fn media_type(&self) -> MediaType {
        self.media_type
    }

    #[must_use]
    pub const fn outcome(&self) -> SourceCaptureOutcome {
        self.outcome
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceKind {
    Text,
    Markdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaType {
    TextPlain,
    TextMarkdown,
}

impl MediaType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TextPlain => "text/plain",
            Self::TextMarkdown => "text/markdown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceOriginKind {
    SyntheticFixture,
    ExplicitUserInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceArtifactParams {
    pub source_id: Identifier,
    pub lineage_id: Identifier,
    pub version: Version,
    pub namespace_id: Identifier,
    pub source_kind: SourceKind,
    pub media_type: MediaType,
    pub content: NonEmptyText,
    pub content_length: u64,
    pub content_digest: Digest,
    pub title: Option<NonEmptyText>,
    pub origin_kind: SourceOriginKind,
    pub origin_ref: Option<NonEmptyText>,
    pub observed_at: Timestamp,
    pub captured_at: Timestamp,
    pub supersedes_source_ids: Vec<Identifier>,
    pub governance: Governance,
    pub producer: ProducerRef,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceArtifact(SourceArtifactParams);

impl SourceArtifact {
    pub fn new(params: SourceArtifactParams) -> Result<Self, CoreError> {
        let expected_media_type = match params.source_kind {
            SourceKind::Text => MediaType::TextPlain,
            SourceKind::Markdown => MediaType::TextMarkdown,
        };
        if params.media_type != expected_media_type {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        if usize::try_from(params.content_length) != Ok(params.content.utf8_len()) {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::ContentLengthMismatch,
            ));
        }
        require_profile(&params.content_digest, DigestProfile::ExactBytesV1)?;
        if compute_exact_bytes_digest(params.content.as_str().as_bytes()) != params.content_digest {
            return Err(CoreError::digest_mismatch());
        }
        require_unique(&params.supersedes_source_ids)?;
        let first_version = params.version.get() == 1;
        if first_version != params.supersedes_source_ids.is_empty() {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        Ok(Self(params))
    }

    #[must_use]
    pub const fn params(&self) -> &SourceArtifactParams {
        &self.0
    }
}

impl CanonicalObject for SourceArtifact {
    fn object_type(&self) -> CanonicalObjectType {
        CanonicalObjectType::SourceArtifact
    }

    fn object_id(&self) -> &Identifier {
        &self.0.source_id
    }

    fn namespace_id(&self) -> &Identifier {
        &self.0.namespace_id
    }
}

impl GovernedCanonicalObject for SourceArtifact {
    fn governance(&self) -> &Governance {
        &self.0.governance
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFragmentParams {
    pub fragment_id: Identifier,
    pub namespace_id: Identifier,
    pub source_id: Identifier,
    pub ordinal: u64,
    pub byte_start: u64,
    pub byte_end: u64,
    pub heading_path: Option<Vec<NonEmptyText>>,
    pub content: NonEmptyText,
    pub content_digest: Digest,
    pub segmenter: ProducerRef,
    pub governance: Governance,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFragment(SourceFragmentParams);

impl SourceFragment {
    pub fn new(params: SourceFragmentParams) -> Result<Self, CoreError> {
        if params.byte_start >= params.byte_end {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidByteRange,
            ));
        }
        let range_length = params.byte_end - params.byte_start;
        if usize::try_from(range_length) != Ok(params.content.utf8_len()) {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::ContentLengthMismatch,
            ));
        }
        if params.heading_path.as_ref().is_some_and(Vec::is_empty) {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::EmptyRequiredCollection,
            ));
        }
        require_profile(&params.content_digest, DigestProfile::ExactBytesV1)?;
        if compute_exact_bytes_digest(params.content.as_str().as_bytes()) != params.content_digest {
            return Err(CoreError::digest_mismatch());
        }
        Ok(Self(params))
    }

    #[must_use]
    pub const fn params(&self) -> &SourceFragmentParams {
        &self.0
    }
}

impl CanonicalObject for SourceFragment {
    fn object_type(&self) -> CanonicalObjectType {
        CanonicalObjectType::SourceFragment
    }

    fn object_id(&self) -> &Identifier {
        &self.0.fragment_id
    }

    fn namespace_id(&self) -> &Identifier {
        &self.0.namespace_id
    }
}

impl GovernedCanonicalObject for SourceFragment {
    fn governance(&self) -> &Governance {
        &self.0.governance
    }
}
