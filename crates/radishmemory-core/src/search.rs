use crate::{
    CanonicalObjectType, CoreError, Identifier, InvalidCanonicalObjectReason, MediaType,
    MemoryRecord, NonEmptyText, ProducerType, Sensitivity, SourceArtifact, SourceFragment,
    SourceOriginKind, Timestamp,
};

/// Optional restrictions for a source-only search over current artifacts.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SourceSearchScope {
    pub user_input_producer: Option<(ProducerType, Identifier)>,
    pub media_type: Option<MediaType>,
    pub saved_within_seconds: Option<u64>,
}

/// Validated boundary for one local M0 full-text search.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSearchRequest {
    namespace_id: Identifier,
    query: NonEmptyText,
    as_of: Timestamp,
    top_k: usize,
    allowed_sensitivities: Vec<Sensitivity>,
    source_scope: Option<SourceSearchScope>,
}

impl LocalSearchRequest {
    pub fn new(
        namespace_id: Identifier,
        query: NonEmptyText,
        as_of: Timestamp,
        top_k: usize,
        allowed_sensitivities: impl IntoIterator<Item = Sensitivity>,
    ) -> Result<Self, CoreError> {
        if query.as_str().split_whitespace().next().is_none() {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::EmptyText,
            ));
        }
        if top_k == 0 {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::EmptyRequiredCollection,
            ));
        }
        let allowed_sensitivities = allowed_sensitivities.into_iter().collect::<Vec<_>>();
        if allowed_sensitivities.is_empty() {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::EmptyRequiredCollection,
            ));
        }
        Ok(Self {
            namespace_id,
            query,
            as_of,
            top_k,
            allowed_sensitivities,
            source_scope: None,
        })
    }

    #[must_use]
    pub fn for_sources(mut self, scope: SourceSearchScope) -> Self {
        self.source_scope = Some(scope);
        self
    }

    #[must_use]
    pub fn includes_source(&self, source: &SourceArtifact) -> bool {
        let Some(scope) = &self.source_scope else {
            return true;
        };
        let source = source.params();
        scope.user_input_producer.as_ref().is_none_or(|(kind, id)| {
            source.origin_kind == SourceOriginKind::ExplicitUserInput
                && source.producer.producer_type() == *kind
                && source.producer.producer_id() == id
        }) && scope
            .media_type
            .is_none_or(|media| source.media_type == media)
            && scope.saved_within_seconds.is_none_or(|seconds| {
                source
                    .captured_at
                    .at_or_after_seconds_before(&self.as_of, seconds)
            })
    }

    #[must_use]
    pub const fn includes_memories(&self) -> bool {
        self.source_scope.is_none()
    }

    #[must_use]
    pub const fn namespace_id(&self) -> &Identifier {
        &self.namespace_id
    }

    #[must_use]
    pub const fn query(&self) -> &NonEmptyText {
        &self.query
    }

    #[must_use]
    pub const fn as_of(&self) -> &Timestamp {
        &self.as_of
    }

    #[must_use]
    pub const fn top_k(&self) -> usize {
        self.top_k
    }

    #[must_use]
    pub fn allows_sensitivity(&self, sensitivity: Sensitivity) -> bool {
        self.allowed_sensitivities.contains(&sensitivity)
    }
}

/// One verified local recall result without exposing adapter ranks or row IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalSearchHit {
    SourceFragment(Box<SourceFragment>),
    MemoryRecord(Box<MemoryRecord>),
}

impl LocalSearchHit {
    #[must_use]
    pub const fn object_type(&self) -> CanonicalObjectType {
        match self {
            Self::SourceFragment(_) => CanonicalObjectType::SourceFragment,
            Self::MemoryRecord(_) => CanonicalObjectType::MemoryRecord,
        }
    }

    #[must_use]
    pub const fn object_id(&self) -> &Identifier {
        match self {
            Self::SourceFragment(fragment) => &fragment.params().fragment_id,
            Self::MemoryRecord(record) => &record.params().memory_id,
        }
    }
}
