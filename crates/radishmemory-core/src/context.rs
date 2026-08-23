use std::collections::BTreeSet;

use crate::{
    CanonicalObject, CanonicalObjectType, CoreError, Digest, DigestProfile, EvidenceRef,
    EvidenceType, Governance, GovernedCanonicalObject, Identifier, InvalidCanonicalObjectReason,
    NonEmptyText, ObjectRef, Timestamp, compute_nfc_text_digest, require_non_empty,
    require_profile, require_unique, require_unique_by,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetUnit {
    Utf8Bytes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Budget {
    limit: u64,
    used: u64,
}

impl Budget {
    pub fn new(limit: u64, used: u64) -> Result<Self, CoreError> {
        if used > limit {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::BudgetExceeded,
            ));
        }
        Ok(Self { limit, used })
    }

    #[must_use]
    pub const fn unit(&self) -> BudgetUnit {
        BudgetUnit::Utf8Bytes
    }

    #[must_use]
    pub const fn limit(&self) -> u64 {
        self.limit
    }

    #[must_use]
    pub const fn used(&self) -> u64 {
        self.used
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TruncationFacts {
    was_truncated: bool,
    original_utf8_bytes: u64,
    rendered_utf8_bytes: u64,
    reason_code: Option<NonEmptyText>,
}

impl TruncationFacts {
    pub fn new(
        was_truncated: bool,
        original_utf8_bytes: u64,
        rendered_utf8_bytes: u64,
        reason_code: Option<NonEmptyText>,
    ) -> Result<Self, CoreError> {
        let valid = if was_truncated {
            rendered_utf8_bytes < original_utf8_bytes && reason_code.is_some()
        } else {
            rendered_utf8_bytes == original_utf8_bytes && reason_code.is_none()
        };
        if !valid {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        Ok(Self {
            was_truncated,
            original_utf8_bytes,
            rendered_utf8_bytes,
            reason_code,
        })
    }

    #[must_use]
    pub const fn was_truncated(&self) -> bool {
        self.was_truncated
    }

    #[must_use]
    pub const fn original_utf8_bytes(&self) -> u64 {
        self.original_utf8_bytes
    }

    #[must_use]
    pub const fn rendered_utf8_bytes(&self) -> u64 {
        self.rendered_utf8_bytes
    }

    #[must_use]
    pub fn reason_code(&self) -> Option<&NonEmptyText> {
        self.reason_code.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextItemType {
    SourceFragment,
    MemoryRecord,
    ConflictNotice,
    Constraint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalRole {
    Current,
    Historical,
    Conflict,
    NotApplicable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextItemParams {
    pub item_id: Identifier,
    pub ordinal: u64,
    pub item_type: ContextItemType,
    pub object_refs: Vec<ObjectRef>,
    pub rendered_content: NonEmptyText,
    pub content_digest: Digest,
    pub evidence_refs: Vec<EvidenceRef>,
    pub citation_ids: Vec<Identifier>,
    pub selection_reason_codes: Vec<NonEmptyText>,
    pub temporal_role: TemporalRole,
    pub truncation: TruncationFacts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextItem(ContextItemParams);

impl ContextItem {
    pub fn new(params: ContextItemParams) -> Result<Self, CoreError> {
        require_unique(&params.object_refs)?;
        require_non_empty(&params.evidence_refs)?;
        require_unique(&params.evidence_refs)?;
        require_unique(&params.citation_ids)?;
        require_non_empty(&params.selection_reason_codes)?;
        require_unique(&params.selection_reason_codes)?;

        let references_are_valid = if params.item_type == ContextItemType::Constraint {
            !params.object_refs.is_empty()
                || params
                    .evidence_refs
                    .iter()
                    .any(|reference| reference.evidence_type() == EvidenceType::PolicyBasis)
        } else {
            !params.object_refs.is_empty()
        };
        if !references_are_valid {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }

        require_profile(&params.content_digest, DigestProfile::Utf8NfcTextV1)?;
        if compute_nfc_text_digest(params.rendered_content.as_str()) != params.content_digest {
            return Err(CoreError::digest_mismatch());
        }
        if usize::try_from(params.truncation.rendered_utf8_bytes())
            != Ok(params.rendered_content.utf8_len())
        {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::ContentLengthMismatch,
            ));
        }
        Ok(Self(params))
    }

    #[must_use]
    pub const fn params(&self) -> &ContextItemParams {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Citation {
    citation_id: Identifier,
    source_id: Identifier,
    fragment_id: Identifier,
    byte_start: u64,
    byte_end: u64,
    fragment_digest: Digest,
}

impl Citation {
    pub fn new(
        citation_id: Identifier,
        source_id: Identifier,
        fragment_id: Identifier,
        byte_start: u64,
        byte_end: u64,
        fragment_digest: Digest,
    ) -> Result<Self, CoreError> {
        if byte_start >= byte_end {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidByteRange,
            ));
        }
        require_profile(&fragment_digest, DigestProfile::ExactBytesV1)?;
        Ok(Self {
            citation_id,
            source_id,
            fragment_id,
            byte_start,
            byte_end,
            fragment_digest,
        })
    }

    #[must_use]
    pub const fn citation_id(&self) -> &Identifier {
        &self.citation_id
    }

    #[must_use]
    pub const fn source_id(&self) -> &Identifier {
        &self.source_id
    }

    #[must_use]
    pub const fn fragment_id(&self) -> &Identifier {
        &self.fragment_id
    }

    #[must_use]
    pub const fn byte_start(&self) -> u64 {
        self.byte_start
    }

    #[must_use]
    pub const fn byte_end(&self) -> u64 {
        self.byte_end
    }

    #[must_use]
    pub const fn fragment_digest(&self) -> &Digest {
        &self.fragment_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilterCount {
    reason_code: NonEmptyText,
    allowed_count: u64,
    rejected_count: u64,
    truncated_count: u64,
}

impl FilterCount {
    pub fn new(
        reason_code: NonEmptyText,
        allowed_count: u64,
        rejected_count: u64,
        truncated_count: u64,
    ) -> Result<Self, CoreError> {
        if allowed_count == 0 && rejected_count == 0 && truncated_count == 0 {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        Ok(Self {
            reason_code,
            allowed_count,
            rejected_count,
            truncated_count,
        })
    }

    #[must_use]
    pub const fn reason_code(&self) -> &NonEmptyText {
        &self.reason_code
    }

    #[must_use]
    pub const fn allowed_count(&self) -> u64 {
        self.allowed_count
    }

    #[must_use]
    pub const fn rejected_count(&self) -> u64 {
        self.rejected_count
    }

    #[must_use]
    pub const fn truncated_count(&self) -> u64 {
        self.truncated_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryScope {
    Local,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextPackParams {
    pub context_pack_id: Identifier,
    pub namespace_id: Identifier,
    pub request_id: Identifier,
    pub task: NonEmptyText,
    pub task_digest: Digest,
    pub as_of: Timestamp,
    pub compiled_at: Timestamp,
    pub governance: Governance,
    pub budget: Budget,
    pub items: Vec<ContextItem>,
    pub citation_map: Vec<Citation>,
    pub filter_summary: Vec<FilterCount>,
    pub content_digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextPack(ContextPackParams);

impl ContextPack {
    pub fn new(params: ContextPackParams) -> Result<Self, CoreError> {
        require_profile(&params.task_digest, DigestProfile::Utf8NfcTextV1)?;
        if compute_nfc_text_digest(params.task.as_str()) != params.task_digest {
            return Err(CoreError::digest_mismatch());
        }
        require_profile(&params.content_digest, DigestProfile::ContextPackV1)?;
        require_unique_by(&params.items, |item| item.params().item_id.clone())?;
        require_unique_by(&params.citation_map, |citation| {
            citation.citation_id().clone()
        })?;
        require_unique_by(&params.filter_summary, |count| count.reason_code().clone())?;

        for (ordinal, item) in params.items.iter().enumerate() {
            if usize::try_from(item.params().ordinal) != Ok(ordinal) {
                return Err(CoreError::invalid_canonical_object(
                    InvalidCanonicalObjectReason::DuplicateOrdinal,
                ));
            }
        }

        let citation_ids = params
            .citation_map
            .iter()
            .map(Citation::citation_id)
            .collect::<BTreeSet<_>>();
        if params.items.iter().any(|item| {
            item.params()
                .citation_ids
                .iter()
                .any(|citation_id| !citation_ids.contains(citation_id))
        }) {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        Ok(Self(params))
    }

    #[must_use]
    pub const fn delivery_scope(&self) -> DeliveryScope {
        DeliveryScope::Local
    }

    #[must_use]
    pub const fn params(&self) -> &ContextPackParams {
        &self.0
    }
}

impl CanonicalObject for ContextPack {
    fn object_type(&self) -> CanonicalObjectType {
        CanonicalObjectType::ContextPack
    }

    fn object_id(&self) -> &Identifier {
        &self.0.context_pack_id
    }

    fn namespace_id(&self) -> &Identifier {
        &self.0.namespace_id
    }
}

impl GovernedCanonicalObject for ContextPack {
    fn governance(&self) -> &Governance {
        &self.0.governance
    }
}
