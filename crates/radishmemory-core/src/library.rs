use std::fmt;

use crate::{
    CoreError, DeletionState, DigestProfile, Governance, Identifier, InvalidCanonicalObjectReason,
    MediaType, NonEmptyText, SourceArtifact, SourceKind, SourceOriginKind, Timestamp, Version,
};

/// Stable page request for the local source lineage catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCatalogRequest {
    namespace_id: Identifier,
    offset: u64,
    limit: usize,
}

impl SourceCatalogRequest {
    pub fn new(namespace_id: Identifier, offset: u64, limit: usize) -> Result<Self, CoreError> {
        if limit == 0 {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::EmptyRequiredCollection,
            ));
        }
        Ok(Self {
            namespace_id,
            offset,
            limit,
        })
    }

    #[must_use]
    pub const fn namespace_id(&self) -> &Identifier {
        &self.namespace_id
    }

    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }
}

/// Adapter-verified state required to update one explicit source lineage.
#[derive(Clone, Eq, PartialEq)]
pub struct SourceLineageState {
    namespace_id: Identifier,
    origin_binding_id: Identifier,
    lineage_id: Identifier,
    current_source_id: Identifier,
    current_version: Version,
}

impl SourceLineageState {
    pub fn new(
        namespace_id: Identifier,
        origin_binding_id: Identifier,
        lineage_id: Identifier,
        current_source_id: Identifier,
        current_version: Version,
    ) -> Self {
        Self {
            namespace_id,
            origin_binding_id,
            lineage_id,
            current_source_id,
            current_version,
        }
    }

    #[must_use]
    pub const fn namespace_id(&self) -> &Identifier {
        &self.namespace_id
    }

    #[must_use]
    pub const fn origin_binding_id(&self) -> &Identifier {
        &self.origin_binding_id
    }

    #[must_use]
    pub const fn lineage_id(&self) -> &Identifier {
        &self.lineage_id
    }

    #[must_use]
    pub const fn current_source_id(&self) -> &Identifier {
        &self.current_source_id
    }

    #[must_use]
    pub const fn current_version(&self) -> Version {
        self.current_version
    }
}

impl fmt::Debug for SourceLineageState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceLineageState")
            .field("namespace_id", &self.namespace_id)
            .field("lineage_id", &self.lineage_id)
            .field("current_source_id", &self.current_source_id)
            .field("current_version", &self.current_version)
            .finish_non_exhaustive()
    }
}

/// Body-free current lineage row intended for the application catalog.
#[derive(Clone, Eq, PartialEq)]
pub struct SourceLineageSummary {
    namespace_id: Identifier,
    lineage_id: Identifier,
    current_source_id: Identifier,
    current_version: Version,
    title: Option<NonEmptyText>,
    source_kind: SourceKind,
    media_type: MediaType,
    content_length: u64,
    digest_profile: DigestProfile,
    observed_at: Timestamp,
    captured_at: Timestamp,
    governance: Governance,
    version_count: u64,
}

impl SourceLineageSummary {
    pub fn from_current_source(
        source: &SourceArtifact,
        version_count: u64,
    ) -> Result<Self, CoreError> {
        let params = source.params();
        if params.origin_kind != SourceOriginKind::ExplicitUserInput
            || params.governance.deletion_state() != DeletionState::Active
            || version_count != params.version.get()
        {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        Ok(Self {
            namespace_id: params.namespace_id.clone(),
            lineage_id: params.lineage_id.clone(),
            current_source_id: params.source_id.clone(),
            current_version: params.version,
            title: params.title.clone(),
            source_kind: params.source_kind,
            media_type: params.media_type,
            content_length: params.content_length,
            digest_profile: params.content_digest.profile(),
            observed_at: params.observed_at.clone(),
            captured_at: params.captured_at.clone(),
            governance: params.governance.clone(),
            version_count,
        })
    }

    #[must_use]
    pub const fn namespace_id(&self) -> &Identifier {
        &self.namespace_id
    }

    #[must_use]
    pub const fn lineage_id(&self) -> &Identifier {
        &self.lineage_id
    }

    #[must_use]
    pub const fn current_source_id(&self) -> &Identifier {
        &self.current_source_id
    }

    #[must_use]
    pub const fn current_version(&self) -> Version {
        self.current_version
    }

    #[must_use]
    pub fn title(&self) -> Option<&NonEmptyText> {
        self.title.as_ref()
    }

    #[must_use]
    pub const fn source_kind(&self) -> SourceKind {
        self.source_kind
    }

    #[must_use]
    pub const fn media_type(&self) -> MediaType {
        self.media_type
    }

    #[must_use]
    pub const fn content_length(&self) -> u64 {
        self.content_length
    }

    #[must_use]
    pub const fn digest_profile(&self) -> DigestProfile {
        self.digest_profile
    }

    #[must_use]
    pub const fn observed_at(&self) -> &Timestamp {
        &self.observed_at
    }

    #[must_use]
    pub const fn captured_at(&self) -> &Timestamp {
        &self.captured_at
    }

    #[must_use]
    pub const fn governance(&self) -> &Governance {
        &self.governance
    }

    #[must_use]
    pub const fn version_count(&self) -> u64 {
        self.version_count
    }
}

impl fmt::Debug for SourceLineageSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceLineageSummary")
            .field("namespace_id", &self.namespace_id)
            .field("lineage_id", &self.lineage_id)
            .field("current_source_id", &self.current_source_id)
            .field("current_version", &self.current_version)
            .field("has_title", &self.title.is_some())
            .field("source_kind", &self.source_kind)
            .field("media_type", &self.media_type)
            .field("content_length", &self.content_length)
            .field("digest_profile", &self.digest_profile)
            .field("observed_at", &self.observed_at)
            .field("captured_at", &self.captured_at)
            .field("governance", &self.governance)
            .field("version_count", &self.version_count)
            .finish()
    }
}

/// Body-free version row for one explicit source lineage.
#[derive(Clone, Eq, PartialEq)]
pub struct SourceVersionSummary {
    namespace_id: Identifier,
    lineage_id: Identifier,
    source_id: Identifier,
    version: Version,
    title: Option<NonEmptyText>,
    source_kind: SourceKind,
    media_type: MediaType,
    content_length: u64,
    digest_profile: DigestProfile,
    observed_at: Timestamp,
    captured_at: Timestamp,
    governance: Governance,
    current: bool,
}

impl SourceVersionSummary {
    pub fn from_source(source: &SourceArtifact, current: bool) -> Result<Self, CoreError> {
        let params = source.params();
        if params.origin_kind != SourceOriginKind::ExplicitUserInput
            || params.governance.deletion_state() != DeletionState::Active
        {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        Ok(Self {
            namespace_id: params.namespace_id.clone(),
            lineage_id: params.lineage_id.clone(),
            source_id: params.source_id.clone(),
            version: params.version,
            title: params.title.clone(),
            source_kind: params.source_kind,
            media_type: params.media_type,
            content_length: params.content_length,
            digest_profile: params.content_digest.profile(),
            observed_at: params.observed_at.clone(),
            captured_at: params.captured_at.clone(),
            governance: params.governance.clone(),
            current,
        })
    }

    #[must_use]
    pub const fn namespace_id(&self) -> &Identifier {
        &self.namespace_id
    }

    #[must_use]
    pub const fn lineage_id(&self) -> &Identifier {
        &self.lineage_id
    }

    #[must_use]
    pub const fn source_id(&self) -> &Identifier {
        &self.source_id
    }

    #[must_use]
    pub const fn version(&self) -> Version {
        self.version
    }

    #[must_use]
    pub fn title(&self) -> Option<&NonEmptyText> {
        self.title.as_ref()
    }

    #[must_use]
    pub const fn source_kind(&self) -> SourceKind {
        self.source_kind
    }

    #[must_use]
    pub const fn media_type(&self) -> MediaType {
        self.media_type
    }

    #[must_use]
    pub const fn content_length(&self) -> u64 {
        self.content_length
    }

    #[must_use]
    pub const fn digest_profile(&self) -> DigestProfile {
        self.digest_profile
    }

    #[must_use]
    pub const fn observed_at(&self) -> &Timestamp {
        &self.observed_at
    }

    #[must_use]
    pub const fn captured_at(&self) -> &Timestamp {
        &self.captured_at
    }

    #[must_use]
    pub const fn governance(&self) -> &Governance {
        &self.governance
    }

    #[must_use]
    pub const fn current(&self) -> bool {
        self.current
    }
}

impl fmt::Debug for SourceVersionSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SourceVersionSummary")
            .field("namespace_id", &self.namespace_id)
            .field("lineage_id", &self.lineage_id)
            .field("source_id", &self.source_id)
            .field("version", &self.version)
            .field("has_title", &self.title.is_some())
            .field("source_kind", &self.source_kind)
            .field("media_type", &self.media_type)
            .field("content_length", &self.content_length)
            .field("digest_profile", &self.digest_profile)
            .field("observed_at", &self.observed_at)
            .field("captured_at", &self.captured_at)
            .field("governance", &self.governance)
            .field("current", &self.current)
            .finish()
    }
}
