use crate::{
    ActorRef, CanonicalObject, CanonicalObjectType, CoreError, Digest, EvidenceRef, EvidenceType,
    Governance, GovernedCanonicalObject, Identifier, InvalidCanonicalObjectReason, MemoryValue,
    NonEmptyText, ProducerRef, Timestamp, UnitInterval, ValidTime, Version, require_non_empty,
    require_profile, require_unique,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalOperation {
    Create,
    Supersede,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryType {
    Observation,
    Claim,
    Episode,
    Preference,
    Procedure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryProposalParams {
    pub proposal_id: Identifier,
    pub namespace_id: Identifier,
    pub operation: ProposalOperation,
    pub memory_type: MemoryType,
    pub subject_ref: Identifier,
    pub proposed_content: MemoryValue,
    pub source_fragment_refs: Vec<Identifier>,
    pub target_memory_ids: Vec<Identifier>,
    pub observed_at: Timestamp,
    pub valid_time: ValidTime,
    pub confidence: UnitInterval,
    pub importance: UnitInterval,
    pub governance: Governance,
    pub producer: ProducerRef,
    pub reason_code: NonEmptyText,
    pub proposed_at: Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryProposal(MemoryProposalParams);

impl MemoryProposal {
    pub fn new(params: MemoryProposalParams) -> Result<Self, CoreError> {
        require_non_empty(&params.source_fragment_refs)?;
        require_unique(&params.source_fragment_refs)?;
        require_unique(&params.target_memory_ids)?;
        let targets_are_valid = match params.operation {
            ProposalOperation::Create => params.target_memory_ids.is_empty(),
            ProposalOperation::Supersede => !params.target_memory_ids.is_empty(),
        };
        if !targets_are_valid {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        Ok(Self(params))
    }

    #[must_use]
    pub const fn params(&self) -> &MemoryProposalParams {
        &self.0
    }
}

impl CanonicalObject for MemoryProposal {
    fn object_type(&self) -> CanonicalObjectType {
        CanonicalObjectType::MemoryProposal
    }

    fn object_id(&self) -> &Identifier {
        &self.0.proposal_id
    }

    fn namespace_id(&self) -> &Identifier {
        &self.0.namespace_id
    }
}

impl GovernedCanonicalObject for MemoryProposal {
    fn governance(&self) -> &Governance {
        &self.0.governance
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Decision {
    Accept,
    Reject,
    Defer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryDecisionParams {
    pub decision_id: Identifier,
    pub namespace_id: Identifier,
    pub proposal_id: Identifier,
    pub previous_decision_id: Option<Identifier>,
    pub decision: Decision,
    pub decided_by: ActorRef,
    pub authorization_basis: NonEmptyText,
    pub reason_code: NonEmptyText,
    pub reason_text: Option<NonEmptyText>,
    pub result_memory_id: Option<Identifier>,
    pub decided_at: Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryDecision(MemoryDecisionParams);

impl MemoryDecision {
    pub fn new(params: MemoryDecisionParams) -> Result<Self, CoreError> {
        if (params.decision == Decision::Accept) != params.result_memory_id.is_some() {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        Ok(Self(params))
    }

    #[must_use]
    pub const fn params(&self) -> &MemoryDecisionParams {
        &self.0
    }
}

impl CanonicalObject for MemoryDecision {
    fn object_type(&self) -> CanonicalObjectType {
        CanonicalObjectType::MemoryDecision
    }

    fn object_id(&self) -> &Identifier {
        &self.0.decision_id
    }

    fn namespace_id(&self) -> &Identifier {
        &self.0.namespace_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryState {
    Confirmed,
    Superseded,
    Contradicted,
    Retracted,
    Expired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRecordParams {
    pub memory_id: Identifier,
    pub lineage_id: Identifier,
    pub version: Version,
    pub namespace_id: Identifier,
    pub memory_type: MemoryType,
    pub subject_ref: Identifier,
    pub content: MemoryValue,
    pub source_fragment_refs: Vec<Identifier>,
    pub origin_proposal_id: Identifier,
    pub accepted_by_decision_id: Identifier,
    pub observed_at: Timestamp,
    pub valid_time: ValidTime,
    pub confidence: UnitInterval,
    pub importance: UnitInterval,
    pub governance: Governance,
    pub current_state: MemoryState,
    pub last_state_event_id: Identifier,
    pub supersedes_memory_ids: Vec<Identifier>,
    pub contradicts_memory_ids: Vec<Identifier>,
    pub content_digest: Digest,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRecord(MemoryRecordParams);

impl MemoryRecord {
    pub fn new(params: MemoryRecordParams) -> Result<Self, CoreError> {
        require_non_empty(&params.source_fragment_refs)?;
        require_unique(&params.source_fragment_refs)?;
        require_unique(&params.supersedes_memory_ids)?;
        require_unique(&params.contradicts_memory_ids)?;
        require_profile(
            &params.content_digest,
            params.content.content_digest().profile(),
        )?;
        if &params.content_digest != params.content.content_digest() {
            return Err(CoreError::digest_mismatch());
        }
        let first_version = params.version.get() == 1;
        if first_version != params.supersedes_memory_ids.is_empty() {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidFieldCombination,
            ));
        }
        Ok(Self(params))
    }

    #[must_use]
    pub const fn initial_state(&self) -> MemoryState {
        MemoryState::Confirmed
    }

    #[must_use]
    pub const fn params(&self) -> &MemoryRecordParams {
        &self.0
    }
}

impl CanonicalObject for MemoryRecord {
    fn object_type(&self) -> CanonicalObjectType {
        CanonicalObjectType::MemoryRecord
    }

    fn object_id(&self) -> &Identifier {
        &self.0.memory_id
    }

    fn namespace_id(&self) -> &Identifier {
        &self.0.namespace_id
    }
}

impl GovernedCanonicalObject for MemoryRecord {
    fn governance(&self) -> &Governance {
        &self.0.governance
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryEventType {
    Confirmed,
    Superseded,
    Contradicted,
    Retracted,
    Expired,
}

impl MemoryEventType {
    #[must_use]
    pub const fn to_state(self) -> MemoryState {
        match self {
            Self::Confirmed => MemoryState::Confirmed,
            Self::Superseded => MemoryState::Superseded,
            Self::Contradicted => MemoryState::Contradicted,
            Self::Retracted => MemoryState::Retracted,
            Self::Expired => MemoryState::Expired,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryStateEventParams {
    pub event_id: Identifier,
    pub namespace_id: Identifier,
    pub memory_id: Identifier,
    pub previous_event_id: Option<Identifier>,
    pub event_type: MemoryEventType,
    pub from_state: Option<MemoryState>,
    pub cause_ref: EvidenceRef,
    pub related_memory_ids: Vec<Identifier>,
    pub actor: ActorRef,
    pub reason_code: NonEmptyText,
    pub effective_at: Option<Timestamp>,
    pub occurred_at: Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryStateEvent(MemoryStateEventParams);

impl MemoryStateEvent {
    pub fn new(params: MemoryStateEventParams) -> Result<Self, CoreError> {
        require_unique(&params.related_memory_ids)?;
        let transition_is_valid = match params.event_type {
            MemoryEventType::Confirmed => {
                params.previous_event_id.is_none()
                    && params.from_state.is_none()
                    && params.effective_at.is_none()
                    && params.cause_ref.evidence_type() == EvidenceType::MemoryDecision
            }
            MemoryEventType::Superseded
            | MemoryEventType::Contradicted
            | MemoryEventType::Retracted
            | MemoryEventType::Expired => {
                params.previous_event_id.is_some()
                    && params.from_state == Some(MemoryState::Confirmed)
                    && params.effective_at.is_some()
                    && matches!(
                        params.cause_ref.evidence_type(),
                        EvidenceType::MemoryDecision
                            | EvidenceType::MemoryRecord
                            | EvidenceType::DeleteRequest
                            | EvidenceType::PolicyBasis
                    )
            }
        };
        if !transition_is_valid {
            return Err(CoreError::invalid_canonical_object(
                InvalidCanonicalObjectReason::InvalidStateTransition,
            ));
        }
        Ok(Self(params))
    }

    #[must_use]
    pub const fn to_state(&self) -> MemoryState {
        self.0.event_type.to_state()
    }

    #[must_use]
    pub const fn params(&self) -> &MemoryStateEventParams {
        &self.0
    }
}

impl CanonicalObject for MemoryStateEvent {
    fn object_type(&self) -> CanonicalObjectType {
        CanonicalObjectType::MemoryStateEvent
    }

    fn object_id(&self) -> &Identifier {
        &self.0.event_id
    }

    fn namespace_id(&self) -> &Identifier {
        &self.0.namespace_id
    }
}
