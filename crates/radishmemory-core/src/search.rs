use crate::{
    CanonicalObjectType, CoreError, Identifier, InvalidCanonicalObjectReason, MemoryRecord,
    NonEmptyText, Sensitivity, SourceFragment, Timestamp,
};

/// Validated boundary for one local M0 full-text search.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSearchRequest {
    namespace_id: Identifier,
    query: NonEmptyText,
    as_of: Timestamp,
    top_k: usize,
    allowed_sensitivities: Vec<Sensitivity>,
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
        })
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
