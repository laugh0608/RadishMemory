use crate::{
    CanonicalObject, CanonicalObjectType, CoreError, Digest, DigestProfile, Governance, Identifier,
    InvalidCanonicalObjectReason, NonEmptyText, ProducerRef, Timestamp, Version,
    compute_exact_bytes_digest, require_profile, require_unique,
};

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
